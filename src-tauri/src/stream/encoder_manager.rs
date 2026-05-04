/// `encoder_manager.rs` — manages N EncoderInstance tasks
///
/// Owns the Broadcaster, spawns per-encoder Tokio tasks, handles reconnect
/// logic, and exposes a clean async API to the Tauri command layer.
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use super::broadcaster::{Broadcaster, EncoderRuntimeState, EncoderStatus, SlotId};

// ── Encoder configuration (mirrors DB table) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputType {
    Icecast,
    Shoutcast,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IcecastVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShoutcastVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    Mp3,
    Aac,
    Ogg,
    Wav,
    Flac,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRotation {
    None,
    Hourly,
    Daily,
    BySize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EncoderConfig {
    pub id: i64,
    pub name: String,
    pub enabled: bool,

    // Codec
    pub codec: Codec,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: u32,
    pub channels: u8,        // 1 = mono, 2 = stereo
    pub quality: Option<u8>, // VBR 0-9

    // Output
    pub output_type: OutputType,

    // Icecast / Shoutcast
    pub server_host: Option<String>,
    pub server_port: Option<u16>,
    pub server_username: Option<String>,
    pub server_password: Option<String>,
    pub mount_point: Option<String>,
    pub icecast_version: IcecastVersion,
    pub shoutcast_version: ShoutcastVersion,
    pub shoutcast_sid: u32,
    pub stream_name: Option<String>,
    pub stream_genre: Option<String>,
    pub stream_url: Option<String>,
    pub stream_description: Option<String>,
    pub is_public: bool,

    // File output
    pub file_output_path: Option<String>,
    pub file_rotation: FileRotation,
    pub file_max_size_mb: u64,
    pub file_name_template: String,

    // Metadata
    pub send_metadata: bool,
    pub icy_metadata_interval: u32,
    pub metadata_caption_template: Option<String>,
    pub metadata_url_append: Option<String>,

    // Reconnect
    pub reconnect_delay_secs: u64,
    pub max_reconnect_attempts: u32, // 0 = infinite
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Encoder".to_string(),
            enabled: false,
            codec: Codec::Mp3,
            bitrate_kbps: Some(128),
            sample_rate: 44100,
            channels: 2,
            quality: None,
            output_type: OutputType::Icecast,
            server_host: Some("localhost".to_string()),
            server_port: Some(8000),
            server_username: Some("source".to_string()),
            server_password: Some("hackme".to_string()),
            mount_point: Some("/stream".to_string()),
            icecast_version: IcecastVersion::V2,
            shoutcast_version: ShoutcastVersion::V2,
            shoutcast_sid: 1,
            stream_name: Some("DesiZone Radio".to_string()),
            stream_genre: Some("Various".to_string()),
            stream_url: None,
            stream_description: None,
            is_public: false,
            file_output_path: None,
            file_rotation: FileRotation::Hourly,
            file_max_size_mb: 500,
            file_name_template: "{date}-{time}-{station}.mp3".to_string(),
            send_metadata: true,
            icy_metadata_interval: 8192,
            metadata_caption_template: Some("$combine$".to_string()),
            metadata_url_append: None,
            reconnect_delay_secs: 5,
            max_reconnect_attempts: 0,
        }
    }
}

// ── In-memory record for a running encoder task ───────────────────────────────

struct RunningEncoder {
    handle: JoinHandle<()>,
    stop_tx: tokio::sync::oneshot::Sender<()>,
}

// ── EncoderManager ────────────────────────────────────────────────────────────

/// Shared handle — lives in AppState.
#[derive(Clone)]
pub struct EncoderManager {
    broadcaster: Broadcaster,
    // master ring buffer consumer (from AudioEngine)
    // We take it once and keep it here
    configs: Arc<Mutex<HashMap<i64, EncoderConfig>>>,
    runtime: Arc<Mutex<HashMap<i64, EncoderRuntimeState>>>,
    tasks: Arc<Mutex<HashMap<i64, RunningEncoder>>>,
    started_at: Arc<Mutex<HashMap<i64, Instant>>>,
}

