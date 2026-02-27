use serde::{Deserialize, Serialize};

use crate::audio::crossfade::DeckId;

pub const STARLIGHT_PROFILE: &str = "hercules_djcontrol_starlight";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerDevice {
    pub id: String,
    pub name: String,
    pub is_starlight_candidate: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerConfig {
    pub enabled: bool,
    pub auto_connect: bool,
    pub preferred_device_id: Option<String>,
    pub profile: String,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_connect: true,
            preferred_device_id: None,
            profile: STARLIGHT_PROFILE.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerStatus {
    pub enabled: bool,
    pub connected: bool,
    pub active_device_id: Option<String>,
    pub active_device_name: Option<String>,
    pub profile: String,
    pub last_error: Option<String>,
    pub last_event_at: Option<i64>,
}

impl Default for ControllerStatus {
    fn default() -> Self {
        Self {
            enabled: true,
            connected: false,
            active_device_id: None,
            active_device_name: None,
            profile: STARLIGHT_PROFILE.to_string(),
            last_error: None,
            last_event_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ControllerErrorEvent {
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub enum ControllerAction {
    TogglePlay { deck: DeckId },
    CueToStart { deck: DeckId },
    SyncToOther { deck: DeckId },
    HotCueTrigger { deck: DeckId, slot: u8 },
    HotCueSet { deck: DeckId, slot: u8 },
    SetBeatLoop { deck: DeckId, beats: u8 },
    ClearLoop { deck: DeckId },
    SetTempo {
        deck: DeckId,
        tempo_pct: f32,
        normalized: f32,
    },
    SetGain {
        deck: DeckId,
        gain: f32,
        normalized: f32,
    },
    SetBass {
        deck: DeckId,
        bass_db: f32,
        normalized: f32,
    },
    SetFilter {
        deck: DeckId,
        amount: f32,
        normalized: f32,
    },
    SetCrossfader {
        position: f32,
        normalized: f32,
    },
    SetMasterVolume {
        level: f32,
        normalized: f32,
    },
    JogNudge {
        deck: DeckId,
        delta_steps: i8,
    },
}

impl ControllerAction {
    pub fn analog_key_and_value(&self) -> Option<(String, f32)> {
        match self {
            ControllerAction::SetTempo {
                deck, normalized, ..
            } => Some((format!("tempo:{deck}"), *normalized)),
            ControllerAction::SetGain {
                deck, normalized, ..
            } => Some((format!("gain:{deck}"), *normalized)),
            ControllerAction::SetBass {
                deck, normalized, ..
            } => Some((format!("bass:{deck}"), *normalized)),
            ControllerAction::SetFilter {
                deck, normalized, ..
            } => Some((format!("filter:{deck}"), *normalized)),
            ControllerAction::SetCrossfader { normalized, .. } => {
                Some(("crossfader".to_string(), *normalized))
            }
            ControllerAction::SetMasterVolume { normalized, .. } => {
                Some(("master_level".to_string(), *normalized))
            }
            _ => None,
        }
    }
}

pub fn now_ts_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
