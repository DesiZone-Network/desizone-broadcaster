/// `scripting/engine.rs` — Lua VM manager
///
/// `ScriptEngine` manages the script registry and fires events.
/// Each script runs in its own isolated Lua VM.
/// Output from log.* is captured and stored per-script for the UI.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use mlua::Lua;
use serde::{Deserialize, Serialize};

use super::{
    api::{register_all, ScriptLog, ScriptLogEntry, ScriptStore},
    sandbox::{create_sandboxed_vm, TrustLevel},
    trigger::ScriptEvent,
};

// ── Script record (mirrors DB row) ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub enabled: bool,
    pub trigger_type: String,
    pub last_run_at: Option<i64>,
    pub last_error: Option<String>,
}

// ── Script run result ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptRunResult {
    pub success: bool,
    pub output: Vec<String>,
    pub error: Option<String>,
    pub error_line: Option<u32>,
}

// ── ScriptEngine ──────────────────────────────────────────────────────────────

/// Shared handle — lives in AppState.
#[derive(Clone)]
pub struct ScriptEngine {
    scripts: Arc<Mutex<HashMap<i64, Script>>>,
    /// Per-script log buffers (last 200 entries per script)
    logs: Arc<Mutex<HashMap<i64, Vec<ScriptLogEntry>>>>,
    /// Per-script key/value stores
    stores: Arc<Mutex<HashMap<i64, ScriptStore>>>,
    /// Channel to send events — tokio::sync::broadcast for multi-consumer
    event_tx: tokio::sync::broadcast::Sender<ScriptEvent>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            scripts: Arc::new(Mutex::new(HashMap::new())),
            logs: Arc::new(Mutex::new(HashMap::new())),
            stores: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    // ── Script CRUD ───────────────────────────────────────────────────────

    pub fn save_script(&self, mut script: Script) -> i64 {
        let mut scripts = self.scripts.lock().unwrap();
        if script.id == 0 {
            let next_id = scripts.keys().max().copied().unwrap_or(0) + 1;
            script.id = next_id;
        }
        let id = script.id;
        scripts.insert(id, script);
        // Ensure log and store exist
        self.logs.lock().unwrap().entry(id).or_default();
        let stores = self.stores.lock().unwrap();
        if !stores.contains_key(&id) {
            drop(stores);
            self.stores.lock().unwrap().insert(id, Arc::new(Mutex::new(HashMap::new())));
        }
        id
    }

    pub fn delete_script(&self, id: i64) {
        self.scripts.lock().unwrap().remove(&id);
        self.logs.lock().unwrap().remove(&id);
        self.stores.lock().unwrap().remove(&id);
    }

    pub fn get_scripts(&self) -> Vec<Script> {
        self.scripts.lock().unwrap().values().cloned().collect()
    }

    pub fn get_script(&self, id: i64) -> Option<Script> {
        self.scripts.lock().unwrap().get(&id).cloned()
    }

    pub fn get_log(&self, id: i64, limit: usize) -> Vec<ScriptLogEntry> {
        let logs = self.logs.lock().unwrap();
        let entries = logs.get(&id).cloned().unwrap_or_default();
        let skip = entries.len().saturating_sub(limit);
        entries[skip..].to_vec()
    }

    // ── Event dispatch ────────────────────────────────────────────────────

