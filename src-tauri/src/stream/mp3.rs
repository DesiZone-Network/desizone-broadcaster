use shine_rs::{Mp3EncoderConfig, StereoMode, SUPPORTED_BITRATES, SUPPORTED_SAMPLE_RATES};

use super::encoder_manager::EncoderConfig;

pub struct Mp3Encoder {
    encoder: shine_rs::Mp3Encoder,
    frame_samples: usize,
    pcm_i16: Vec<i16>,
    mp3_out: Vec<u8>,
}

// Encoder state is owned by a single streaming task and never aliased across
// threads. We only need move semantics for Tokio task scheduling.
unsafe impl Send for Mp3Encoder {}

impl Mp3Encoder {
    pub fn from_config(config: &EncoderConfig) -> Result<Self, String> {
        let channels = config.channels.clamp(1, 2);
        let sample_rate = nearest_u32(config.sample_rate, SUPPORTED_SAMPLE_RATES);
        let bitrate = nearest_u32(config.bitrate_kbps.unwrap_or(128), SUPPORTED_BITRATES);
        let stereo_mode = if channels == 1 {
            StereoMode::Mono
        } else {
            StereoMode::Stereo
        };

        let enc_cfg = Mp3EncoderConfig::new()
            .sample_rate(sample_rate)
            .bitrate(bitrate)
            .channels(channels)
            .stereo_mode(stereo_mode);

        let encoder = shine_rs::Mp3Encoder::new(enc_cfg)
            .map_err(|e| format!("MP3 encoder init failed: {e}"))?;
        let frame_samples = encoder.samples_per_frame();

        log::info!(
            "MP3 encoder initialised: channels={} sample_rate={} bitrate={} frame_samples={}",
            channels,
            sample_rate,
            bitrate,
            frame_samples
        );

        Ok(Self {
            encoder,
            frame_samples,
            pcm_i16: Vec::new(),
            mp3_out: Vec::new(),
        })
    }

    pub fn frame_samples(&self) -> usize {
        self.frame_samples
    }

    pub fn encode_f32_interleaved(&mut self, input: &[f32]) -> Result<&[u8], String> {
        self.pcm_i16.clear();
        self.pcm_i16.reserve(input.len());
        for &s in input {
            // Keep samples within [-32767, 32767] to avoid i16::MIN edge-case
            // overflows in downstream abs/neg operations.
            let scaled = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i32;
            let s16 = scaled.clamp(-(i16::MAX as i32), i16::MAX as i32) as i16;
            self.pcm_i16.push(s16);
        }

        self.mp3_out.clear();
        let frames = self
            .encoder
            .encode_interleaved(&self.pcm_i16)
            .map_err(|e| format!("MP3 encode failed: {e}"))?;
        for frame in frames {
            self.mp3_out.extend_from_slice(&frame);
        }
        Ok(self.mp3_out.as_slice())
    }

    pub fn flush(&mut self) -> Result<&[u8], String> {
        let tail = self
            .encoder
            .finish()
            .map_err(|e| format!("MP3 flush failed: {e}"))?;
        self.mp3_out.clear();
        self.mp3_out.extend_from_slice(&tail);
        Ok(self.mp3_out.as_slice())
    }
}

fn nearest_u32(value: u32, supported: &[u32]) -> u32 {
    supported
        .iter()
        .copied()
        .min_by_key(|candidate| value.abs_diff(*candidate))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::nearest_u32;

    #[test]
    fn nearest_value_works() {
        assert_eq!(nearest_u32(127, &[96, 128, 160]), 128);
        assert_eq!(nearest_u32(11026, &[8000, 11025, 12000]), 11025);
    }
}
