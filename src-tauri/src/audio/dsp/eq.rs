use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type, Q_BUTTERWORTH_F32};
use serde::{Deserialize, Serialize};

/// 3-band parametric EQ: low shelf → peaking mid → high shelf
/// One instance per audio channel.
pub struct ChannelEQ {
    sample_rate: f32,
    low_shelf: DirectForm2Transposed<f32>,
    peak_mid: DirectForm2Transposed<f32>,
    high_shelf: DirectForm2Transposed<f32>,
    // Keep config for re-init on parameter change
    config: EqConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqConfig {
    /// Low shelf gain in dB (positive = boost, negative = cut)
    pub low_gain_db: f32,
    /// Low shelf corner frequency in Hz
    pub low_freq_hz: f32,

    /// Peak/notch mid gain in dB
    pub mid_gain_db: f32,
    /// Mid centre frequency in Hz
    pub mid_freq_hz: f32,
    /// Mid Q factor (bandwidth). 0.707 = one octave, higher = narrower
    pub mid_q: f32,

    /// High shelf gain in dB
    pub high_gain_db: f32,
    /// High shelf corner frequency in Hz
    pub high_freq_hz: f32,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            low_gain_db: 0.0,
            low_freq_hz: 100.0,
            mid_gain_db: 0.0,
            mid_freq_hz: 1000.0,
            mid_q: Q_BUTTERWORTH_F32,
            high_gain_db: 0.0,
            high_freq_hz: 8000.0,
        }
    }
}

impl ChannelEQ {
    pub fn new(sample_rate: f32, config: EqConfig) -> Self {
        let (low_shelf, peak_mid, high_shelf) = Self::build_filters(sample_rate, &config);
        Self {
            sample_rate,
            low_shelf,
            peak_mid,
            high_shelf,
            config,
        }
    }

    pub fn with_defaults(sample_rate: f32) -> Self {
        Self::new(sample_rate, EqConfig::default())
    }

    /// Update EQ parameters. Rebuilds biquad coefficients.
    pub fn set_config(&mut self, config: EqConfig) {
        let (low_shelf, peak_mid, high_shelf) = Self::build_filters(self.sample_rate, &config);
        self.low_shelf = low_shelf;
        self.peak_mid = peak_mid;
        self.high_shelf = high_shelf;
        self.config = config;
    }

    pub fn config(&self) -> &EqConfig {
        &self.config
    }

    /// Process a single stereo frame (in-place). Call once per audio sample.
    #[inline]
    pub fn process_stereo(&mut self, left: &mut f32, right: &mut f32) {
        *left = self.process_mono(*left);
        *right = self.process_mono(*right);
    }

    /// Process a mono sample.
    #[inline]
    pub fn process_mono(&mut self, sample: f32) -> f32 {
        let s = self.low_shelf.run(sample);
        let s = self.peak_mid.run(s);
        self.high_shelf.run(s)
    }

    /// Process an interleaved stereo buffer (L R L R …) in-place.
    pub fn process_buffer(&mut self, buf: &mut [f32]) {
        for chunk in buf.chunks_exact_mut(2) {
            let (l, r) = chunk.split_at_mut(1);
            self.process_stereo(&mut l[0], &mut r[0]);
        }
    }

    fn build_filters(
        sample_rate: f32,
        cfg: &EqConfig,
    ) -> (
        DirectForm2Transposed<f32>,
        DirectForm2Transposed<f32>,
        DirectForm2Transposed<f32>,
    ) {
        let fs = sample_rate.hz();

        let low_coeffs = Coefficients::<f32>::from_params(
            Type::LowShelf(cfg.low_gain_db),
            fs,
            cfg.low_freq_hz.clamp(20.0, sample_rate / 2.0 - 1.0).hz(),
            Q_BUTTERWORTH_F32,
        )
        .unwrap_or_else(|_| Self::unity_coeffs());

        let mid_coeffs = Coefficients::<f32>::from_params(
            Type::PeakingEQ(cfg.mid_gain_db),
            fs,
            cfg.mid_freq_hz.clamp(20.0, sample_rate / 2.0 - 1.0).hz(),
            cfg.mid_q.max(0.1),
        )
        .unwrap_or_else(|_| Self::unity_coeffs());

        let high_coeffs = Coefficients::<f32>::from_params(
            Type::HighShelf(cfg.high_gain_db),
            fs,
            cfg.high_freq_hz.clamp(20.0, sample_rate / 2.0 - 1.0).hz(),
            Q_BUTTERWORTH_F32,
        )
        .unwrap_or_else(|_| Self::unity_coeffs());

        (
            DirectForm2Transposed::<f32>::new(low_coeffs),
            DirectForm2Transposed::<f32>::new(mid_coeffs),
            DirectForm2Transposed::<f32>::new(high_coeffs),
        )
    }

    /// Coefficients that pass audio unmodified (gain = 0 dB, all zeros)
    fn unity_coeffs() -> Coefficients<f32> {
        // b0=1, b1=0, b2=0, a1=0, a2=0
        Coefficients {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_pass_through() {
        let mut eq = ChannelEQ::with_defaults(44100.0);
        // With all gains at 0 dB the signal should be unchanged (within float precision)
        let input = 0.5_f32;
        let output = eq.process_mono(input);
        assert!(
            (output - input).abs() < 1e-4,
            "unity EQ should not alter signal: {output}"
        );
    }

    #[test]
    fn boost_increases_level() {
        let config = EqConfig {
            low_gain_db: 6.0,
            ..Default::default()
        };
        let mut eq = ChannelEQ::new(44100.0, config);
        // Feed a DC offset — low shelf at DC should boost it
        let output = eq.process_mono(0.5);
        assert!(output > 0.5, "6 dB boost should increase level");
    }
}
