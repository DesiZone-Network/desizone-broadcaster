use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sqlx::{MySqlPool, SqlitePool};
use tokio::sync::RwLock;

use crate::{
    analytics::health_monitor::HealthMonitor,
    audio::{
        engine::AudioEngine,
        mic_input::{MicConfig, MicInput},
    },
    commands::gateway_commands::AutoPilotStatus,
    gateway::client::GatewayClient,
    gateway::remote_dj::{DjPermissions, RemoteSession},
    scripting::engine::ScriptEngine,
    stream::{
        broadcaster::Broadcaster,
        encoder_manager::EncoderManager,
        icecast::StreamHandle,
    },
};

/// Global application state — shared across all Tauri command handlers.
pub struct AppState {
    pub engine: Mutex<AudioEngine>,
    pub local_db: Option<SqlitePool>,
    /// SAM Broadcaster MySQL pool — wrapped in RwLock so commands can
    /// connect/disconnect at runtime without restarting the app.
    pub sam_db: Arc<RwLock<Option<MySqlPool>>>,
    /// Phase 1 legacy single-stream handle (kept for backward compat)
    pub stream_handle: Mutex<Option<StreamHandle>>,
    /// Phase 4 — multi-encoder manager
    pub encoder_manager: EncoderManager,
    /// Phase 4 — audio fan-out broadcaster
    pub broadcaster: Broadcaster,
    /// Phase 5 — Lua scripting engine
    pub script_engine: ScriptEngine,
    /// Phase 5 — microphone input + Voice FX pipeline
    pub mic_input: MicInput,
    /// Phase 5 — temp path for current voice recording
    pub voice_recording_path: Mutex<Option<String>>,
    /// Phase 6 — Gateway WebSocket client
    pub gateway_client: Mutex<Option<GatewayClient>>,
    /// Phase 6 — AutoPilot status
    pub autopilot_status: Mutex<AutoPilotStatus>,
    /// Phase 6 — Active remote DJ sessions
    pub remote_sessions: Mutex<HashMap<String, RemoteSession>>,
    /// Phase 6 — Remote DJ permissions per session
    pub remote_dj_permissions: Mutex<HashMap<String, DjPermissions>>,
    /// Phase 6 — Live talk active channel
    pub live_talk_active: Mutex<Option<String>>,
    /// Phase 6 — Mix-minus enabled
    pub mix_minus_enabled: Mutex<bool>,
    /// Phase 7 — System health monitor
    pub health_monitor: Arc<HealthMonitor>,
}

impl AppState {
    pub fn new(engine: AudioEngine) -> Self {
        let broadcaster = Broadcaster::new();
        let encoder_manager = EncoderManager::new(broadcaster.clone());
        let mic_input = MicInput::new(MicConfig::default());

        Self {
            engine: Mutex::new(engine),
            local_db: None,
            sam_db: Arc::new(RwLock::new(None)),
            stream_handle: Mutex::new(None),
            encoder_manager,
            broadcaster,
            script_engine: ScriptEngine::new(),
            mic_input,
            voice_recording_path: Mutex::new(None),
            gateway_client: Mutex::new(None),
            autopilot_status: Mutex::new(AutoPilotStatus {
                enabled: false,
                mode: "rotation".to_string(),
                current_rule: None,
            }),
            remote_sessions: Mutex::new(HashMap::new()),
            remote_dj_permissions: Mutex::new(HashMap::new()),
            live_talk_active: Mutex::new(None),
            mix_minus_enabled: Mutex::new(false),
            health_monitor: Arc::new(HealthMonitor::new()),
        }
    }

    pub fn with_local_db(mut self, pool: SqlitePool) -> Self {
        self.local_db = Some(pool);
        self
    }

    pub fn with_sam_db(self, pool: MySqlPool) -> Self {
        // We can't use `mut self` and then call blocking_write, but since this
        // is called at construction time (no concurrent readers yet), it is fine.
        *self.sam_db.blocking_write() = Some(pool);
        self
    }
}

// SAFETY: AudioEngine is !Send due to cpal::Stream, but we wrap it in a Mutex
// and access it only from async command handlers (one at a time, via lock).
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

