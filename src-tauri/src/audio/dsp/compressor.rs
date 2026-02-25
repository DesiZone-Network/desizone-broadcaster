use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type};
use serde::{Deserialize, Serialize};

use super::agc::{db_to_linear, linear_to_db};

// ── Single-band compressor ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandConfig {
    /// Compression threshold in dBFS
    pub threshold_db: f32,
    /// Compression ratio (e.g. 4.0 = 4:1). Values ≥ 20 approach limiting.
    pub ratio: f32,
    /// Knee width in dB (0 = hard knee)
    pub knee_db: f32,
    /// Attack time in ms
    pub attack_ms: f32,
    /// Release time in ms
    pub release_ms: f32,
    /// Make-up gain in dB
    pub makeup_db: f32,
}

impl Default for BandConfig {
    fn default() -> Self {
        Self {
            threshold_db: -18.0,
            ratio: 3.0,
            knee_db: 6.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            makeup_db: 0.0,
        }
    }
}

/// Single-band peak compressor with soft knee.
struct Band {
    config: BandConfig,
    /// Smoothed detector level (linear)
    detector: f32,
    attack_coeff: f32,
    release_coeff: f32,
    makeup_gain: f32,
}

impl Band {
    fn new(sample_rate: f32, config: BandConfig) -> Self {
        let attack_coeff = time_coeff(config.attack_ms, sample_rate);
        let release_coeff = time_coeff(config.release_ms, sample_rate);
        let makeup_gain = db_to_linear(config.makeup_db);
        Self {
            config,
            detector: 0.0,
            attack_coeff,
            release_coeff,
            makeup_gain,
        }
    }

    fn reconfigure(&mut self, sample_rate: f32, config: BandConfig) {
        self.attack_coeff = time_coeff(config.attack_ms, sample_rate);
        self.release_coeff = time_coeff(config.release_ms, sample_rate);
        self.makeup_gain = db_to_linear(config.makeup_db);
        self.config = config;
    }

    #[inline]
    fn process(&mut self, sample: f32) -> f32 {
        let abs_in = sample.abs();

        let coeff = if abs_in > self.detector {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.detector = coeff * self.detector + (1.0 - coeff) * abs_in;

        let level_db = linear_to_db(self.detector.max(1e-10));
        let gain_db = self.compute_gain_db(level_db);
        sample * db_to_linear(gain_db) * self.makeup_gain
    }

    #[inline]
    fn compute_gain_db(&self, level_db: f32) -> f32 {
        let t = self.config.threshold_db;
        let r = self.config.ratio;
        let w = self.config.knee_db;
        let excess = level_db - t;

        if w > 0.0 {
            let half_w = w / 2.0;
            if excess < -half_w {
                0.0
            } else if excess > half_w {
                (t + excess / r) - level_db
            } else {
                let x = (excess + half_w) / w;
                let interp_ratio = 1.0 + (r - 1.0) * x;
                (t - half_w + (excess + half_w) / interp_ratio) - level_db
            }
        } else {
            if excess > 0.0 {
                (t + excess / r) - level_db
            } else {
                0.0
            }
        }
    }
}

// ── 5-band multiband compressor ────────────────────────────────────────────

const CROSSOVERS_HZ: [f32; 4] = [100.0, 400.0, 2500.0, 8000.0];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultibandConfig {
    pub enabled: bool,
    pub bands: [BandConfig; 5],
}

impl Default for MultibandConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: [
                BandConfig {
                    threshold_db: -20.0,
                    ratio: 2.0,
                    ..Default::default()
                },
                BandConfig {
                    threshold_db: -20.0,
                    ratio: 2.5,
                    ..Default::default()
                },
                BandConfig {
                    threshold_db: -20.0,
                    ratio: 3.0,
                    ..Default::default()
                },
                BandConfig {
                    threshold_db: -20.0,
                    ratio: 3.0,
                    ..Default::default()
                },
                BandConfig {
                    threshold_db: -20.0,
                    ratio: 2.0,
                    ..Default::default()
                },
            ],
        }
    }
}

