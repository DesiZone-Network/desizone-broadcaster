use serde::{Deserialize, Serialize};

use super::crossfade::DeckId;

/// Per-channel gain/mute settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStrip {
    /// Fader gain 0.0 – 1.0 (linear; corresponds to 0 dB at 1.0)
    pub fader: f32,
    pub muted: bool,
    /// Pre-fader level for VU metering (computed each callback)
    pub vu_left_db: f32,
    pub vu_right_db: f32,
}

impl Default for ChannelStrip {
    fn default() -> Self {
        Self {
            fader: 1.0,
            muted: false,
            vu_left_db: -96.0,
            vu_right_db: -96.0,
        }
    }
}

/// 6-channel mixer: Deck A, Deck B, Sound FX, Aux 1, Aux 2, Voice FX → stereo master bus
///
/// All buffers are interleaved stereo f32 (L R L R …).
pub struct Mixer {
    pub deck_a: ChannelStrip,
    pub deck_b: ChannelStrip,
    pub sound_fx: ChannelStrip,
    pub aux1: ChannelStrip,
    pub aux2: ChannelStrip,
    pub voice_fx: ChannelStrip,
    pub master_gain: f32,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            deck_a: ChannelStrip::default(),
            deck_b: ChannelStrip::default(),
            sound_fx: ChannelStrip::default(),
            aux1: ChannelStrip::default(),
            aux2: ChannelStrip::default(),
            voice_fx: ChannelStrip::default(),
            master_gain: 1.0,
        }
    }
}

impl Mixer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn channel_mut(&mut self, id: DeckId) -> &mut ChannelStrip {
        match id {
            DeckId::DeckA => &mut self.deck_a,
            DeckId::DeckB => &mut self.deck_b,
            DeckId::SoundFx => &mut self.sound_fx,
            DeckId::Aux1 => &mut self.aux1,
            DeckId::Aux2 => &mut self.aux2,
            DeckId::VoiceFx => &mut self.voice_fx,
        }
    }

    pub fn channel(&self, id: DeckId) -> &ChannelStrip {
        match id {
            DeckId::DeckA => &self.deck_a,
            DeckId::DeckB => &self.deck_b,
            DeckId::SoundFx => &self.sound_fx,
            DeckId::Aux1 => &self.aux1,
            DeckId::Aux2 => &self.aux2,
            DeckId::VoiceFx => &self.voice_fx,
        }
    }

    /// Sum six channel buffers into `master_buf` (in-place add with gain scaling).
    ///
    /// Each channel buffer must be the same length as `master_buf` and is
    /// interleaved stereo (L R L R …).
    ///
    /// Also updates VU meter readings on each `ChannelStrip`.
    ///
    /// **Called on the real-time audio thread — no allocations.**
    pub fn mix_into(
        &mut self,
        master_buf: &mut [f32],
        ch_deck_a: &[f32],
        ch_deck_b: &[f32],
        ch_sound_fx: &[f32],
        ch_aux1: &[f32],
        ch_aux2: &[f32],
        ch_voice_fx: &[f32],
    ) {
        debug_assert_eq!(master_buf.len(), ch_deck_a.len());
        debug_assert_eq!(master_buf.len(), ch_deck_b.len());

        master_buf.fill(0.0);

        Self::accumulate(master_buf, ch_deck_a, &mut self.deck_a);
        Self::accumulate(master_buf, ch_deck_b, &mut self.deck_b);
        Self::accumulate(master_buf, ch_sound_fx, &mut self.sound_fx);
        Self::accumulate(master_buf, ch_aux1, &mut self.aux1);
        Self::accumulate(master_buf, ch_aux2, &mut self.aux2);
        Self::accumulate(master_buf, ch_voice_fx, &mut self.voice_fx);

        // Apply master gain
        if (self.master_gain - 1.0).abs() > 1e-6 {
            for s in master_buf.iter_mut() {
                *s *= self.master_gain;
            }
        }
    }

    /// Apply channel gain + mute, accumulate into `dest`, update VU readings.
    #[inline]
    fn accumulate(dest: &mut [f32], src: &[f32], ch: &mut ChannelStrip) {
        if ch.muted {
            ch.vu_left_db = -96.0;
            ch.vu_right_db = -96.0;
            return;
        }

        let gain = ch.fader;
        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;

        for (i, (&s, d)) in src.iter().zip(dest.iter_mut()).enumerate() {
            let scaled = s * gain;
            *d += scaled;
            if i % 2 == 0 {
                peak_l = peak_l.max(scaled.abs());
            } else {
                peak_r = peak_r.max(scaled.abs());
            }
        }

        ch.vu_left_db = linear_to_db(peak_l);
        ch.vu_right_db = linear_to_db(peak_r);
    }
}

#[inline]
fn linear_to_db(linear: f32) -> f32 {
    if linear < 1e-10 {
        -96.0
    } else {
        20.0 * linear.log10()
    }
}
