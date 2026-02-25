/// `audio/dsp/reverb.rs` — Schroeder reverb for the Voice FX chain
///
/// Classic Schroeder design: 4 parallel comb filters → 2 allpass filters.
/// Based on public domain Schroeder/Moorer algorithms.
/// Suitable for small room/voice reverb effects.

/// Presets for quick selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoomPreset {
    Small,
    Medium,
    Large,
    Hall,
}

impl RoomPreset {
    pub fn to_params(self) -> ReverbParams {
        match self {
            RoomPreset::Small => ReverbParams {
                room_size: 0.3,
                damping: 0.7,
                wet: 0.12,
                dry: 0.88,
            },
            RoomPreset::Medium => ReverbParams {
                room_size: 0.55,
                damping: 0.55,
                wet: 0.22,
                dry: 0.78,
            },
            RoomPreset::Large => ReverbParams {
                room_size: 0.75,
                damping: 0.40,
                wet: 0.35,
                dry: 0.65,
            },
            RoomPreset::Hall => ReverbParams {
                room_size: 0.92,
                damping: 0.25,
                wet: 0.50,
                dry: 0.50,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReverbParams {
    /// 0.0 = small room, 1.0 = large hall
    pub room_size: f32,
    /// 0.0 = bright, 1.0 = dark (high-freq damping)
    pub damping: f32,
    /// Wet/dry mix (0.0 = dry, 1.0 = full wet)
    pub wet: f32,
    pub dry: f32,
}

/// A simple comb filter with feedback and damping.
struct CombFilter {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
    damp1: f32,
    damp2: f32,
    filter_store: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size],
            pos: 0,
            feedback: 0.5,
            damp1: 0.5,
            damp2: 0.5,
            filter_store: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let out = self.buf[self.pos];
        self.filter_store = out * self.damp2 + self.filter_store * self.damp1;
        self.buf[self.pos] = input + self.filter_store * self.feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }

    fn set_feedback(&mut self, v: f32) {
        self.feedback = v;
    }
    fn set_damp(&mut self, damp: f32) {
        self.damp1 = damp;
        self.damp2 = 1.0 - damp;
    }
}

/// Allpass filter.
struct AllpassFilter {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size],
            pos: 0,
            feedback: 0.5,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buf[self.pos];
        let v = input + buf_out * self.feedback;
        self.buf[self.pos] = v;
        self.pos = (self.pos + 1) % self.buf.len();
        buf_out - v
    }
}

/// Stereo Schroeder reverb.
pub struct Reverb {
    pub enabled: bool,
    pub params: ReverbParams,

    // L channel
    combs_l: [CombFilter; 4],
    allpasses_l: [AllpassFilter; 2],
    // R channel (slightly offset delay sizes for stereo spread)
    combs_r: [CombFilter; 4],
    allpasses_r: [AllpassFilter; 2],
}

// Delay lengths (in samples at 44100 Hz). Spread for stereo.
const COMB_TUNINGS_L: [usize; 4] = [1116, 1188, 1277, 1356];
const COMB_TUNINGS_R: [usize; 4] = [1124, 1196, 1285, 1364];
const AP_TUNINGS_L: [usize; 2] = [556, 441];
const AP_TUNINGS_R: [usize; 2] = [564, 449];

impl Reverb {
    pub fn new(_sample_rate: f32) -> Self {
        let mut r = Self {
            enabled: false,
            params: RoomPreset::Medium.to_params(),
            combs_l: [
                CombFilter::new(COMB_TUNINGS_L[0]),
                CombFilter::new(COMB_TUNINGS_L[1]),
                CombFilter::new(COMB_TUNINGS_L[2]),
                CombFilter::new(COMB_TUNINGS_L[3]),
            ],
            combs_r: [
                CombFilter::new(COMB_TUNINGS_R[0]),
                CombFilter::new(COMB_TUNINGS_R[1]),
                CombFilter::new(COMB_TUNINGS_R[2]),
                CombFilter::new(COMB_TUNINGS_R[3]),
            ],
            allpasses_l: [
                AllpassFilter::new(AP_TUNINGS_L[0]),
                AllpassFilter::new(AP_TUNINGS_L[1]),
            ],
            allpasses_r: [
                AllpassFilter::new(AP_TUNINGS_R[0]),
                AllpassFilter::new(AP_TUNINGS_R[1]),
            ],
        };
        r.apply_params();
        r
    }

    pub fn set_preset(&mut self, preset: RoomPreset) {
        self.params = preset.to_params();
        self.apply_params();
    }

    pub fn set_params(&mut self, params: ReverbParams) {
        self.params = params;
        self.apply_params();
    }

    fn apply_params(&mut self) {
        let fb = self.params.room_size * 0.28 + 0.7; // 0.7–0.98
        let damp = self.params.damping;
        for c in &mut self.combs_l {
            c.set_feedback(fb);
            c.set_damp(damp);
        }
        for c in &mut self.combs_r {
            c.set_feedback(fb);
            c.set_damp(damp);
        }
    }

    /// Process a stereo frame [L, R] in place.
    pub fn process(&mut self, frame: &mut [f32]) {
        if !self.enabled || frame.len() < 2 {
            return;
        }

        let input = (frame[0] + frame[1]) * 0.015; // scale down before reverb

        // Parallel combs
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for c in &mut self.combs_l {
            out_l += c.process(input);
        }
        for c in &mut self.combs_r {
            out_r += c.process(input);
        }

        // Series allpasses
        for ap in &mut self.allpasses_l {
            out_l = ap.process(out_l);
        }
        for ap in &mut self.allpasses_r {
            out_r = ap.process(out_r);
        }

        frame[0] = frame[0] * self.params.dry + out_l * self.params.wet;
        frame[1] = frame[1] * self.params.dry + out_r * self.params.wet;
    }
}
