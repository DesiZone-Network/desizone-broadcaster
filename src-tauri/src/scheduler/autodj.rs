/// AutoDJ Controller
///
/// Bridges the rotation engine to deck playback. When AutoDJ mode is active,
/// this module monitors the deck state and automatically queues the next track.
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Mutex, OnceLock,
};

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
static DJ_MODE: AtomicU8 = AtomicU8::new(2); // Manual

pub fn get_dj_mode() -> DjMode {
    match DJ_MODE.load(Ordering::Relaxed) {
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
    DJ_MODE.store(val, Ordering::Relaxed);
}

// ── Auto transition mode/config ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutodjTransitionEngine {
    #[default]
    SamClassic,
    MixxxPlanner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoTransitionMode {
    FullIntroOutro,
    FadeAtOutroStart,
    FixedFullTrack,
    FixedSkipSilence,
    FixedStartCenterSkipSilence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixxxPlannerConfig {
    pub enabled: bool,
    pub mode: AutoTransitionMode,
    /// Positive: overlap time. Negative: intentional gap for fixed modes.
    pub transition_time_sec: i32,
    pub min_track_duration_ms: u32,
}

impl Default for MixxxPlannerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: AutoTransitionMode::FullIntroOutro,
            transition_time_sec: 10,
            min_track_duration_ms: 200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTransitionConfig {
    pub engine: AutodjTransitionEngine,
    pub mixxx_planner_config: MixxxPlannerConfig,
}

impl Default for AutoTransitionConfig {
    fn default() -> Self {
        Self {
            engine: AutodjTransitionEngine::SamClassic,
            mixxx_planner_config: MixxxPlannerConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDecisionDebug {
    pub engine: String,
    pub from_deck: Option<String>,
    pub to_deck: Option<String>,
    pub trigger_mode: Option<String>,
    pub reason: String,
    pub outgoing_rms_db: Option<f32>,
    pub threshold_db: Option<f32>,
    pub outgoing_remaining_ms: Option<u64>,
    pub fixed_point_ms: Option<u32>,
    pub hold_ms: Option<u32>,
    pub skip_cause: Option<String>,
}

impl Default for TransitionDecisionDebug {
    fn default() -> Self {
        Self {
            engine: "sam_classic".to_string(),
            from_deck: None,
            to_deck: None,
            trigger_mode: None,
            reason: "idle".to_string(),
            outgoing_rms_db: None,
            threshold_db: None,
            outgoing_remaining_ms: None,
            fixed_point_ms: None,
            hold_ms: None,
            skip_cause: None,
        }
    }
}

static AUTO_TRANSITION_CONFIG: OnceLock<Mutex<AutoTransitionConfig>> = OnceLock::new();
static REPLAN_REQUESTED: AtomicBool = AtomicBool::new(false);
static LAST_TRANSITION_DECISION: OnceLock<Mutex<TransitionDecisionDebug>> = OnceLock::new();

fn auto_transition_cell() -> &'static Mutex<AutoTransitionConfig> {
    AUTO_TRANSITION_CONFIG.get_or_init(|| Mutex::new(AutoTransitionConfig::default()))
}

pub fn get_auto_transition_config() -> AutoTransitionConfig {
    auto_transition_cell().lock().unwrap().clone()
}

pub fn set_auto_transition_config(config: AutoTransitionConfig) {
    *auto_transition_cell().lock().unwrap() = config;
    request_replan();
}

pub fn request_replan() {
    REPLAN_REQUESTED.store(true, Ordering::Relaxed);
}

pub fn take_replan_requested() -> bool {
    REPLAN_REQUESTED.swap(false, Ordering::Relaxed)
}

fn decision_cell() -> &'static Mutex<TransitionDecisionDebug> {
    LAST_TRANSITION_DECISION.get_or_init(|| Mutex::new(TransitionDecisionDebug::default()))
}

pub fn get_last_transition_decision() -> TransitionDecisionDebug {
    decision_cell().lock().unwrap().clone()
}

pub fn set_last_transition_decision(decision: TransitionDecisionDebug) {
    *decision_cell().lock().unwrap() = decision;
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