impl EncoderManager {
    pub fn new(broadcaster: Broadcaster) -> Self {
        Self {
            broadcaster,
            configs: Arc::new(Mutex::new(HashMap::new())),
            runtime: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            started_at: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // ── Config management ─────────────────────────────────────────────────

    /// Upsert an encoder config.  Returns the assigned id.
    pub fn save_encoder(&self, mut config: EncoderConfig) -> i64 {
        let mut configs = self.configs.lock().unwrap();
        if config.id <= 0 {
            // Assign a new id
            let next_id = configs.keys().max().copied().unwrap_or(0) + 1;
            config.id = next_id;
        }
        let id = config.id;
        configs.insert(id, config);

        // Initialise runtime state if not present
        let mut rt = self.runtime.lock().unwrap();
        rt.entry(id).or_insert_with(|| EncoderRuntimeState {
            id,
            status: EncoderStatus::Disabled,
            listeners: None,
            uptime_secs: 0,
            bytes_sent: 0,
            current_bitrate_kbps: None,
            error: None,
            recording_file: None,
        });
        id
    }

    pub fn delete_encoder(&self, id: i64) {
        self.stop_encoder(id);
        self.configs.lock().unwrap().remove(&id);
        self.runtime.lock().unwrap().remove(&id);
        self.broadcaster.remove_slot(id);
    }

    pub fn get_encoders(&self) -> Vec<EncoderConfig> {
        self.configs.lock().unwrap().values().cloned().collect()
    }

    pub fn get_encoder(&self, id: i64) -> Option<EncoderConfig> {
        self.configs.lock().unwrap().get(&id).cloned()
    }

    // ── Runtime state ─────────────────────────────────────────────────────

    pub fn get_all_runtime(&self) -> Vec<EncoderRuntimeState> {
        self.runtime.lock().unwrap().values().cloned().collect()
    }

    pub fn get_runtime(&self, id: i64) -> Option<EncoderRuntimeState> {
        self.runtime.lock().unwrap().get(&id).cloned()
    }

    pub(crate) fn set_status(&self, id: i64, status: EncoderStatus, error: Option<String>) {
        let mut started = self.started_at.lock().unwrap();
        let mut rt = self.runtime.lock().unwrap();
        if let Some(r) = rt.get_mut(&id) {
            if matches!(status, EncoderStatus::Streaming) {
                started.entry(id).or_insert_with(Instant::now);
            } else {
                started.remove(&id);
                r.uptime_secs = 0;
                r.current_bitrate_kbps = None;
                // Listener counts are only meaningful while actively streaming.
                r.listeners = None;
            }
            r.status = status;
            r.error = error;
        }
    }

    pub fn begin_stream_session(&self, id: i64, bitrate_kbps: Option<u32>) {
        let mut started = self.started_at.lock().unwrap();
        let mut rt = self.runtime.lock().unwrap();
        if let Some(r) = rt.get_mut(&id) {
            r.bytes_sent = 0;
            r.uptime_secs = 0;
            r.current_bitrate_kbps = bitrate_kbps;
        }
        started.insert(id, Instant::now());
    }

    pub fn add_bytes_sent(&self, id: i64, bytes: u64) {
        if bytes == 0 {
            return;
        }
        let mut rt = self.runtime.lock().unwrap();
        if let Some(r) = rt.get_mut(&id) {
            r.bytes_sent = r.bytes_sent.saturating_add(bytes);
        }
    }

    pub fn refresh_runtime_counters(&self) {
        let started = self.started_at.lock().unwrap();
        let mut rt = self.runtime.lock().unwrap();
        let now = Instant::now();
        for (id, started_at) in started.iter() {
            if let Some(r) = rt.get_mut(id) {
                r.uptime_secs = now.saturating_duration_since(*started_at).as_secs();
            }
        }
    }

    pub fn update_listeners(&self, encoder_id: i64, count: u32) {
        let mut rt = self.runtime.lock().unwrap();
        if let Some(r) = rt.get_mut(&encoder_id) {
            r.listeners = Some(count);
        }
    }

    // ── Start / Stop ──────────────────────────────────────────────────────

    pub fn start_encoder(&self, id: i64, master_consumer: Option<ringbuf::HeapCons<f32>>) {
        self.start_encoder_with_sample_rate(id, None, master_consumer);
    }

    pub fn start_encoder_with_sample_rate(
        &self,
        id: i64,
        source_sample_rate: Option<u32>,
        master_consumer: Option<ringbuf::HeapCons<f32>>,
    ) {
        let mut config = match self.get_encoder(id) {
            Some(c) => c,
            None => {
                log::error!("start_encoder: encoder {id} not found");
                return;
            }
        };

        if let Some(sr) = source_sample_rate {
            if sr > 0 && config.sample_rate != sr {
                log::info!(
                    "Encoder {} sample rate aligned to engine output: {} -> {}",
                    id,
                    config.sample_rate,
                    sr
                );
                config.sample_rate = sr;
            }
        }

        let already_active = self
            .runtime
            .lock()
            .unwrap()
            .get(&id)
            .map(|rt| {
                matches!(
                    rt.status,
                    EncoderStatus::Connecting
                        | EncoderStatus::Streaming
                        | EncoderStatus::Recording
                        | EncoderStatus::Retrying { .. }
                )
            })
            .unwrap_or(false);
        if already_active {
            log::info!(
                "start_encoder: encoder {} already active; ignoring duplicate start",
                id
            );
            return;
        }

        // If already running, stop first
        self.stop_encoder(id);

        // Register a slot in the broadcaster
        let consumer = self.broadcaster.add_slot(id as SlotId);

        // If we got the master consumer this is the first encoder start —
        // kick off the broadcast loop that feeds master → all slots.
        // In practice: we pass None here and the broadcast loop is started
        // separately by the command layer once the engine produces audio.
        // The slot consumer is all that each task needs.

        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

        let manager_clone = self.clone();
        let config_clone = config.clone();

        let handle = tokio::spawn(async move {
            run_encoder_task(config_clone, consumer, stop_rx, manager_clone).await;
        });

        self.tasks
            .lock()
            .unwrap()
            .insert(id, RunningEncoder { handle, stop_tx });
        self.set_status(id, EncoderStatus::Connecting, None);
    }

    pub fn stop_encoder(&self, id: i64) {
        if let Some(running) = self.tasks.lock().unwrap().remove(&id) {
            let _ = running.stop_tx.send(());
            running.handle.abort();
        }
        self.broadcaster.remove_slot(id);
        self.set_status(id, EncoderStatus::Disabled, None);
    }

    pub fn start_all(&self) {
        self.start_all_with_sample_rate(None);
    }

    pub fn start_all_with_sample_rate(&self, source_sample_rate: Option<u32>) {
        let ids: Vec<i64> = self
            .configs
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.enabled)
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.start_encoder_with_sample_rate(id, source_sample_rate, None);
        }
    }

