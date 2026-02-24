use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

// ── FadeCurve ─────────────────────────────────────────────────────────────────

/// Crossfade curve type — matches SAM Broadcaster Pro curve options exactly.
///
/// The `t` parameter is fade progress in [0.0, 1.0] where 0.0 is the start
/// of the fade and 1.0 is the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FadeCurve {
    Linear,
    Exponential,
    #[default]
    SCurve,
    Logarithmic,
    ConstantPower,
}

impl FadeCurve {
    /// Gain for the outgoing track at fade progress `t` ∈ [0.0, 1.0].
    ///
    /// `t = 0.0` → full volume; `t = 1.0` → silent.
    ///
    /// This is the primary SAM-parity method. For `ConstantPower` this returns
    /// the cosine component (energy-preserving outgoing gain).
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            FadeCurve::Linear => 1.0 - t,
            FadeCurve::Exponential => (1.0 - t).powi(2),
            FadeCurve::SCurve => 0.5 * (1.0 + (PI * t).cos()),
            FadeCurve::Logarithmic => (1.0 + 9.0 * (1.0 - t)).log10() / 10.0_f32.log10(),
            FadeCurve::ConstantPower => (t * std::f32::consts::FRAC_PI_2).cos(),
        }
    }

    /// Gain for the incoming track at fade progress `t` ∈ [0.0, 1.0].
    ///
    /// `t = 0.0` → silent; `t = 1.0` → full volume.
    ///
    /// For all curves except `ConstantPower` the incoming gain mirrors the
    /// outgoing: `1.0 - self.apply(t)`.  For `ConstantPower` it is the sine
    /// component so that `out² + in² = 1` (constant power property).
    pub fn apply_incoming(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            FadeCurve::ConstantPower => (t * std::f32::consts::FRAC_PI_2).sin(),
            other => 1.0 - other.apply(t),
        }
    }

    // ── Legacy aliases kept for callers that used the original names ──────

    /// Alias for [`apply`](FadeCurve::apply).
    #[inline(always)]
    pub fn gain_out(self, t: f32) -> f32 {
        self.apply(t)
    }

    /// Alias for [`apply_incoming`](FadeCurve::apply_incoming).
    #[inline(always)]
    pub fn gain_in(self, t: f32) -> f32 {
        self.apply_incoming(t)
    }

    /// Generate a preview curve for frontend visualization.
    ///
    /// Returns `steps + 1` evenly-spaced [`CurvePoint`] values from `t = 0`
    /// to `t = 1`.
    pub fn preview(self, steps: usize) -> Vec<CurvePoint> {
        (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                CurvePoint { t, gain_out: self.apply(t), gain_in: self.apply_incoming(t) }
            })
            .collect()
    }
}

/// A single point on a fade curve for UI rendering.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CurvePoint {
    pub t: f32,
    pub gain_out: f32,
    pub gain_in: f32,
}

// ── CrossfadeMode ─────────────────────────────────────────────────────────────

/// How the crossfade is triggered or positioned.
///
/// This covers both the *overlap style* used by the real-time engine and the
/// *trigger mode* used by the crossfade state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CrossfadeMode {
    // ── Overlap style (used by the real-time state machine) ───────────────

    /// Both tracks play simultaneously during the overlap window (SAM default).
    #[default]
    Overlap,
    /// Outgoing fades out first, brief silence, then incoming fades in.
    Segue,
    /// Hard cut — no fade applied.
    Instant,

    // ── Trigger mode (used by the high-level state machine) ───────────────

    /// Crossfade is triggered when the outgoing track's RMS drops below the
    /// `auto_detect_db` threshold.
    AutoDetect,
    /// Crossfade is triggered at `(track_duration − fixed_crossfade_ms)` ms.
    Fixed,
    /// Crossfade is triggered explicitly by the user or an external script.
    Manual,
}

// ── CrossfadeConfig ───────────────────────────────────────────────────────────

