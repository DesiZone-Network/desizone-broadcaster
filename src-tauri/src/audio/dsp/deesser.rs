/// `audio/dsp/deesser.rs` — De-esser for the Voice FX chain
///
/// Detects harsh sibilance (6–10 kHz) via a band-pass sidechain,
/// then attenuates that frequency range when the level exceeds a threshold.
/// Uses biquad band-pass + gain reduction.

use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Q_BUTTERWORTH_F32};

#[derive(Debug, Clone)]
pub struct Deesser {
    pub enabled: bool,
    /// Centre frequency of the sibilance band (Hz) — default 7500 Hz
    pub frequency_hz: f32,
    /// Threshold in dBFS above which reduction kicks in — default -12 dB
    pub threshold_db: f32,
    /// Gain reduction applied when over threshold — default 6 dB
    pub ratio: f32,
    /// Frequency range width around centre (Hz) — default 3000 Hz (6–9 kHz)
    pub range_hz: f32,

    // Internal
    sample_rate: f32,
    detector_l: DirectForm1<f32>,
    detector_r: DirectForm1<f32>,
    envelope_l: f32,
    envelope_r: f32,
}

impl Deesser {
    pub fn new(sample_rate: f32) -> Self {
        let (dl, dr) = Self::make_filters(sample_rate, 7500.0, 3000.0);
        Self {
            enabled: false,
            frequency_hz: 7500.0,
            threshold_db: -12.0,
            ratio: 6.0,
            range_hz: 3000.0,
            sample_rate,
            detector_l: dl,
            detector_r: dr,
            envelope_l: 0.0,
            envelope_r: 0.0,
        }
    }

    pub fn update_params(&mut self) {
        let (dl, dr) = Self::make_filters(self.sample_rate, self.frequency_hz, self.range_hz);
        self.detector_l = dl;
        self.detector_r = dr;
    }

    fn make_filters(sr: f32, freq: f32, _range: f32) -> (DirectForm1<f32>, DirectForm1<f32>) {
        let q = Q_BUTTERWORTH_F32;
        let coeffs = Coefficients::<f32>::from_params(
            biquad::Type::BandPass,
            sr.hz(),
            freq.hz(),
            q,
        ).unwrap_or_else(|_| Coefficients::<f32>::from_params(
            biquad::Type::BandPass,
            44100.0.hz(),
            7500.0.hz(),
            q,
        ).unwrap());
        (DirectForm1::<f32>::new(coeffs.clone()), DirectForm1::<f32>::new(coeffs))
    }

    /// Process a stereo frame [L, R].
    pub fn process(&mut self, frame: &mut [f32]) {
        if !self.enabled || frame.len() < 2 {
            return;
        }

        let threshold_lin = db_to_linear(self.threshold_db);
        let attack = 0.001f32;  // fast (per-sample smoothing coefficient)
        let release = 0.9999f32; // slow release

        // Sidechain: detect band energy
        let sc_l = self.detector_l.run(frame[0]).abs();
        let sc_r = self.detector_r.run(frame[1]).abs();

        // Envelope follower
        self.envelope_l = if sc_l > self.envelope_l {
            attack * sc_l + (1.0 - attack) * self.envelope_l
        } else {
            release * self.envelope_l
        };
        self.envelope_r = if sc_r > self.envelope_r {
            attack * sc_r + (1.0 - attack) * self.envelope_r
        } else {
            release * self.envelope_r
        };

        // Gain reduction
        let over_l = (self.envelope_l / threshold_lin.max(1e-10)).max(1.0);
        let over_r = (self.envelope_r / threshold_lin.max(1e-10)).max(1.0);
        let db_red_l = (over_l.log10() * 20.0).min(self.ratio);
        let db_red_r = (over_r.log10() * 20.0).min(self.ratio);

        frame[0] *= db_to_linear(-db_red_l);
        frame[1] *= db_to_linear(-db_red_r);
    }
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}
