use serde::{Deserialize, Serialize};

use super::{
    agc::{AgcConfig, GatedAGC},
    compressor::{Clipper, ClipperConfig, DualBandCompressor, DualBandConfig, MultibandCompressor, MultibandConfig},
    eq::{ChannelEQ, EqConfig},
};

/// Complete per-channel DSP chain: EQ → AGC → MultibandComp → DualBandComp → Clipper
///
/// This mirrors SAM Broadcaster's per-channel DSP pipeline:
/// Audio Settings → each channel → EQ → AGC → 5-band processor → Dual-band → Clipper
pub struct ChannelPipeline {
    pub eq: ChannelEQ,
    pub agc: GatedAGC,
    pub multiband: MultibandCompressor,
    pub dual_band: DualBandCompressor,
    pub clipper: Clipper,
}

/// Serializable settings snapshot — stored in SQLite `channel_dsp_settings`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineSettings {
    pub eq: EqConfig,
    pub agc: AgcConfig,
    pub multiband: MultibandConfig,
    pub dual_band: DualBandConfig,
    pub clipper: ClipperConfig,
}

impl ChannelPipeline {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            eq: ChannelEQ::with_defaults(sample_rate),
            agc: GatedAGC::with_defaults(sample_rate),
            multiband: MultibandCompressor::with_defaults(sample_rate),
            dual_band: DualBandCompressor::with_defaults(sample_rate),
            clipper: Clipper::new(ClipperConfig::default()),
        }
    }

    pub fn from_settings(sample_rate: f32, settings: PipelineSettings) -> Self {
        Self {
            eq: ChannelEQ::new(sample_rate, settings.eq),
            agc: GatedAGC::new(sample_rate, settings.agc),
            multiband: MultibandCompressor::new(sample_rate, settings.multiband),
            dual_band: DualBandCompressor::new(sample_rate, settings.dual_band),
            clipper: Clipper::new(settings.clipper),
        }
    }

    /// Snapshot current settings for persistence
    pub fn settings(&self) -> PipelineSettings {
        PipelineSettings {
            eq: self.eq.config().clone(),
            agc: self.agc.config().clone(),
            multiband: self.multiband.config().clone(),
            dual_band: self.dual_band.config().clone(),
            clipper: self.clipper.config().clone(),
        }
    }

    /// Process an interleaved stereo buffer (L R L R …) through the full chain in-place.
    ///
    /// This is called on the real-time audio thread — no allocations inside.
    #[inline]
    pub fn process(&mut self, buf: &mut [f32]) {
        // 1. 3-band parametric EQ
        self.eq.process_buffer(buf);

        // 2. Gated AGC
        self.agc.process_buffer(buf);

        // 3. 5-band multiband compressor
        self.multiband.process_buffer(buf);

        // 4. Dual-band (LF / HF) compressor
        self.dual_band.process_buffer(buf);

        // 5. Hard clipper (last-resort ceiling)
        self.clipper.process_buffer(buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_processes_without_panic() {
        let mut pipeline = ChannelPipeline::new(44100.0);
        let mut buf: Vec<f32> = (0..256).map(|i| (i as f32 / 128.0 - 1.0) * 0.5).collect();
        pipeline.process(&mut buf);
        // Just confirm it ran without NaN/infinity
        for s in &buf {
            assert!(s.is_finite(), "pipeline output contains non-finite value");
        }
    }
}