/// Full SAM Broadcaster parity — maps to every field in SAM's Cross-Fading
/// dialog plus the additional trigger-mode fields needed by the DBE engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossfadeConfig {
    // ── Fade Out ──────────────────────────────────────────────────────────
    pub fade_out_enabled: bool,
    pub fade_out_curve: FadeCurve,
    /// Fade-out duration in milliseconds (e.g. 3000).
    pub fade_out_time_ms: u32,
    /// Maximum outgoing level at the start of the fade (0–100 %).
    pub fade_out_level_pct: u8,

    // ── Fade In ───────────────────────────────────────────────────────────
    pub fade_in_enabled: bool,
    pub fade_in_curve: FadeCurve,
    /// Fade-in duration in milliseconds (e.g. 3000).
    pub fade_in_time_ms: u32,
    /// Maximum incoming level at the end of the fade (0–100 %).
    pub fade_in_level_pct: u8,

    // ── Cross-fade trigger ────────────────────────────────────────────────
    pub crossfade_mode: CrossfadeMode,
    /// Used when `crossfade_mode == Fixed`: begin fade this many ms before
    /// the track end.
    pub fixed_crossfade_ms: u32,
    /// Used when `crossfade_mode == AutoDetect`: trigger threshold in dBFS.
    /// Typical value: −3.0.
    pub auto_detect_db: f32,
    /// Minimum crossfade duration that will be applied (ms).
    pub min_fade_time_ms: u32,
    /// Maximum crossfade duration that will be applied (ms).
    pub max_fade_time_ms: u32,
    /// Do not apply crossfade to tracks shorter than this (seconds).
    /// `None` means no minimum.
    pub skip_short_tracks_secs: Option<u32>,

    // ── Legacy auto-detect fields (kept for engine.rs compatibility) ──────
    /// Master switch for the auto-detect trigger.
    pub auto_detect_enabled: bool,
    /// Minimum ms from track start before auto-detect is allowed to fire.
    pub auto_detect_min_ms: u32,
    /// Maximum ms from track end to search for the auto-detect trigger.
    pub auto_detect_max_ms: u32,
    /// If `Some`, crossfade begins this many ms before the track's xfade cue
    /// point (or end).  Overrides auto-detect when set.
    pub fixed_crossfade_point_ms: Option<u32>,
}

impl Default for CrossfadeConfig {
    fn default() -> Self {
        Self {
            // Fade out — SAM defaults
            fade_out_enabled: true,
            fade_out_curve: FadeCurve::Exponential,
            fade_out_time_ms: 3000,
            fade_out_level_pct: 80,

            // Fade in — SAM defaults
            fade_in_enabled: true,
            fade_in_curve: FadeCurve::SCurve,
            fade_in_time_ms: 3000,
            fade_in_level_pct: 100,

            // Cross-fade trigger
            crossfade_mode: CrossfadeMode::AutoDetect,
            fixed_crossfade_ms: 8000,
            auto_detect_db: -3.0,
            min_fade_time_ms: 3000,
            max_fade_time_ms: 10000,
            skip_short_tracks_secs: Some(30),

            // Legacy engine fields
            auto_detect_enabled: true,
            auto_detect_min_ms: 500,
            auto_detect_max_ms: 15000,
            fixed_crossfade_point_ms: None,
        }
    }
}

// ── SongFadeOverride ──────────────────────────────────────────────────────────

/// Per-song fade overrides — if all fields are `None`, inherit from
/// [`CrossfadeConfig`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SongFadeOverride {
    pub fade_out_enabled: Option<bool>,
    pub fade_out_curve: Option<FadeCurve>,
    pub fade_out_time_ms: Option<u32>,
    pub fade_in_enabled: Option<bool>,
    pub fade_in_curve: Option<FadeCurve>,
    pub fade_in_time_ms: Option<u32>,
    pub crossfade_mode: Option<CrossfadeMode>,
    /// Per-song gain offset in dB.
    pub gain_db: Option<f32>,
}

impl SongFadeOverride {
    /// Merge this override into a base config, returning the effective config.
    pub fn apply_to(&self, base: &CrossfadeConfig) -> CrossfadeConfig {
        CrossfadeConfig {
            fade_out_enabled: self.fade_out_enabled.unwrap_or(base.fade_out_enabled),
            fade_out_curve: self.fade_out_curve.unwrap_or(base.fade_out_curve),
            fade_out_time_ms: self.fade_out_time_ms.unwrap_or(base.fade_out_time_ms),
            fade_in_enabled: self.fade_in_enabled.unwrap_or(base.fade_in_enabled),
            fade_in_curve: self.fade_in_curve.unwrap_or(base.fade_in_curve),
            fade_in_time_ms: self.fade_in_time_ms.unwrap_or(base.fade_in_time_ms),
            crossfade_mode: self.crossfade_mode.unwrap_or(base.crossfade_mode),
            ..*base
        }
    }
}