    /// Fire an event — all enabled scripts whose trigger_type matches will run.
    pub fn fire(&self, event: ScriptEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Spawn a background task that processes events for a script.
    pub fn start_event_loop(&self, id: i64) {
        let engine = self.clone();
        let mut rx = self.event_tx.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let script = engine.get_script(id);
                        if let Some(script) = script {
                            if script.enabled && script.trigger_type == event.trigger_type() {
                                engine.run_script_with_event(&script, &event).await;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(_) => {} // lagged, skip
                }
            }
        });
    }

    // ── Script execution ──────────────────────────────────────────────────

    /// Run a script immediately (manual trigger or event dispatch).
    pub async fn run_script(&self, id: i64) -> ScriptRunResult {
        let script = match self.get_script(id) {
            Some(s) => s,
            None => return ScriptRunResult {
                success: false,
                output: vec![],
                error: Some("Script not found".to_string()),
                error_line: None,
            },
        };
        let event = ScriptEvent::Manual;
        self.run_script_with_event(&script, &event).await
    }

    async fn run_script_with_event(&self, script: &Script, event: &ScriptEvent) -> ScriptRunResult {
        let id = script.id;
        let content = script.content.clone();
        let event = event.clone();

        // Build per-run log sink
        let log_sink: ScriptLog = Arc::new(Mutex::new(Vec::new()));
        let log_sink_clone = Arc::clone(&log_sink);

        // Get or create store
        let store = {
            let stores = self.stores.lock().unwrap();
            stores.get(&id).cloned().unwrap_or_else(|| {
                Arc::new(Mutex::new(HashMap::new()))
            })
        };

        // Run in blocking task (Lua is sync)
        let result = tokio::task::spawn_blocking(move || {
            Self::execute_script(id, &content, &event, log_sink_clone, store)
        }).await.unwrap_or_else(|e| ScriptRunResult {
            success: false,
            output: vec![],
            error: Some(format!("Script task panicked: {e}")),
            error_line: None,
        });

        // Append log entries to global per-script log buffer
        let new_entries: Vec<ScriptLogEntry> = log_sink.lock().unwrap().clone();
        {
            let mut logs = self.logs.lock().unwrap();
            let buf = logs.entry(id).or_default();
            for e in new_entries {
                buf.push(e);
            }
            // Keep only last 200
            if buf.len() > 200 {
                let skip = buf.len() - 200;
                buf.drain(..skip);
            }
        }

        // Update last_run_at and last_error
        {
            let mut scripts = self.scripts.lock().unwrap();
            if let Some(s) = scripts.get_mut(&id) {
                s.last_run_at = Some(chrono::Utc::now().timestamp());
                s.last_error = result.error.clone();
            }
        }

        result
    }

    fn execute_script(
        id: i64,
        content: &str,
        event: &ScriptEvent,
        log_sink: ScriptLog,
        store: ScriptStore,
    ) -> ScriptRunResult {
        // Create a fresh sandboxed VM for each run
        let lua = match create_sandboxed_vm(TrustLevel::Basic) {
            Ok(l) => l,
            Err(e) => return ScriptRunResult {
                success: false,
                output: vec![],
                error: Some(format!("Failed to create Lua VM: {e}")),
                error_line: None,
            },
        };

        // Register DesiZone API
        if let Err(e) = register_all(&lua, id, Arc::clone(&log_sink), Arc::clone(&store)) {
            return ScriptRunResult {
                success: false,
                output: vec![],
                error: Some(format!("API registration failed: {e}")),
                error_line: None,
            };
        }

        // Inject event payload as `event` global table in the Lua VM
        let _ = inject_event_table(&lua, event);

        // Execute the script
        match lua.load(content).exec() {
            Ok(_) => {
                let output: Vec<String> = log_sink.lock().unwrap().iter()
                    .map(|e| format!("[{}] {}", e.level, e.message))
                    .collect();
                ScriptRunResult {
                    success: true,
                    output,
                    error: None,
                    error_line: None,
                }
            }
            Err(e) => {
                let error_str = e.to_string();
                let error_line = parse_error_line(&error_str);
                let output: Vec<String> = log_sink.lock().unwrap().iter()
                    .map(|e| format!("[{}] {}", e.level, e.message))
                    .collect();
                ScriptRunResult {
                    success: false,
                    output,
                    error: Some(error_str),
                    error_line,
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Inject event data as `event` global table in the Lua VM.
fn inject_event_table(lua: &Lua, event: &ScriptEvent) -> Result<(), mlua::Error> {
    let tbl = lua.create_table()?;
    match event {
        ScriptEvent::TrackStart { id, title, artist, album, duration_ms, category } => {
            tbl.set("id", *id)?;
            tbl.set("title", title.as_str())?;
            tbl.set("artist", artist.as_str())?;
            tbl.set("album", album.as_deref().unwrap_or(""))?;
            tbl.set("duration_ms", *duration_ms)?;
            tbl.set("category", category.as_deref().unwrap_or(""))?;
        }
        ScriptEvent::TrackEnd { id, title } => {
            tbl.set("id", *id)?;
            tbl.set("title", title.as_str())?;
        }
        ScriptEvent::QueueEmpty => {}
        ScriptEvent::Hour { hour } => {
            tbl.set("hour", *hour)?;
        }
        ScriptEvent::RequestReceived { song_id, song_title, requester } => {
            tbl.set("song_id", *song_id)?;
            tbl.set("song_title", song_title.as_str())?;
            tbl.set("requester", requester.as_str())?;
        }
        ScriptEvent::EncoderConnect { encoder_id } => {
            tbl.set("encoder_id", *encoder_id)?;
        }
        ScriptEvent::EncoderDisconnect { encoder_id, reason } => {
            tbl.set("encoder_id", *encoder_id)?;
            tbl.set("reason", reason.as_str())?;
        }
        ScriptEvent::CrossfadeStart { outgoing_id, outgoing_title, incoming_id, incoming_title } => {
            tbl.set("outgoing_id", *outgoing_id)?;
            tbl.set("outgoing_title", outgoing_title.as_str())?;
            tbl.set("incoming_id", *incoming_id)?;
            tbl.set("incoming_title", incoming_title.as_str())?;
        }
        ScriptEvent::Manual => {}
    }
    lua.globals().set("event", tbl)?;
    Ok(())
}

/// Parse a line number from mlua error message (e.g. "[string]:5: ...")
fn parse_error_line(err: &str) -> Option<u32> {
    // mlua errors look like: [string "..."]:5: ...
    let colon_parts: Vec<&str> = err.splitn(3, ':').collect();
    if colon_parts.len() >= 2 {
        colon_parts[1].trim().parse::<u32>().ok()
    } else {
        None
    }
}
