use std::{
    collections::HashMap,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use midir::{Ignore, MidiInput, MidiInputConnection};
use tauri::{AppHandle, Emitter};

use super::{
    decode::{decode_message, DecodeState},
    executor::execute_action,
    starlight_profile::{DEVICE_NAME_HINT, MASTER_VOLUME_CC, XFADE_CC, XFADE_STATUS},
    types::{
        now_ts_ms, ControllerAction, ControllerConfig, ControllerDevice, ControllerErrorEvent,
        ControllerStatus,
    },
};

const ACTION_QUEUE_SIZE: usize = 512;
const ANALOG_MIN_INTERVAL_MS: u64 = 25;
const ANALOG_MIN_DELTA: f32 = 0.005;
const ANALOG_HEARTBEAT_MS: u64 = 250;
const XFADE_RELATIVE_PATTERN_MIN_MAGNITUDE: f32 = 0.75;
const XFADE_RELATIVE_PATTERN_HITS: u8 = 5;
const XFADE_RELATIVE_DEADZONE: f32 = 0.02;
const JOG_FLUSH_INTERVAL_MS: u64 = 80;
const JOG_FLUSH_STEP_TRIGGER: i16 = 4;
const JOG_MAX_BATCH_STEPS: i16 = 12;

struct AnalogState {
    last_sent_at: Instant,
    last_value: f32,
}

struct JogState {
    pending_steps: i16,
    last_sent_at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CrossfaderMode {
    Unknown,
    Absolute,
    Relative,
}

struct CrossfaderState {
    mode: CrossfaderMode,
    virtual_position: f32,
    last_raw_position: Option<f32>,
    last_raw_at: Option<Instant>,
    relative_hits: u8,
}

impl Default for CrossfaderState {
    fn default() -> Self {
        Self {
            mode: CrossfaderMode::Unknown,
            virtual_position: 0.0,
            last_raw_position: None,
            last_raw_at: None,
            relative_hits: 0,
        }
    }
}

struct ControllerInner {
    config: ControllerConfig,
    status: ControllerStatus,
    connection: Option<MidiInputConnection<()>>,
    decode_state: DecodeState,
    analog_state: HashMap<String, AnalogState>,
    jog_state: HashMap<crate::audio::crossfade::DeckId, JogState>,
    crossfader_state: CrossfaderState,
    learned_headphone_level_cc: Option<u8>,
    worker_started: bool,
    reconnect_started: bool,
}

#[derive(Clone)]
pub struct ControllerService {
    inner: Arc<Mutex<ControllerInner>>,
    action_tx: SyncSender<ControllerAction>,
    action_rx: Arc<Mutex<Option<Receiver<ControllerAction>>>>,
}

impl ControllerService {
    pub fn new() -> Self {
        let (action_tx, action_rx) = sync_channel(ACTION_QUEUE_SIZE);
        Self {
            inner: Arc::new(Mutex::new(ControllerInner {
                config: ControllerConfig::default(),
                status: ControllerStatus::default(),
                connection: None,
                decode_state: DecodeState::default(),
                analog_state: HashMap::new(),
                jog_state: HashMap::new(),
                crossfader_state: CrossfaderState::default(),
                learned_headphone_level_cc: None,
                worker_started: false,
                reconnect_started: false,
            })),
            action_tx,
            action_rx: Arc::new(Mutex::new(Some(action_rx))),
        }
    }

    pub fn start_background(&self, app_handle: AppHandle) {
        let maybe_rx = {
            let mut inner = self.inner.lock().unwrap();
            if inner.worker_started {
                None
            } else {
                inner.worker_started = true;
                self.action_rx.lock().unwrap().take()
            }
        };
        if let Some(rx) = maybe_rx {
            let app = app_handle.clone();
            thread::Builder::new()
                .name("controller-action-worker".to_string())
                .spawn(move || {
                    while let Ok(action) = rx.recv() {
                        let app_clone = app.clone();
                        tauri::async_runtime::spawn(async move {
                            execute_action(app_clone, action).await;
                        });
                    }
                })
                .ok();
        }

        let should_start_reconnect = {
            let mut inner = self.inner.lock().unwrap();
            if inner.reconnect_started {
                false
            } else {
                inner.reconnect_started = true;
                true
            }
        };
        if should_start_reconnect {
            let service = self.clone();
            thread::Builder::new()
                .name("controller-reconnect".to_string())
                .spawn(move || loop {
                    thread::sleep(Duration::from_secs(2));
                    let should_reconnect = {
                        let inner = service.inner.lock().unwrap();
                        inner.config.enabled && inner.config.auto_connect && !inner.status.connected
                    };
                    if should_reconnect {
                        let _ = service.connect(None, &app_handle);
                    }
                })
                .ok();
        }
    }

    pub fn get_config(&self) -> ControllerConfig {
        self.inner.lock().unwrap().config.clone()
    }

    pub fn set_config(&self, config: ControllerConfig, app_handle: Option<&AppHandle>) {
        let status = {
            let mut inner = self.inner.lock().unwrap();
            inner.config = config.clone();
            inner.status.enabled = config.enabled;
            inner.status.profile = config.profile.clone();
            inner.status.clone()
        };
        if !config.enabled {
            if let Some(app) = app_handle {
                let _ = self.disconnect(app);
            }
        } else if let Some(app) = app_handle {
            let _ = app.emit("controller_status_changed", status);
        }
    }

    pub fn get_status(&self) -> ControllerStatus {
        self.inner.lock().unwrap().status.clone()
    }

    pub fn list_devices(&self) -> Result<Vec<ControllerDevice>, String> {
        let input = MidiInput::new("desizone-controller-discovery")
            .map_err(|e| format!("MIDI init failed: {e}"))?;
        let ports = input.ports();
        let status = self.get_status();

        let out = ports
            .iter()
            .enumerate()
            .filter_map(|(index, port)| {
                let name = input.port_name(port).ok()?;
                let id = device_id(index, &name);
                let connected =
                    status.connected && status.active_device_id.as_deref() == Some(id.as_str());
                Some(ControllerDevice {
                    id,
                    is_starlight_candidate: is_starlight_name(&name),
                    name,
                    connected,
                })
            })
            .collect();
        Ok(out)
    }

    pub fn connect(
        &self,
        requested_device_id: Option<String>,
        app_handle: &AppHandle,
    ) -> Result<ControllerStatus, String> {
        let preferred_device_id = self.get_config().preferred_device_id;
        let mut input = MidiInput::new("desizone-controller-input")
            .map_err(|e| format!("MIDI init failed: {e}"))?;
        input.ignore(Ignore::None);
        let ports = input.ports();

        let mut selected: Option<(usize, String)> = None;
        for (idx, port) in ports.iter().enumerate() {
            if let Ok(name) = input.port_name(port) {
                let id = device_id(idx, &name);
                if requested_device_id.as_deref() == Some(id.as_str()) {
                    selected = Some((idx, name));
                    break;
                }
            }
        }
        if selected.is_none() {
            for (idx, port) in ports.iter().enumerate() {
                if let Ok(name) = input.port_name(port) {
                    let id = device_id(idx, &name);
                    if preferred_device_id.as_deref() == Some(id.as_str()) {
                        selected = Some((idx, name));
                        break;
                    }
                }
            }
        }
        if selected.is_none() {
            for (idx, port) in ports.iter().enumerate() {
                if let Ok(name) = input.port_name(port) {
                    if is_starlight_name(&name) {
                        selected = Some((idx, name));
                        break;
                    }
                }
            }
        }
        if selected.is_none() {
            if let Some(port) = ports.first() {
                if let Ok(name) = input.port_name(port) {
                    selected = Some((0, name));
                }
            }
        }

        let Some((index, name)) = selected else {
            let available: Vec<String> = ports
                .iter()
                .filter_map(|p| input.port_name(p).ok())
                .collect();
            let details = if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            };
            self.set_last_error(
                format!("No MIDI input device found (available: {details})"),
                app_handle,
            );
            return Ok(self.get_status());
        };
        let Some(port) = ports.get(index) else {
            self.set_last_error("Selected MIDI port is unavailable".to_string(), app_handle);
            return Ok(self.get_status());
        };

        let service = self.clone();
        let app = app_handle.clone();
        let conn = input
            .connect(
                port,
                "desizone-starlight-input",
                move |_timestamp, message, _| {
                    service.handle_midi_message(message, &app);
                },
                (),
            )
            .map_err(|e| format!("Failed to connect MIDI input: {e}"))?;

        let status = {
            let mut inner = self.inner.lock().unwrap();
            // Drop any previous connection first.
            let _ = inner.connection.take();
            inner.connection = Some(conn);
            inner.jog_state.clear();
            inner.crossfader_state = CrossfaderState::default();
            inner.learned_headphone_level_cc = None;
            inner.status.connected = true;
            inner.status.active_device_id = Some(device_id(index, &name));
            inner.status.active_device_name = Some(name.clone());
            inner.status.last_error = None;
            inner.status.last_event_at = Some(now_ts_ms());
            inner.status.profile = inner.config.profile.clone();
            inner.status.enabled = inner.config.enabled;
            inner.status.clone()
        };
        let _ = app_handle.emit("controller_status_changed", status.clone());
        Ok(status)
    }

    pub fn disconnect(&self, app_handle: &AppHandle) -> Result<ControllerStatus, String> {
        let status = {
            let mut inner = self.inner.lock().unwrap();
            let _ = inner.connection.take();
            inner.jog_state.clear();
            inner.crossfader_state = CrossfaderState::default();
            inner.learned_headphone_level_cc = None;
            inner.status.connected = false;
            inner.status.active_device_id = None;
            inner.status.active_device_name = None;
            inner.status.last_event_at = Some(now_ts_ms());
            inner.status.clone()
        };
        let _ = app_handle.emit("controller_status_changed", status.clone());
        Ok(status)
    }

    fn handle_midi_message(&self, message: &[u8], app_handle: &AppHandle) {
        let actions = {
            let mut inner = self.inner.lock().unwrap();
            inner.status.last_event_at = Some(now_ts_ms());
            let mut actions = Vec::new();
            for action in decode_message(&mut inner.decode_state, message) {
                match action {
                    ControllerAction::JogNudge { deck, delta_steps } => {
                        if let Some(jog_action) =
                            self.accumulate_jog_action(&mut inner, deck, delta_steps)
                        {
                            actions.push(jog_action);
                        }
                    }
                    ControllerAction::SetCrossfader {
                        position,
                        normalized,
                    } => {
                        if let Some(mapped) = self.normalize_crossfader_action(
                            &mut inner, position, normalized, app_handle,
                        ) {
                            if self.should_dispatch_action(&mut inner, &mapped) {
                                actions.push(mapped);
                            }
                        }
                    }
                    other => {
                        if self.should_dispatch_action(&mut inner, &other) {
                            actions.push(other);
                        }
                    }
                }
            }
            if let Some(action) = self.maybe_decode_headphone_level(&mut inner, message, app_handle)
            {
                if self.should_dispatch_action(&mut inner, &action) {
                    actions.push(action);
                }
            }
            self.flush_due_jog_actions(&mut inner, &mut actions);
            actions
        };

        for action in actions {
            match self.action_tx.try_send(action.clone()) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    // Keep latest analog value by dropping excess analog messages.
                    if action.analog_key_and_value().is_none()
                        && !matches!(action, ControllerAction::JogNudge { .. })
                    {
                        self.emit_error(
                            "Controller action queue is full; dropped button event".to_string(),
                            app_handle,
                        );
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.emit_error(
                        "Controller action queue disconnected".to_string(),
                        app_handle,
                    );
                }
            }
        }
    }

    fn should_dispatch_action(
        &self,
        inner: &mut ControllerInner,
        action: &ControllerAction,
    ) -> bool {
        let Some((key, value)) = action.analog_key_and_value() else {
            return true;
        };
        let now = Instant::now();
        if let Some(prev) = inner.analog_state.get(&key) {
            let elapsed = now.duration_since(prev.last_sent_at);
            if elapsed < Duration::from_millis(ANALOG_MIN_INTERVAL_MS) {
                return false;
            }
            let delta = (value - prev.last_value).abs();
            if delta < ANALOG_MIN_DELTA && elapsed < Duration::from_millis(ANALOG_HEARTBEAT_MS) {
                return false;
            }
        }
        inner.analog_state.insert(
            key,
            AnalogState {
                last_sent_at: now,
                last_value: value,
            },
        );
        true
    }

    fn normalize_crossfader_action(
        &self,
        inner: &mut ControllerInner,
        raw_position: f32,
        raw_normalized: f32,
        app_handle: &AppHandle,
    ) -> Option<ControllerAction> {
        let now = Instant::now();
        let state = &mut inner.crossfader_state;

        if let (Some(prev), Some(prev_at)) = (state.last_raw_position, state.last_raw_at) {
            let rapid = now.duration_since(prev_at) <= Duration::from_millis(20);
            let strong_flip = prev.signum() != raw_position.signum()
                && prev.abs() >= XFADE_RELATIVE_PATTERN_MIN_MAGNITUDE
                && raw_position.abs() >= XFADE_RELATIVE_PATTERN_MIN_MAGNITUDE;
            if rapid && strong_flip {
                state.relative_hits = state.relative_hits.saturating_add(1);
            } else if state.relative_hits > 0 {
                state.relative_hits = state.relative_hits.saturating_sub(1);
            }
        }
        state.last_raw_position = Some(raw_position);
        state.last_raw_at = Some(now);

        if state.mode == CrossfaderMode::Unknown {
            if state.relative_hits >= XFADE_RELATIVE_PATTERN_HITS {
                state.mode = CrossfaderMode::Relative;
                self.emit_error(
                    "Detected relative crossfader MIDI mode; applying compatibility translation."
                        .to_string(),
                    app_handle,
                );
            } else if raw_position.abs() < 0.7 {
                state.mode = CrossfaderMode::Absolute;
            }
        }

        if state.mode == CrossfaderMode::Relative {
            let delta = if raw_position < -XFADE_RELATIVE_DEADZONE {
                (0.01 + raw_position.abs() * 0.03).clamp(0.01, 0.04)
            } else if raw_position > XFADE_RELATIVE_DEADZONE {
                -(0.01 + raw_position.abs() * 0.03).clamp(0.01, 0.04)
            } else {
                0.0
            };
            if delta.abs() <= f32::EPSILON {
                return None;
            }
            state.virtual_position = (state.virtual_position + delta).clamp(-1.0, 1.0);
            let normalized = ((state.virtual_position + 1.0) * 0.5).clamp(0.0, 1.0);
            return Some(ControllerAction::SetCrossfader {
                position: state.virtual_position,
                normalized,
            });
        }

        // Absolute mode: apply light smoothing to damp hardware jitter.
        let alpha = 0.45_f32;
        state.virtual_position = (state.virtual_position
            + alpha * (raw_position - state.virtual_position))
            .clamp(-1.0, 1.0);
        let normalized = ((state.virtual_position + 1.0) * 0.5).clamp(0.0, 1.0);

        // Keep analog key value close to source in case mode just switched to absolute.
        if state.mode == CrossfaderMode::Unknown {
            state.virtual_position = raw_position.clamp(-1.0, 1.0);
            return Some(ControllerAction::SetCrossfader {
                position: state.virtual_position,
                normalized: raw_normalized.clamp(0.0, 1.0),
            });
        }

        Some(ControllerAction::SetCrossfader {
            position: state.virtual_position,
            normalized,
        })
    }

    fn accumulate_jog_action(
        &self,
        inner: &mut ControllerInner,
        deck: crate::audio::crossfade::DeckId,
        delta_steps: i8,
    ) -> Option<ControllerAction> {
        if delta_steps == 0 {
            return None;
        }
        let now = Instant::now();
        let state = inner.jog_state.entry(deck).or_insert_with(|| JogState {
            pending_steps: 0,
            last_sent_at: now
                .checked_sub(Duration::from_millis(JOG_FLUSH_INTERVAL_MS))
                .unwrap_or(now),
        });
        state.pending_steps = (state.pending_steps + delta_steps as i16)
            .clamp(-JOG_MAX_BATCH_STEPS, JOG_MAX_BATCH_STEPS);

        let elapsed = now.duration_since(state.last_sent_at);
        if elapsed < Duration::from_millis(JOG_FLUSH_INTERVAL_MS)
            && state.pending_steps.abs() < JOG_FLUSH_STEP_TRIGGER
        {
            return None;
        }

        let steps = state
            .pending_steps
            .clamp(-JOG_MAX_BATCH_STEPS, JOG_MAX_BATCH_STEPS) as i8;
        state.pending_steps = 0;
        state.last_sent_at = now;
        Some(ControllerAction::JogNudge {
            deck,
            delta_steps: steps,
        })
    }

    fn flush_due_jog_actions(&self, inner: &mut ControllerInner, out: &mut Vec<ControllerAction>) {
        let now = Instant::now();
        let flush_after = Duration::from_millis(JOG_FLUSH_INTERVAL_MS);
        for (deck, state) in inner.jog_state.iter_mut() {
            if state.pending_steps == 0 {
                continue;
            }
            if now.duration_since(state.last_sent_at) < flush_after {
                continue;
            }
            let steps = state
                .pending_steps
                .clamp(-JOG_MAX_BATCH_STEPS, JOG_MAX_BATCH_STEPS) as i8;
            state.pending_steps = 0;
            state.last_sent_at = now;
            out.push(ControllerAction::JogNudge {
                deck: *deck,
                delta_steps: steps,
            });
        }
    }

    fn set_last_error(&self, message: String, app_handle: &AppHandle) {
        let status = {
            let mut inner = self.inner.lock().unwrap();
            inner.status.connected = false;
            inner.status.active_device_id = None;
            inner.status.active_device_name = None;
            inner.status.last_error = Some(message.clone());
            inner.status.last_event_at = Some(now_ts_ms());
            inner.status.clone()
        };
        let _ = app_handle.emit("controller_status_changed", status);
        self.emit_error(message, app_handle);
    }

    fn emit_error(&self, message: String, app_handle: &AppHandle) {
        let payload = ControllerErrorEvent {
            message,
            timestamp: now_ts_ms(),
        };
        let _ = app_handle.emit("controller_error", payload);
    }

    fn maybe_decode_headphone_level(
        &self,
        inner: &mut ControllerInner,
        message: &[u8],
        app_handle: &AppHandle,
    ) -> Option<ControllerAction> {
        if message.len() < 3 {
            return None;
        }
        let status = message[0];
        let cc = message[1];
        let value = message[2];
        if status != XFADE_STATUS {
            return None;
        }
        if cc == XFADE_CC || cc == MASTER_VOLUME_CC || cc == 0x7F {
            return None;
        }

        match inner.learned_headphone_level_cc {
            Some(learned) if learned == cc => {}
            Some(_) => return None,
            None => {
                inner.learned_headphone_level_cc = Some(cc);
                self.emit_error(
                    format!(
                        "Learned headphone level MIDI CC 0x{cc:02X}; mapping this control to headphone level."
                    ),
                    app_handle,
                );
            }
        }

        let normalized = (value as f32 / 127.0).clamp(0.0, 1.0);
        Some(ControllerAction::SetHeadphoneLevel {
            level: normalized,
            normalized,
        })
    }
}

fn is_starlight_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains(DEVICE_NAME_HINT)
}

fn device_id(index: usize, name: &str) -> String {
    format!("{index}:{name}")
}