// ── DeckId ────────────────────────────────────────────────────────────────────

/// Audio channel / deck identifier used throughout the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckId {
    DeckA,
    DeckB,
    SoundFx,
    Aux1,
    VoiceFx,
}

impl std::fmt::Display for DeckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeckId::DeckA => write!(f, "deck_a"),
            DeckId::DeckB => write!(f, "deck_b"),
            DeckId::SoundFx => write!(f, "sound_fx"),
            DeckId::Aux1 => write!(f, "aux_1"),
            DeckId::VoiceFx => write!(f, "voice_fx"),
        }
    }
}

// ── CrossfadeState (real-time engine enum) ────────────────────────────────────

/// Real-time crossfade state machine used by `AudioEngine` on the CPAL thread.
///
/// This enum-based state is allocation-free in its hot path and is the type
/// referenced by `engine.rs`.
#[derive(Debug, Clone)]
pub enum CrossfadeState {
    Idle,
    Fading {
        outgoing: DeckId,
        incoming: DeckId,
        /// Progress 0.0 → 1.0.
        progress: f32,
        total_samples: u64,
        elapsed_samples: u64,
        config: CrossfadeConfig,
    },
    /// Fade completed; engine should mute outgoing and promote incoming.
    Complete {
        new_active: DeckId,
    },
}

impl Default for CrossfadeState {
    fn default() -> Self {
        CrossfadeState::Idle
    }
}

impl CrossfadeState {
    /// Begin a crossfade.  Returns the initial state immediately.
    pub fn start(
        outgoing: DeckId,
        incoming: DeckId,
        config: CrossfadeConfig,
        sample_rate: u32,
    ) -> Self {
        if config.crossfade_mode == CrossfadeMode::Instant {
            return CrossfadeState::Complete { new_active: incoming };
        }

        // Use the longer of the two fade times, clamped to [min, max].
        let raw_ms = config.fade_out_time_ms.max(config.fade_in_time_ms);
        let window_ms = raw_ms
            .max(config.min_fade_time_ms)
            .min(config.max_fade_time_ms);

        let total_samples = (window_ms as u64 * sample_rate as u64) / 1000;

        if total_samples == 0 {
            return CrossfadeState::Complete { new_active: incoming };
        }

        CrossfadeState::Fading {
            outgoing,
            incoming,
            progress: 0.0,
            total_samples,
            elapsed_samples: 0,
            config,
        }
    }

    /// Advance by `frames` samples.  Returns `(gain_out, gain_in, is_complete)`.
    ///
    /// Gains are scaled by the per-deck level percentages from the config.
    /// Call this once per audio callback with the frame count.
    ///
    /// **Called on the real-time audio thread — no allocations.**
    pub fn advance(&mut self, frames: u64) -> (f32, f32, bool) {
        match self {
            CrossfadeState::Fading {
                incoming,
                progress,
                total_samples,
                elapsed_samples,
                config,
                ..
            } => {
                *elapsed_samples = (*elapsed_samples + frames).min(*total_samples);
                *progress = *elapsed_samples as f32 / *total_samples as f32;
                let t = *progress;

                let out_scale = config.fade_out_level_pct as f32 / 100.0;
                let in_scale = config.fade_in_level_pct as f32 / 100.0;

                let gain_out = if config.fade_out_enabled {
                    config.fade_out_curve.apply(t) * out_scale
                } else {
                    1.0
                };
                let gain_in = if config.fade_in_enabled {
                    config.fade_in_curve.apply_incoming(t) * in_scale
                } else {
                    1.0
                };

                if *elapsed_samples >= *total_samples {
                    let new_active = *incoming;
                    *self = CrossfadeState::Complete { new_active };
                    (0.0, 1.0, true)
                } else {
                    (gain_out, gain_in, false)
                }
            }
            CrossfadeState::Idle => (1.0, 0.0, false),
            CrossfadeState::Complete { .. } => (0.0, 1.0, true),
        }
    }

    /// Current fade progress (0.0–1.0), or `None` when not fading.
    pub fn progress(&self) -> Option<f32> {
        match self {
            CrossfadeState::Fading { progress, .. } => Some(*progress),
            _ => None,
        }
    }