    pub fn stop_all(&self) {
        let ids: Vec<i64> = self.tasks.lock().unwrap().keys().copied().collect();
        for id in ids {
            self.stop_encoder(id);
        }
    }

    // ── Connection test ───────────────────────────────────────────────────

    pub async fn test_connection(&self, id: i64) -> Result<(), String> {
        let config = self.get_encoder(id).ok_or("Encoder not found")?;
        let host = config.server_host.as_deref().unwrap_or("localhost");
        let port = config.server_port.unwrap_or(8000);
        let user = config.server_username.as_deref().unwrap_or("<unset>");
        let mount = config.mount_point.as_deref().unwrap_or("/stream");
        match config.output_type {
            OutputType::Icecast => {
                log::info!(
                    "Encoder test: type=icecast id={} host={} port={} mount={} user={} version={:?}",
                    config.id,
                    host,
                    port,
                    mount,
                    user,
                    config.icecast_version
                );
                super::icecast::test_icecast_connection(&config).await
            }
            OutputType::Shoutcast => {
                log::info!(
                    "Encoder test: type=shoutcast id={} host={} port={} sid={} user={} version={:?}",
                    config.id,
                    host,
                    port,
                    config.shoutcast_sid,
                    user,
                    config.shoutcast_version
                );
                super::shoutcast::test_shoutcast_connection(&config).await
            }
            OutputType::File => {
                log::info!("Encoder test: type=file id={} (always passes)", config.id);
                Ok(())
            }
        }
    }

    // ── Metadata push ─────────────────────────────────────────────────────

