use std::{
    path::PathBuf,
    sync::atomic::Ordering,
};

use ringbuf::traits::Observer as _;

use serde::{Deserialize, Serialize};

use super::{
    crossfade::DeckId,
    decoder::{spawn_decoder, DecoderHandle},
};

/// Deck playback states — exposed to the frontend via IPC events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckState {
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Crossfading,
    Stopped,
}

/// Per-deck playback control and state.
///
/// The deck does not own an audio output thread. Instead, the `AudioEngine`'s
/// CPAL callback calls `fill_buffer()` each time it needs audio.
pub struct Deck {
    pub id: DeckId,
    pub state: DeckState,

    // Active decoder (None when Idle/Stopped)
    decoder: Option<DecoderHandle>,

    // Current track info
    pub file_path: Option<PathBuf>,
    pub song_id: Option<i64>,
    pub sample_rate: u32,

    // Frame-accurate position tracking
    /// Total frames consumed by the render thread
    pub frames_consumed: u64,
    /// Gain applied during mix (0.0 – 1.0, modified during crossfade)
    pub gain: f32,

    // Pause state: when paused we stop pulling from the ring buffer
    paused: bool,
}

impl Deck {
    pub fn new(id: DeckId) -> Self {
        Self {
            id,
            state: DeckState::Idle,
            decoder: None,
            file_path: None,
            song_id: None,
            sample_rate: 44100,
            frames_consumed: 0,
            gain: 1.0,
            paused: false,
        }
    }

    /// Load a new track. Stops any existing playback.
    pub fn load(&mut self, path: PathBuf, song_id: Option<i64>) -> Result<(), String> {
        self.stop_decoder();
        self.state = DeckState::Loading;
        self.file_path = Some(path.clone());
        self.song_id = song_id;
        self.frames_consumed = 0;

        let handle = spawn_decoder(path, None)?;
        self.sample_rate = handle.sample_rate;
        self.decoder = Some(handle);
        self.state = DeckState::Ready;
        Ok(())
    }

    /// Seek to a position (stops current decoder and spawns a new one at the target).
    pub fn seek(&mut self, position_ms: u64) -> Result<(), String> {
        let path = self.file_path.clone().ok_or("No track loaded")?;
        self.stop_decoder();
        self.frames_consumed = (position_ms * self.sample_rate as u64) / 1000;

        let handle = spawn_decoder(path, Some(position_ms))?;
        self.sample_rate = handle.sample_rate;
        self.decoder = Some(handle);

        if self.state == DeckState::Playing || self.state == DeckState::Crossfading {
            // Keep playing state — the render thread will pick up the new ring buffer
        } else {
            self.state = DeckState::Ready;
        }
        Ok(())
    }

    pub fn play(&mut self) {
        if self.state == DeckState::Ready || self.state == DeckState::Paused {
            self.paused = false;
            self.state = DeckState::Playing;
        }
    }

    pub fn pause(&mut self) {
        if self.state == DeckState::Playing {
            self.paused = true;
            self.state = DeckState::Paused;
        }
    }

    pub fn stop(&mut self) {
        self.stop_decoder();
        self.state = DeckState::Stopped;
        self.frames_consumed = 0;
        self.paused = false;
    }

    pub fn set_crossfading(&mut self) {
        if self.state == DeckState::Playing {
            self.state = DeckState::Crossfading;
        }
    }

    /// Current position in ms based on frames consumed
    pub fn position_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        self.frames_consumed * 1000 / self.sample_rate as u64
    }

    /// Total duration in ms (0 if unknown)
    pub fn duration_ms(&self) -> u64 {
        self.decoder.as_ref().map(|d| d.duration_ms()).unwrap_or(0)
    }

    /// How many frames remain (approximately)
    pub fn remaining_frames(&self) -> u64 {
        let total = self.decoder.as_ref().map(|d| d.total_frames.load(Ordering::Relaxed)).unwrap_or(0);
        if total > self.frames_consumed { total - self.frames_consumed } else { 0 }
    }

    /// Remaining time in ms
    pub fn remaining_ms(&self) -> u64 {
        if self.sample_rate == 0 { return 0; }
        self.remaining_frames() * 1000 / self.sample_rate as u64
    }

    /// Whether the decoder ring buffer is exhausted and the track has ended
    pub fn is_eof(&self) -> bool {
        match &self.decoder {
            Some(d) => {
                // EOF when decoder has written all frames and ring buffer is empty
                let written = d.frames_written.load(Ordering::Relaxed);
                let total = d.total_frames.load(Ordering::Relaxed);
                total > 0 && written >= total && d.consumer.is_empty()
            }
            None => true,
        }
    }

    /// Fill `output` with interleaved stereo f32 samples, scaled by `self.gain`.
    /// Zeros are written for any frames the ring buffer cannot supply (underrun).
    ///
    /// Called on the real-time audio thread — **no allocations, no locks**.
    pub fn fill_buffer(&mut self, output: &mut [f32]) {
        if self.paused || self.state == DeckState::Idle || self.state == DeckState::Stopped {
            output.fill(0.0);
            return;
        }

        let decoder = match &mut self.decoder {
            Some(d) => d,
            None => {
                output.fill(0.0);
                return;
            }
        };

        let frames = output.len() / 2;
        let mut i = 0;

        use ringbuf::traits::Consumer as _;
        while i < output.len() {
            match decoder.consumer.try_pop() {
                Some(s) => {
                    output[i] = s * self.gain;
                    i += 1;
                }
                None => {
                    // Ring buffer underrun — fill rest with silence
                    output[i..].fill(0.0);
                    break;
                }
            }
        }

        self.frames_consumed += frames as u64;
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn stop_decoder(&mut self) {
        if let Some(d) = self.decoder.take() {
            d.stop_flag.store(true, Ordering::Relaxed);
            // Thread will exit on its own after seeing stop_flag
        }
    }
}

impl Drop for Deck {
    fn drop(&mut self) {
        self.stop_decoder();
    }
}