    /// The outgoing deck, if a fade is in progress.
    pub fn outgoing(&self) -> Option<DeckId> {
        match self {
            CrossfadeState::Fading { outgoing, .. } => Some(*outgoing),
            _ => None,
        }
    }

    /// The incoming deck, if a fade is in progress.
    pub fn incoming(&self) -> Option<DeckId> {
        match self {
            CrossfadeState::Fading { incoming, .. } => Some(*incoming),
            _ => None,
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, CrossfadeState::Idle)
    }

    pub fn is_fading(&self) -> bool {
        matches!(self, CrossfadeState::Fading { .. })
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, CrossfadeState::Complete { .. })
    }

    /// Reset back to `Idle` (call after the engine has handled `Complete`).
    pub fn reset(&mut self) {
        *self = CrossfadeState::Idle;
    }
}

// ── CrossfadePhase / CrossfadeStateMachine ────────────────────────────────────
//
// These types implement the spec's `CrossfadePhase` + `CrossfadeState` design,
// using per-sample stepping and `usize` deck IDs.  They are offered as a
// higher-level alternative to the real-time `CrossfadeState` enum above and
// are allocation-free in their hot paths.

/// Phase of the high-level crossfade state machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossfadePhase {
    Idle,
    Playing {
        deck_id: usize,
    },
    Crossfading {
        outgoing_deck: usize,
        incoming_deck: usize,
        /// Progress from 0.0 (start) to 1.0 (end).
        progress: f32,
        /// Precomputed increment per sample: `1.0 / (sample_rate × fade_time_s)`.
        step_per_sample: f32,
    },
}

/// High-level crossfade state machine.
///
/// Designed for per-sample advancement on the audio thread.  No allocations
/// occur inside [`CrossfadeStateMachine::advance`].
pub struct CrossfadeStateMachine {
    pub phase: CrossfadePhase,
    pub config: CrossfadeConfig,
}

impl CrossfadeStateMachine {
    /// Create a new state machine in the `Idle` phase.
    pub fn new(config: CrossfadeConfig) -> Self {
        Self { phase: CrossfadePhase::Idle, config }
    }

    /// Start a crossfade from `outgoing_deck` to `incoming_deck`.
    ///
    /// `sample_rate` is the audio output sample rate (e.g. 44 100 or 48 000).
    /// The fade time is taken from `config.fade_out_time_ms` and clamped to
    /// `[min_fade_time_ms, max_fade_time_ms]`.
    pub fn start(&mut self, outgoing_deck: usize, incoming_deck: usize, sample_rate: u32) {
        let raw_ms = self.config.fade_out_time_ms;
        let clamped_ms = raw_ms
            .max(self.config.min_fade_time_ms)
            .min(self.config.max_fade_time_ms);

        let fade_time_secs = clamped_ms as f32 / 1000.0;
        let total_samples = sample_rate as f32 * fade_time_secs;

        let step_per_sample = if total_samples > 0.0 {
            1.0 / total_samples
        } else {
            1.0 // Degenerate: complete immediately on first advance
        };

        self.phase = CrossfadePhase::Crossfading {
            outgoing_deck,
            incoming_deck,
            progress: 0.0,
            step_per_sample,
        };
    }

    /// Advance by one sample.
    ///
    /// Returns `Some((outgoing_gain, incoming_gain))` with linear amplitudes,
    /// or `None` when the crossfade is complete (caller should transition the
    /// phase to `Playing { deck_id: incoming_deck }`).
    ///
    /// **Called on the real-time audio thread — no allocations.**
    pub fn advance(&mut self) -> Option<(f32, f32)> {
        match &mut self.phase {
            CrossfadePhase::Crossfading {
                outgoing_deck: _,
                incoming_deck,
                progress,
                step_per_sample,
            } => {
                let t = *progress;

                let out_scale = self.config.fade_out_level_pct as f32 / 100.0;
                let in_scale = self.config.fade_in_level_pct as f32 / 100.0;

                let outgoing_gain = self.config.fade_out_curve.apply(t) * out_scale;
                let incoming_gain = self.config.fade_in_curve.apply_incoming(t) * in_scale;

                *progress += *step_per_sample;

                if *progress >= 1.0 {
                    // Crossfade complete — caller must update phase.
                    let _incoming = *incoming_deck;
                    self.phase = CrossfadePhase::Idle;
                    None
                } else {
                    Some((outgoing_gain, incoming_gain))
                }
            }
            _ => None,
        }
    }