    pub async fn push_metadata(&self, artist: &str, title: &str) {
        let configs = self.get_encoders();
        for cfg in &configs {
            if !cfg.send_metadata {
                continue;
            }
            let combined = format!("{artist} - {title}");
            let song = cfg
                .metadata_caption_template
                .as_deref()
                .filter(|t| !t.trim().is_empty())
                .map(|template| {
                    template
                        .replace("$combine$", &combined)
                        .replace("$artist$", artist)
                        .replace("$title$", title)
                })
                .unwrap_or(combined);
            match cfg.output_type {
                OutputType::Icecast => {
                    if let Err(e) =
                        super::metadata_pusher::push_icecast_metadata(cfg, artist, title, &song)
                            .await
                    {
                        log::warn!("Metadata push failed for encoder {}: {e}", cfg.id);
                    }
                }
                OutputType::Shoutcast => {
                    if let Err(e) =
                        super::metadata_pusher::push_shoutcast_metadata(cfg, artist, title, &song)
                            .await
                    {
                        log::warn!("Metadata push failed for encoder {}: {e}", cfg.id);
                    }
                }
                OutputType::File => {}
            }
        }
    }
}

// ── Per-encoder async task ────────────────────────────────────────────────────

async fn run_encoder_task(
    config: EncoderConfig,
    mut consumer: ringbuf::HeapCons<f32>,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    manager: EncoderManager,
) {
    let id = config.id;
    let max_attempts = config.max_reconnect_attempts;
    let delay = Duration::from_secs(config.reconnect_delay_secs);
    let mut attempt = 0u32;
    let mut terminal_failed = false;
    let stable_reset_window = Duration::from_secs(30);

    loop {
        // Check stop signal
        if stop_rx.try_recv().is_ok() {
            break;
        }

        let session_started = Instant::now();
        let result = match config.output_type {
            OutputType::Icecast => {
                manager.begin_stream_session(id, config.bitrate_kbps);
                manager.set_status(id, EncoderStatus::Streaming, None);
                std::panic::AssertUnwindSafe(super::icecast::stream_loop_async(
                    &config,
                    &mut consumer,
                    &mut stop_rx,
                    &manager,
                ))
                .catch_unwind()
                .await
                .map_err(panic_payload_to_string)
                .and_then(|r| r)
            }
            OutputType::Shoutcast => {
                manager.begin_stream_session(id, config.bitrate_kbps);
                manager.set_status(id, EncoderStatus::Streaming, None);
                std::panic::AssertUnwindSafe(super::shoutcast::stream_loop_async(
                    &config,
                    &mut consumer,
                    &mut stop_rx,
                    &manager,
                ))
                .catch_unwind()
                .await
                .map_err(panic_payload_to_string)
                .and_then(|r| r)
            }
            OutputType::File => {
                std::panic::AssertUnwindSafe(super::encoder_file::record_loop_async(
                    &config,
                    &mut consumer,
                    &mut stop_rx,
                    &manager,
                ))
                .catch_unwind()
                .await
                .map_err(panic_payload_to_string)
                .and_then(|r| r)
            }
        };

        match result {
            Ok(_) => {
                // Graceful stop requested
                break;
            }
            Err(e) => {
                let run_for = session_started.elapsed();
                if run_for >= stable_reset_window && attempt > 0 {
                    log::info!(
                        "Encoder {id} had a stable run ({:?}); resetting retry counter",
                        run_for
                    );
                    attempt = 0;
                }
                attempt += 1;
                log::warn!("Encoder {id} error (attempt {attempt}): {e}");

                if max_attempts > 0 && attempt >= max_attempts {
                    manager.set_status(id, EncoderStatus::Failed, Some(e));
                    terminal_failed = true;
                    break;
                }

                manager.set_status(
                    id,
                    EncoderStatus::Retrying {
                        attempt,
                        max: max_attempts,
                    },
                    Some(e),
                );

                tokio::time::sleep(delay).await;
                manager.set_status(id, EncoderStatus::Connecting, None);
            }
        }
    }

    if !terminal_failed {
        manager.set_status(id, EncoderStatus::Disabled, None);
    }
    log::info!("Encoder {id} task exited");
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return format!("encoder task panicked: {s}");
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return format!("encoder task panicked: {s}");
    }
    "encoder task panicked with non-string payload".to_string()
}
