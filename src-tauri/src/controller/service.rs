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
    starlight_profile::DEVICE_NAME_HINT,
    types::{
        now_ts_ms, ControllerAction, ControllerConfig, ControllerDevice, ControllerErrorEvent,
        ControllerStatus,
    },
};

const ACTION_QUEUE_SIZE: usize = 512;
const ANALOG_MIN_INTERVAL_MS: u64 = 25;
const ANALOG_MIN_DELTA: f32 = 0.005;
const ANALOG_HEARTBEAT_MS: u64 = 250;
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

struct ControllerInner {
    config: ControllerConfig,
    status: ControllerStatus,
    connection: Option<MidiInputConnection<()>>,
    decode_state: DecodeState,
    analog_state: HashMap<String, AnalogState>,
    jog_state: HashMap<crate::audio::crossfade::DeckId, JogState>,
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
                .spawn(move || {
                    loop {
                        thread::sleep(Duration::from_secs(2));
                        let should_reconnect = {
                            let inner = service.inner.lock().unwrap();
                            inner.config.enabled
                                && inner.config.auto_connect
                                && !inner.status.connected
                        };
                        if should_reconnect {
                            let _ = service.connect(None, &app_handle);
                        }
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
        let mut input =
            MidiInput::new("desizone-controller-input").map_err(|e| format!("MIDI init failed: {e}"))?;
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
                    other => {
                        if self.should_dispatch_action(&mut inner, &other) {
                            actions.push(other);
                        }
                    }
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
                    self.emit_error("Controller action queue disconnected".to_string(), app_handle);
                }
            }
        }
    }

    fn should_dispatch_action(&self, inner: &mut ControllerInner, action: &ControllerAction) -> bool {
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
        state.pending_steps =
            (state.pending_steps + delta_steps as i16).clamp(-JOG_MAX_BATCH_STEPS, JOG_MAX_BATCH_STEPS);

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

    fn flush_due_jog_actions(
        &self,
        inner: &mut ControllerInner,
        out: &mut Vec<ControllerAction>,
    ) {
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
}

fn is_starlight_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains(DEVICE_NAME_HINT)
}

fn device_id(index: usize, name: &str) -> String {
    format!("{index}:{name}")
}