    /// Check whether the current RMS level of the outgoing deck should trigger
    /// an auto-detect crossfade.
    ///
    /// `rms_db` is the current RMS in dBFS.
    /// Returns `true` when the level has dropped below the `auto_detect_db`
    /// threshold in the config.
    pub fn should_auto_trigger(&self, rms_db: f32) -> bool {
        rms_db < self.config.auto_detect_db
    }

    /// Compute a preview of the fade curves for UI rendering.
    ///
    /// Returns a `Vec` of `(t, gain_out, gain_in)` tuples for `n_points`
    /// evenly spaced points in [0.0, 1.0].
    ///
    /// This is the only method on `CrossfadeStateMachine` that allocates; it
    /// is intended for UI use, not the audio thread.
    pub fn preview_curve(
        curve_out: FadeCurve,
        curve_in: FadeCurve,
        n_points: usize,
    ) -> Vec<(f32, f32, f32)> {
        if n_points == 0 {
            return Vec::new();
        }
        let last = (n_points - 1).max(1) as f32;
        (0..n_points)
            .map(|i| {
                let t = i as f32 / last;
                let t_clamped = t.clamp(0.0, 1.0);
                (t_clamped, curve_out.apply(t_clamped), curve_in.apply_incoming(t_clamped))
            })
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── FadeCurve boundary conditions ────────────────────────────────────

    #[test]
    fn linear_gain_boundaries() {
        let c = FadeCurve::Linear;
        assert!((c.apply(0.0) - 1.0).abs() < 1e-6, "linear out t=0 should be 1.0");
        assert!((c.apply(1.0) - 0.0).abs() < 1e-6, "linear out t=1 should be 0.0");
        assert!((c.apply_incoming(0.0) - 0.0).abs() < 1e-6, "linear in t=0 should be 0.0");
        assert!((c.apply_incoming(1.0) - 1.0).abs() < 1e-6, "linear in t=1 should be 1.0");
    }

    #[test]
    fn exponential_gain_boundaries() {
        let c = FadeCurve::Exponential;
        assert!((c.apply(0.0) - 1.0).abs() < 1e-6, "exp out t=0 should be 1.0");
        assert!((c.apply(1.0) - 0.0).abs() < 1e-6, "exp out t=1 should be 0.0");
        assert!((c.apply_incoming(0.0) - 0.0).abs() < 1e-6, "exp in t=0 should be 0.0");
        assert!((c.apply_incoming(1.0) - 1.0).abs() < 1e-6, "exp in t=1 should be 1.0");
    }

    #[test]
    fn scurve_gain_boundaries() {
        let c = FadeCurve::SCurve;
        assert!((c.apply(0.0) - 1.0).abs() < 1e-6, "scurve out t=0 should be 1.0");
        assert!((c.apply(1.0) - 0.0).abs() < 1e-6, "scurve out t=1 should be 0.0");
        assert!((c.apply(0.5) - 0.5).abs() < 1e-5, "scurve out t=0.5 should be 0.5");
        assert!((c.apply_incoming(0.0) - 0.0).abs() < 1e-6, "scurve in t=0 should be 0.0");
        assert!((c.apply_incoming(1.0) - 1.0).abs() < 1e-6, "scurve in t=1 should be 1.0");
    }

    #[test]
    fn logarithmic_gain_boundaries() {
        let c = FadeCurve::Logarithmic;
        assert!((c.apply(0.0) - 1.0).abs() < 1e-5, "log out t=0 should be 1.0");
        assert!((c.apply(1.0) - 0.0).abs() < 1e-5, "log out t=1 should be 0.0");
        assert!((c.apply_incoming(0.0) - 0.0).abs() < 1e-5, "log in t=0 should be 0.0");
        assert!((c.apply_incoming(1.0) - 1.0).abs() < 1e-5, "log in t=1 should be 1.0");
    }

    #[test]
    fn constant_power_unity_sum() {
        let c = FadeCurve::ConstantPower;
        // At any t, out² + in² == 1.0 (constant power property).
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let out = c.apply(t);
            let inp = c.apply_incoming(t);
            assert!(
                (out * out + inp * inp - 1.0).abs() < 1e-5,
                "ConstantPower: out²+in² should be 1.0 at t={t}, got {}",
                out * out + inp * inp
            );
        }
    }

