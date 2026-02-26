use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StemFilterMode {
    #[default]
    Off,
    Vocal,
    Instrumental,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StemFilterConfig {
    pub mode: StemFilterMode,
    pub amount: f32,
}

impl Default for StemFilterConfig {
    fn default() -> Self {
        Self {
            mode: StemFilterMode::Off,
            amount: 0.85,
        }
    }
}

/// Lightweight mid/side "stem-style" filter.
///
/// This is not AI stem separation, but in practice:
/// - `Vocal` mode emphasizes center (where vocals often sit).
/// - `Instrumental` mode attenuates center for karaoke-style playback.
pub struct StemFilter {
    cfg: StemFilterConfig,
}

impl StemFilter {
    pub fn new(cfg: StemFilterConfig) -> Self {
        Self { cfg }
    }

    pub fn config(&self) -> &StemFilterConfig {
        &self.cfg
    }

    pub fn set_config(&mut self, cfg: StemFilterConfig) {
        self.cfg = cfg;
    }

    #[inline]
    pub fn process_buffer(&mut self, buf: &mut [f32]) {
        if buf.len() < 2 {
            return;
        }

        let amount = self.cfg.amount.clamp(0.0, 1.0);
        if amount <= 0.0 || matches!(self.cfg.mode, StemFilterMode::Off) {
            return;
        }

        let (mid_gain, side_gain) = match self.cfg.mode {
            StemFilterMode::Off => (1.0, 1.0),
            StemFilterMode::Vocal => (1.0, 1.0 - amount * 0.92),
            StemFilterMode::Instrumental => (1.0 - amount * 0.92, 1.0),
        };

        for i in (0..buf.len()).step_by(2) {
            let l = buf[i];
            let r = buf[i + 1];
            let mid = 0.5 * (l + r);
            let side = 0.5 * (l - r);
            buf[i] = mid * mid_gain + side * side_gain;
            buf[i + 1] = mid * mid_gain - side * side_gain;
        }
    }
}
