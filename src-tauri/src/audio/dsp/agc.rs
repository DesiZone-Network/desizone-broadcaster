use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type};
use serde::{Deserialize, Serialize};

/// Pre-emphasis time constant variants (broadcast standard)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreEmphasis {
    /// No pre-emphasis
    None,
    /// 50 µs — Japan / some European markets
    #[default]
    Us50,
    /// 75 µs — North America, most FM stations
    Us75,
}

impl PreEmphasis {
    /// Corner frequency in Hz: f = 1 / (2π × τ)
    fn corner_freq_hz(self) -> Option<f32> {
        match self {
            PreEmphasis::None => None,
            PreEmphasis::Us50 => Some(3183.1), // 1 / (2π × 50e-6)
            PreEmphasis::Us75 => Some(2122.1), // 1 / (2π × 75e-6)
        }
    }
}

/// Gated AGC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgcConfig {
    pub enabled: bool,
    /// Noise gate threshold in dBFS — signals quieter than this are not amplified
    pub gate_db: f32,
    /// Maximum gain the AGC will apply, in dB
    pub max_gain_db: f32,
    /// Target output RMS level in dBFS (e.g. -18.0)
    pub target_db: f32,
    /// Attack time constant in ms — how fast gain decreases when signal is too loud
    pub attack_ms: f32,
    /// Release time constant in ms — how fast gain increases when signal is quiet
    pub release_ms: f32,
    /// Pre-emphasis applied before RMS measurement (affects what the AGC "hears")
    pub pre_emphasis: PreEmphasis,
}

impl Default for AgcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gate_db: -31.0,
            max_gain_db: 5.0,
            target_db: -18.0,
            attack_ms: 100.0,
            release_ms: 500.0,
            pre_emphasis: PreEmphasis::Us75,
        }
    }
}

/// Gated AGC with RMS measurement, attack/release smoothing, and optional pre-emphasis.
///
/// Pre-emphasis is applied to a **sidechain copy** for measurement only; the main
/// audio path is unaffected by the pre-emphasis filter.
pub struct GatedAGC {
    sample_rate: f32,
    config: AgcConfig,

    // Smoothed gain (linear, not dB)
    current_gain: f32,

    // Attack / release coefficients (one-pole IIR)
    attack_coeff: f32,
    release_coeff: f32,

    // RMS estimation: running sum of squares over a short window
    rms_window: Vec<f32>,
    rms_write_pos: usize,
    rms_sum: f32,

    // Pre-emphasis sidechain filter (L + R averaged into mono for measurement)
    pre_emphasis_filter: Option<DirectForm2Transposed<f32>>,
}

impl GatedAGC {
    /// Window size for RMS estimation: ~10 ms
    const RMS_WINDOW_MS: f32 = 10.0;

    pub fn new(sample_rate: f32, config: AgcConfig) -> Self {
        let window_samples = ((Self::RMS_WINDOW_MS / 1000.0) * sample_rate) as usize;
        let window_samples = window_samples.max(1);

        let attack_coeff = Self::time_to_coeff(config.attack_ms, sample_rate);
        let release_coeff = Self::time_to_coeff(config.release_ms, sample_rate);
        let pre_emphasis_filter = Self::build_pre_emphasis(sample_rate, config.pre_emphasis);
        let max_gain_linear = db_to_linear(config.max_gain_db);

        Self {
            sample_rate,
            config,
            current_gain: max_gain_linear,
            attack_coeff,
            release_coeff,
            rms_window: vec![0.0; window_samples],
            rms_write_pos: 0,
            rms_sum: 0.0,
            pre_emphasis_filter,
        }
    }

    pub fn with_defaults(sample_rate: f32) -> Self {
        Self::new(sample_rate, AgcConfig::default())
    }

    /// Reconfigure AGC parameters without resetting gain state.
    pub fn set_config(&mut self, config: AgcConfig) {
        let window_samples = ((Self::RMS_WINDOW_MS / 1000.0) * self.sample_rate) as usize;
        let window_samples = window_samples.max(1);
        if window_samples != self.rms_window.len() {
            self.rms_window = vec![0.0; window_samples];
            self.rms_sum = 0.0;
            self.rms_write_pos = 0;
        }
        self.attack_coeff = Self::time_to_coeff(config.attack_ms, self.sample_rate);
        self.release_coeff = Self::time_to_coeff(config.release_ms, self.sample_rate);
        self.pre_emphasis_filter = Self::build_pre_emphasis(self.sample_rate, config.pre_emphasis);
        self.config = config;
    }

    pub fn config(&self) -> &AgcConfig {
        &self.config
    }