    #[test]
    fn constant_power_boundaries() {
        let c = FadeCurve::ConstantPower;
        assert!((c.apply(0.0) - 1.0).abs() < 1e-6, "cp out t=0 should be 1.0");
        assert!((c.apply(1.0) - 0.0).abs() < 1e-6, "cp out t=1 should be 0.0");
        assert!((c.apply_incoming(0.0) - 0.0).abs() < 1e-6, "cp in t=0 should be 0.0");
        assert!((c.apply_incoming(1.0) - 1.0).abs() < 1e-6, "cp in t=1 should be 1.0");
    }

    #[test]
    fn clamp_outside_range() {
        // Values outside [0, 1] must be clamped, not panic.
        let c = FadeCurve::SCurve;
        let _ = c.apply(-0.5);
        let _ = c.apply(1.5);
        let _ = c.apply_incoming(-0.5);
        let _ = c.apply_incoming(1.5);
    }

    #[test]
    fn gain_out_alias_equals_apply() {
        for curve in [
            FadeCurve::Linear,
            FadeCurve::Exponential,
            FadeCurve::SCurve,
            FadeCurve::Logarithmic,
            FadeCurve::ConstantPower,
        ] {
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                assert_eq!(
                    curve.gain_out(t),
                    curve.apply(t),
                    "gain_out != apply for {curve:?} at t={t}"
                );
                assert_eq!(
                    curve.gain_in(t),
                    curve.apply_incoming(t),
                    "gain_in != apply_incoming for {curve:?} at t={t}"
                );
            }
        }
    }

    // ── CrossfadeState (real-time enum) ──────────────────────────────────

    #[test]
    fn state_machine_advances_to_complete() {
        let config = CrossfadeConfig::default();
        let sample_rate = 44100_u32;
        let mut state = CrossfadeState::start(DeckId::DeckA, DeckId::DeckB, config, sample_rate);

        assert!(state.is_fading());

        // Advance by the full fade window in one shot.
        let window_ms = 3000_u64; // default min_fade_time_ms
        let total_samples = window_ms * sample_rate as u64 / 1000;
        let (gain_out, gain_in, complete) = state.advance(total_samples);

        assert!(complete, "crossfade should be complete");
        assert!((gain_out - 0.0).abs() < 1e-6, "outgoing gain should be 0");
        assert!((gain_in - 1.0).abs() < 1e-6, "incoming gain should be 1");
        assert!(state.is_complete());
    }

    #[test]
    fn instant_mode_completes_immediately() {
        let mut config = CrossfadeConfig::default();
        config.crossfade_mode = CrossfadeMode::Instant;
        let state = CrossfadeState::start(DeckId::DeckA, DeckId::DeckB, config, 44100);
        assert!(state.is_complete());
    }

    #[test]
    fn idle_state_returns_full_out_gain() {
        let mut state = CrossfadeState::Idle;
        let (gain_out, gain_in, complete) = state.advance(1024);
        assert!((gain_out - 1.0).abs() < 1e-6);
        assert!((gain_in - 0.0).abs() < 1e-6);
        assert!(!complete);
    }

    #[test]
    fn complete_state_returns_full_in_gain() {
        let mut state = CrossfadeState::Complete { new_active: DeckId::DeckB };
        let (gain_out, gain_in, complete) = state.advance(1024);
        assert!((gain_out - 0.0).abs() < 1e-6);
        assert!((gain_in - 1.0).abs() < 1e-6);
        assert!(complete);
    }

    #[test]
    fn reset_returns_to_idle() {
        let mut config = CrossfadeConfig::default();
        config.crossfade_mode = CrossfadeMode::Instant;
        let mut state = CrossfadeState::start(DeckId::DeckA, DeckId::DeckB, config, 44100);
        assert!(state.is_complete());
        state.reset();
        assert!(state.is_idle());
    }

    // ── CrossfadeStateMachine (per-sample state machine) ─────────────────

    #[test]
    fn state_machine_advance_returns_none_at_end() {
        let config = CrossfadeConfig::default();
        let mut machine = CrossfadeStateMachine::new(config);
        machine.start(0, 1, 44100);

        assert!(matches!(machine.phase, CrossfadePhase::Crossfading { .. }));

        // Advance until complete.
        let mut last = None;
        for _ in 0..(44100 * 10 + 1) {
            // max 10 s safety bound
            last = machine.advance();
            if last.is_none() {
                break;
            }
        }
        assert!(last.is_none(), "advance() should eventually return None");
        assert!(
            matches!(machine.phase, CrossfadePhase::Idle),
            "phase should revert to Idle after completion"
        );
    }

    #[test]
    fn state_machine_gains_are_in_range() {
        let config = CrossfadeConfig::default();
        let mut machine = CrossfadeStateMachine::new(config);
        machine.start(0, 1, 44100);

        for _ in 0..1000 {
            if let Some((out, inp)) = machine.advance() {
                assert!(out >= 0.0 && out <= 1.0, "outgoing gain out of range: {out}");
                assert!(inp >= 0.0 && inp <= 1.0, "incoming gain out of range: {inp}");
            } else {
                break;
            }
        }
    }

    #[test]
    fn should_auto_trigger_threshold() {
        let config = CrossfadeConfig::default(); // auto_detect_db = -3.0
        let machine = CrossfadeStateMachine::new(config);
        assert!(machine.should_auto_trigger(-10.0), "-10 dB should trigger");
        assert!(machine.should_auto_trigger(-3.1), "-3.1 dB should trigger");
        assert!(!machine.should_auto_trigger(-3.0), "-3.0 dB exactly should not trigger");
        assert!(!machine.should_auto_trigger(0.0), "0 dB should not trigger");
    }

    #[test]
    fn preview_curve_length_and_range() {
        let points = CrossfadeStateMachine::preview_curve(FadeCurve::SCurve, FadeCurve::SCurve, 11);
        assert_eq!(points.len(), 11);
        let (t0, out0, in0) = points[0];
        let (t1, out1, in1) = points[10];
        assert!((t0 - 0.0).abs() < 1e-6);
        assert!((out0 - 1.0).abs() < 1e-5, "out at t=0 should be 1.0, got {out0}");
        assert!((in0 - 0.0).abs() < 1e-5, "in at t=0 should be 0.0, got {in0}");
        assert!((t1 - 1.0).abs() < 1e-6);
        assert!((out1 - 0.0).abs() < 1e-5, "out at t=1 should be 0.0, got {out1}");
        assert!((in1 - 1.0).abs() < 1e-5, "in at t=1 should be 1.0, got {in1}");
    }

    #[test]
    fn preview_curve_empty() {
        let points = CrossfadeStateMachine::preview_curve(FadeCurve::Linear, FadeCurve::Linear, 0);
        assert!(points.is_empty());
    }

    #[test]
    fn song_fade_override_apply() {
        let base = CrossfadeConfig::default();
        let override_ = SongFadeOverride {
            fade_out_curve: Some(FadeCurve::Linear),
            fade_out_time_ms: Some(1500),
            ..Default::default()
        };
        let effective = override_.apply_to(&base);
        assert_eq!(effective.fade_out_curve, FadeCurve::Linear);
        assert_eq!(effective.fade_out_time_ms, 1500);
        // Fields not in override must come from base.
        assert_eq!(effective.fade_in_curve, base.fade_in_curve);
        assert_eq!(effective.fade_in_time_ms, base.fade_in_time_ms);
    }

    #[test]
    fn crossfade_config_default_sam_parity() {
        let cfg = CrossfadeConfig::default();
        assert!(cfg.fade_out_enabled);
        assert_eq!(cfg.fade_out_curve, FadeCurve::Exponential);
        assert_eq!(cfg.fade_out_time_ms, 3000);
        assert_eq!(cfg.fade_out_level_pct, 80);
        assert!(cfg.fade_in_enabled);
        assert_eq!(cfg.fade_in_curve, FadeCurve::SCurve);
        assert_eq!(cfg.fade_in_time_ms, 3000);
        assert_eq!(cfg.fade_in_level_pct, 100);
        assert_eq!(cfg.crossfade_mode, CrossfadeMode::AutoDetect);
        assert_eq!(cfg.fixed_crossfade_ms, 8000);
        assert!((cfg.auto_detect_db - (-3.0)).abs() < 1e-6);
        assert_eq!(cfg.min_fade_time_ms, 3000);
        assert_eq!(cfg.max_fade_time_ms, 10000);
        assert_eq!(cfg.skip_short_tracks_secs, Some(30));
    }
}