/// Linkwitz-Riley 4th-order crossover filter pair
struct CrossoverPair {
    lp1: DirectForm2Transposed<f32>,
    lp2: DirectForm2Transposed<f32>,
    hp1: DirectForm2Transposed<f32>,
    hp2: DirectForm2Transposed<f32>,
}

impl CrossoverPair {
    fn new(sample_rate: f32, crossover_hz: f32) -> Self {
        let freq = crossover_hz.clamp(20.0, sample_rate / 2.0 - 1.0).hz();
        let fs = sample_rate.hz();
        let q = 0.7071_f32;

        let lp_c = Coefficients::<f32>::from_params(Type::LowPass, fs, freq, q)
            .unwrap_or_else(|_| unity_coeffs());
        let hp_c = Coefficients::<f32>::from_params(Type::HighPass, fs, freq, q)
            .unwrap_or_else(|_| unity_coeffs());

        Self {
            lp1: DirectForm2Transposed::<f32>::new(lp_c),
            lp2: DirectForm2Transposed::<f32>::new(lp_c),
            hp1: DirectForm2Transposed::<f32>::new(hp_c),
            hp2: DirectForm2Transposed::<f32>::new(hp_c),
        }
    }

    #[inline]
    fn split(&mut self, x: f32) -> (f32, f32) {
        let lp = self.lp2.run(self.lp1.run(x));
        let hp = self.hp2.run(self.hp1.run(x));
        (lp, hp)
    }
}

pub struct MultibandCompressor {
    config: MultibandConfig,
    sample_rate: f32,
    crossovers: [CrossoverPair; 4],
    bands: [Band; 5],
}

impl MultibandCompressor {
    pub fn new(sample_rate: f32, config: MultibandConfig) -> Self {
        let crossovers = CROSSOVERS_HZ.map(|hz| CrossoverPair::new(sample_rate, hz));
        let bands = std::array::from_fn(|i| Band::new(sample_rate, config.bands[i].clone()));
        Self {
            config,
            sample_rate,
            crossovers,
            bands,
        }
    }

    pub fn with_defaults(sample_rate: f32) -> Self {
        Self::new(sample_rate, MultibandConfig::default())
    }

    pub fn set_config(&mut self, config: MultibandConfig) {
        for (i, band) in self.bands.iter_mut().enumerate() {
            band.reconfigure(self.sample_rate, config.bands[i].clone());
        }
        self.config = config;
    }

    pub fn config(&self) -> &MultibandConfig {
        &self.config
    }

    #[inline]
    pub fn process_mono(&mut self, x: f32) -> f32 {
        if !self.config.enabled {
            return x;
        }
        // Cascade-split into 5 bands
        let (b0, rest1) = self.crossovers[0].split(x);
        let (b1, rest2) = self.crossovers[1].split(rest1);
        let (b2, rest3) = self.crossovers[2].split(rest2);
        let (b3, b4) = self.crossovers[3].split(rest3);

        self.bands[0].process(b0)
            + self.bands[1].process(b1)
            + self.bands[2].process(b2)
            + self.bands[3].process(b3)
            + self.bands[4].process(b4)
    }

    pub fn process_buffer(&mut self, buf: &mut [f32]) {
        if !self.config.enabled {
            return;
        }
        for s in buf.iter_mut() {
            *s = self.process_mono(*s);
        }
    }
}

// ── Dual-band (LF / HF) compressor ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualBandConfig {
    pub enabled: bool,
    pub crossover_hz: f32,
    pub lf_band: BandConfig,
    pub hf_band: BandConfig,
}

impl Default for DualBandConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            crossover_hz: 800.0,
            lf_band: BandConfig {
                threshold_db: -18.0,
                ratio: 4.0,
                ..Default::default()
            },
            hf_band: BandConfig {
                threshold_db: -18.0,
                ratio: 3.0,
                ..Default::default()
            },
        }
    }
}

pub struct DualBandCompressor {
    config: DualBandConfig,
    sample_rate: f32,
    crossover: CrossoverPair,
    lf: Band,
    hf: Band,
}

impl DualBandCompressor {
    pub fn new(sample_rate: f32, config: DualBandConfig) -> Self {
        let crossover = CrossoverPair::new(sample_rate, config.crossover_hz);
        let lf = Band::new(sample_rate, config.lf_band.clone());
        let hf = Band::new(sample_rate, config.hf_band.clone());
        Self {
            config,
            sample_rate,
            crossover,
            lf,
            hf,
        }
    }