    /// Process a single stereo frame (in-place).
    #[inline]
    pub fn process_stereo(&mut self, left: &mut f32, right: &mut f32) {
        if !self.config.enabled {
            return;
        }

        // Sidechain: mono mix for RMS measurement
        let mono = (*left + *right) * 0.5;
        let gain = self.compute_gain(mono);

        *left *= gain;
        *right *= gain;
    }

    /// Process an interleaved stereo buffer (L R L R …) in-place.
    pub fn process_buffer(&mut self, buf: &mut [f32]) {
        if !self.config.enabled {
            return;
        }
        for chunk in buf.chunks_exact_mut(2) {
            let (l, r) = chunk.split_at_mut(1);
            self.process_stereo(&mut l[0], &mut r[0]);
        }
    }

    /// Compute the gain for one sample, updating internal state.
    #[inline]
    fn compute_gain(&mut self, sidechain_mono: f32) -> f32 {
        // Apply pre-emphasis to sidechain copy (for measurement only)
        let measured = if let Some(ref mut f) = self.pre_emphasis_filter {
            f.run(sidechain_mono)
        } else {
            sidechain_mono
        };

        // Update rolling RMS
        let old_sq = self.rms_window[self.rms_write_pos];
        let new_sq = measured * measured;
        self.rms_sum = (self.rms_sum - old_sq + new_sq).max(0.0);
        self.rms_window[self.rms_write_pos] = new_sq;
        self.rms_write_pos = (self.rms_write_pos + 1) % self.rms_window.len();

        let rms = (self.rms_sum / self.rms_window.len() as f32).sqrt();
        let rms_db = linear_to_db(rms.max(1e-10));

        // Noise gate: if below gate threshold, hold current gain (don't pump on silence)
        if rms_db < self.config.gate_db {
            return self.current_gain;
        }

        // Desired gain to reach target level
        let desired_db = self.config.target_db - rms_db;
        let desired_db_clamped = desired_db.min(self.config.max_gain_db);
        let desired_gain = db_to_linear(desired_db_clamped);

        // Attack/release smoothing (one-pole IIR)
        let coeff = if desired_gain < self.current_gain {
            self.attack_coeff   // gain needs to decrease quickly
        } else {
            self.release_coeff  // gain recovers slowly
        };

        self.current_gain = coeff * self.current_gain + (1.0 - coeff) * desired_gain;
        self.current_gain
    }

    /// Current smoothed gain in dB (for metering/display)
    pub fn gain_db(&self) -> f32 {
        linear_to_db(self.current_gain)
    }

    /// One-pole IIR smoothing coefficient for a given time constant in ms
    fn time_to_coeff(time_ms: f32, sample_rate: f32) -> f32 {
        if time_ms <= 0.0 {
            return 0.0;
        }
        let time_samples = (time_ms / 1000.0) * sample_rate;
        // e^(-1/τ) where τ is in samples
        (-1.0_f32 / time_samples).exp()
    }

    fn build_pre_emphasis(
        sample_rate: f32,
        emphasis: PreEmphasis,
    ) -> Option<DirectForm2Transposed<f32>> {
        let corner = emphasis.corner_freq_hz()?;
        let corner_clamped = corner.clamp(20.0, sample_rate / 2.0 - 1.0);
        // High shelf +6 dB at corner — approximates a de-emphasis inverse
        Coefficients::<f32>::from_params(
            Type::HighShelf(6.0),
            sample_rate.hz(),
            corner_clamped.hz(),
            0.7071,
        )
        .ok()
        .map(DirectForm2Transposed::<f32>::new)
    }
}

// ── dB / linear helpers ────────────────────────────────────────────────────

#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    20.0 * linear.abs().max(1e-10).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_agc_is_passthrough() {
        let config = AgcConfig { enabled: false, ..Default::default() };
        let mut agc = GatedAGC::new(44100.0, config);
        let (mut l, mut r) = (0.5_f32, -0.3_f32);
        agc.process_stereo(&mut l, &mut r);
        assert!((l - 0.5).abs() < 1e-10);
        assert!((r - (-0.3)).abs() < 1e-10);
    }

    #[test]
    fn db_round_trip() {
        let db = -18.0_f32;
        assert!((linear_to_db(db_to_linear(db)) - db).abs() < 1e-4);
    }

    #[test]
    fn gate_holds_gain_on_silence() {
        let config = AgcConfig { enabled: true, gate_db: -20.0, ..Default::default() };
        let mut agc = GatedAGC::new(44100.0, config);
        let initial_gain = agc.current_gain;
        // Feed silence (below gate)
        for _ in 0..1000 {
            let mut l = 0.0_f32;
            let mut r = 0.0_f32;
            agc.process_stereo(&mut l, &mut r);
        }
        // Gain should not have changed (gate prevents pumping)
        assert!((agc.current_gain - initial_gain).abs() < 1e-4);
    }
}
