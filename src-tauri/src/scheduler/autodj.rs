/// AutoDJ Controller
///
/// Bridges the rotation engine to deck playback. When AutoDJ mode is active,
/// this module monitors the deck state and automatically queues the next track.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DjMode {
    /// Station runs fully automated. Rotation rules select next track.
    AutoDj,
    /// DJ loads queue manually. AutoDJ fills gaps when queue is empty.
    Assisted,
    /// Nothing plays unless DJ actively controls decks. AutoDJ disabled.
    Manual,
}

impl DjMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "autodj" => Self::AutoDj,
            "assisted" => Self::Assisted,
            _ => Self::Manual,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoDj => "autodj",
            Self::Assisted => "assisted",
            Self::Manual => "manual",
        }
    }
}

/// Global DJ mode — stored in memory, persisted to local DB on change
static DJ_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2); // Manual

pub fn get_dj_mode() -> DjMode {
    match DJ_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => DjMode::AutoDj,
        1 => DjMode::Assisted,
        _ => DjMode::Manual,
    }
}

pub fn set_dj_mode(mode: DjMode) {
    let val = match mode {
        DjMode::AutoDj => 0,
        DjMode::Assisted => 1,
        DjMode::Manual => 2,
    };
    DJ_MODE.store(val, std::sync::atomic::Ordering::Relaxed);
}

/// GAP killer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapKillerConfig {
    /// "off" | "smart" | "aggressive"
    pub mode: String,
    /// Signal below this dB is considered silence (default -50.0)
    pub threshold_db: f32,
    /// Minimum silence duration to trigger gap kill in ms (default 500)
    pub min_silence_ms: u32,
}

impl Default for GapKillerConfig {
    fn default() -> Self {
        Self {
            mode: "smart".to_string(),
            threshold_db: -50.0,
            min_silence_ms: 500,
        }
    }
}