    pub fn with_defaults(sample_rate: f32) -> Self {
        Self::new(sample_rate, DualBandConfig::default())
    }

    pub fn set_config(&mut self, config: DualBandConfig) {
        self.crossover = CrossoverPair::new(self.sample_rate, config.crossover_hz);
        self.lf
            .reconfigure(self.sample_rate, config.lf_band.clone());
        self.hf
            .reconfigure(self.sample_rate, config.hf_band.clone());
        self.config = config;
    }

    pub fn config(&self) -> &DualBandConfig {
        &self.config
    }

    #[inline]
    pub fn process_mono(&mut self, x: f32) -> f32 {
        if !self.config.enabled {
            return x;
        }
        let (lo, hi) = self.crossover.split(x);
        self.lf.process(lo) + self.hf.process(hi)
    }

    pub fn process_buffer(&mut self, buf: &mut [f32]) {
        if !self.config.enabled {
            return;
        }
        for s in buf.iter_mut() {
            *s = self.process_mono(*s);
        }
    }
}

// ── Hard clipper ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipperConfig {
    pub enabled: bool,
    /// Clipping ceiling in dBFS (e.g. -0.1)
    pub ceiling_db: f32,
}

impl Default for ClipperConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ceiling_db: -0.1,
        }
    }
}

pub struct Clipper {
    config: ClipperConfig,
    ceiling_linear: f32,
}

impl Clipper {
    pub fn new(config: ClipperConfig) -> Self {
        let ceiling_linear = db_to_linear(config.ceiling_db);
        Self {
            config,
            ceiling_linear,
        }
    }

    pub fn set_config(&mut self, config: ClipperConfig) {
        self.ceiling_linear = db_to_linear(config.ceiling_db);
        self.config = config;
    }

    pub fn config(&self) -> &ClipperConfig {
        &self.config
    }

    #[inline]
    pub fn process(&self, sample: f32) -> f32 {
        if !self.config.enabled {
            return sample;
        }
        sample.clamp(-self.ceiling_linear, self.ceiling_linear)
    }

    pub fn process_buffer(&self, buf: &mut [f32]) {
        if !self.config.enabled {
            return;
        }
        for s in buf.iter_mut() {
            *s = self.process(*s);
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

#[inline]
fn time_coeff(time_ms: f32, sample_rate: f32) -> f32 {
    if time_ms <= 0.0 {
        return 0.0;
    }
    let samples = (time_ms / 1000.0) * sample_rate;
    (-1.0_f32 / samples.max(1.0)).exp()
}

fn unity_coeffs() -> Coefficients<f32> {
    Coefficients {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipper_hard_limits() {
        let clipper = Clipper::new(ClipperConfig {
            enabled: true,
            ceiling_db: -6.0,
        });
        let ceiling = db_to_linear(-6.0);
        assert!(clipper.process(2.0) <= ceiling + 1e-6);
        assert!(clipper.process(-2.0) >= -ceiling - 1e-6);
    }

    #[test]
    fn disabled_compressor_passthrough() {
        let config = MultibandConfig {
            enabled: false,
            ..Default::default()
        };
        let mut comp = MultibandCompressor::new(44100.0, config);
        let input = 0.42_f32;
        assert!((comp.process_mono(input) - input).abs() < 1e-6);
    }

    #[test]
    fn compressor_reduces_loud_signal() {
        let band_cfg = BandConfig {
            threshold_db: -20.0,
            ratio: 10.0,
            knee_db: 0.0,
            attack_ms: 0.1,
            release_ms: 10.0,
            makeup_db: 0.0,
        };
        let config = MultibandConfig {
            enabled: true,
            bands: std::array::from_fn(|_| band_cfg.clone()),
        };
        let mut comp = MultibandCompressor::new(44100.0, config);
        // Warm up detector
        for _ in 0..1000 {
            comp.process_mono(0.9);
        }
        let loud = comp.process_mono(0.9);
        assert!(
            loud.abs() < 0.9,
            "compressor should reduce loud signal: {loud}"
        );
    }
}
