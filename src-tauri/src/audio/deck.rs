use std::{path::PathBuf, sync::atomic::Ordering};

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
    pub queue_id: Option<i64>,
    pub from_rotation: bool,
    pub sample_rate: u32,
    /// Optional fallback duration from metadata (ms) when decoder can't probe total frames.
    pub declared_duration_ms: Option<u64>,

    // Frame-accurate position tracking
    /// Total frames consumed by the render thread
    pub frames_consumed: u64,
    /// Per-channel operator gain (volume fader).
    pub channel_gain: f32,
    /// Crossfade/manual-xfade gain multiplier.
    pub xfade_gain: f32,
    /// Linked transport controls for this phase.
    pub pitch_pct: f32,
    pub tempo_pct: f32,
    pub playback_rate: f32,
    /// Rolling RMS level (dBFS) before channel/crossfade gain scaling.
    pub rms_db_pre_fader: f32,

    // Pause state: when paused we stop pulling from the ring buffer
    paused: bool,
    ended_naturally: bool,
    completion_pending: Option<TrackCompletion>,

    // ── Resampler state ──────────────────────────────────────────────────
    // Used when the file's sample rate differs from the CPAL device rate.
    // Linear interpolation between two adjacent source frames.
    /// Fractional position within the current source-frame pair [0.0, 1.0)
    resample_phase: f64,
    resample_seeded: bool,
    resample_prev_l: f32,
    resample_prev_r: f32,
    resample_next_l: f32,
    resample_next_r: f32,
    // Short anti-click ramp when playback starts/resumes/seeks.
    play_ramp_armed: bool,
    play_ramp_ms: u64,
    play_ramp_total_frames: u32,
    play_ramp_remaining_frames: u32,
    swap_out_armed: bool,
    swap_out_total_frames: u32,
    swap_out_remaining_frames: u32,
    pending_swap: Option<PendingSwap>,
    loop_state: Option<LoopState>,
}

#[derive(Debug, Clone)]
pub struct TrackCompletion {
    pub song_id: i64,
    pub queue_id: Option<i64>,
    pub from_rotation: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum AttachOp {
    Load,
    Seek,
}

pub struct PreparedTrack {
    pub decoder: DecoderHandle,
    pub file_path: PathBuf,
    pub song_id: Option<i64>,
    pub queue_id: Option<i64>,
    pub from_rotation: bool,
    pub declared_duration_ms: Option<u64>,
    pub initial_frames_consumed: u64,
}

struct PendingSwap {
    prepared: PreparedTrack,
    op: AttachOp,
}

struct LoopState {
    start_frame: u64,
    end_frame: u64,
    cached_frames: u64,
    play_frame: u64,
    playing_from_buffer: bool,
    buffer: Vec<f32>,
}

const SWAP_OUT_MS: u64 = 10;
const SWAP_PREROLL_MS: u64 = 20;
const MAX_LOOP_SECONDS: u64 = 64;
const LOOP_WRAP_MIN_XFADE_FRAMES: u64 = 24;
const LOOP_WRAP_MAX_XFADE_FRAMES: u64 = 160;

impl Deck {
    pub fn new(id: DeckId) -> Self {
        Self {
            id,
            state: DeckState::Idle,
            decoder: None,
            file_path: None,
            song_id: None,
            queue_id: None,
            from_rotation: false,
            sample_rate: 44100,
            declared_duration_ms: None,
            frames_consumed: 0,
            channel_gain: 1.0,
            xfade_gain: 1.0,
            pitch_pct: 0.0,
            tempo_pct: 0.0,
            playback_rate: 1.0,
            rms_db_pre_fader: -96.0,
            paused: false,
            ended_naturally: false,
            completion_pending: None,
            resample_phase: 0.0,
            resample_seeded: false,
            resample_prev_l: 0.0,
            resample_prev_r: 0.0,
            resample_next_l: 0.0,
            resample_next_r: 0.0,
            play_ramp_armed: false,
            play_ramp_ms: 8,
            play_ramp_total_frames: 0,
            play_ramp_remaining_frames: 0,
            swap_out_armed: false,
            swap_out_total_frames: 0,
            swap_out_remaining_frames: 0,
            pending_swap: None,
            loop_state: None,
        }
    }

    pub fn prepare_load(
        path: PathBuf,
        song_id: Option<i64>,
        queue_id: Option<i64>,
        from_rotation: bool,
        declared_duration_ms: Option<u64>,
    ) -> Result<PreparedTrack, String> {
        let decoder = spawn_decoder(path.clone(), None)?;
        Ok(PreparedTrack {
            decoder,
            file_path: path,
            song_id,
            queue_id,
            from_rotation,
            declared_duration_ms,
            initial_frames_consumed: 0,
        })
    }

    pub fn prepare_seek(
        path: PathBuf,
        song_id: Option<i64>,
        queue_id: Option<i64>,
        from_rotation: bool,
        declared_duration_ms: Option<u64>,
        position_ms: u64,
    ) -> Result<PreparedTrack, String> {
        let decoder = spawn_decoder(path.clone(), Some(position_ms))?;
        let initial_frames_consumed = position_ms.saturating_mul(decoder.sample_rate as u64) / 1000;
        Ok(PreparedTrack {
            decoder,
            file_path: path,
            song_id,
            queue_id,
            from_rotation,
            declared_duration_ms,
            initial_frames_consumed,
        })
    }

    pub fn request_attach(&mut self, prepared: PreparedTrack, op: AttachOp) {
        if matches!(self.state, DeckState::Playing | DeckState::Crossfading) {
            self.pending_swap = Some(PendingSwap { prepared, op });
            self.maybe_begin_pending_swap();
        } else {
            self.apply_prepared(prepared, op);
        }
    }

    /// Load a new track. Stops any existing playback.
    pub fn load(
        &mut self,
        path: PathBuf,
        song_id: Option<i64>,
        queue_id: Option<i64>,
        from_rotation: bool,
        declared_duration_ms: Option<u64>,
    ) -> Result<(), String> {
        self.stop_decoder();
        self.state = DeckState::Loading;
        self.file_path = Some(path.clone());
        self.song_id = song_id;
        self.queue_id = queue_id;
        self.from_rotation = from_rotation;
        self.declared_duration_ms = declared_duration_ms;
        self.frames_consumed = 0;
        self.xfade_gain = 1.0;
        self.ended_naturally = false;
        self.completion_pending = None;
        self.reset_resampler();
        self.reset_play_ramp();
        self.reset_swap_state();
        self.clear_loop();

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
        self.reset_resampler();
        self.reset_swap_state();

        let handle = spawn_decoder(path, Some(position_ms))?;
        self.sample_rate = handle.sample_rate;
        self.decoder = Some(handle);

        if self.state == DeckState::Playing || self.state == DeckState::Crossfading {
            // Keep playing state — the render thread will pick up the new ring buffer
            self.arm_play_ramp();
        } else {
            self.state = DeckState::Ready;
        }
        Ok(())
    }

    pub fn play(&mut self) {
        if self.state == DeckState::Ready || self.state == DeckState::Paused {
            self.paused = false;
            self.state = DeckState::Playing;
            self.arm_play_ramp();
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
        self.state = DeckState::Idle;
        self.frames_consumed = 0;
        self.paused = false;
        self.ended_naturally = false;
        self.file_path = None;
        self.song_id = None;
        self.queue_id = None;
        self.from_rotation = false;
        self.declared_duration_ms = None;
        self.clear_loop();
        self.reset_resampler();
        self.reset_play_ramp();
        self.reset_swap_state();
    }

    pub fn set_crossfading(&mut self) {
        if self.state == DeckState::Playing {
            self.state = DeckState::Crossfading;
        }
    }

    pub fn set_linked_playback_pct(&mut self, pct: f32) {
        self.set_pitch_pct(pct);
        self.set_tempo_pct(pct);
    }

    pub fn set_pitch_pct(&mut self, pct: f32) {
        self.pitch_pct = pct.clamp(-50.0, 50.0);
        self.playback_rate = (1.0 + self.pitch_pct / 100.0).clamp(0.5, 1.5);
    }

    pub fn set_tempo_pct(&mut self, pct: f32) {
        self.tempo_pct = pct.clamp(-50.0, 50.0);
        self.playback_rate = (1.0 + self.tempo_pct / 100.0).clamp(0.5, 1.5);
    }

    pub fn set_loop_range_ms(&mut self, start_ms: u64, end_ms: u64) -> Result<(), String> {
        if self.sample_rate == 0 {
            return Err("Invalid sample rate for loop".to_string());
        }
        if end_ms <= start_ms + 10 {
            return Err("Loop end must be greater than loop start".to_string());
        }
        let start_frame = start_ms.saturating_mul(self.sample_rate as u64) / 1000;
        let end_frame = end_ms.saturating_mul(self.sample_rate as u64) / 1000;
        if end_frame <= start_frame + 16 {
            return Err("Loop range too short".to_string());
        }
        let loop_frames = end_frame.saturating_sub(start_frame);
        let max_frames = self.sample_rate as u64 * MAX_LOOP_SECONDS;
        if loop_frames > max_frames {
            return Err(format!("Loop too long (max {MAX_LOOP_SECONDS}s)"));
        }
        let sample_len = loop_frames.saturating_mul(2).min(usize::MAX as u64) as usize;
        self.loop_state = Some(LoopState {
            start_frame,
            end_frame,
            cached_frames: 0,
            play_frame: 0,
            playing_from_buffer: false,
            buffer: vec![0.0; sample_len],
        });
        Ok(())
    }

    pub fn clear_loop(&mut self) {
        if let Some(loop_state) = self.loop_state.take() {
            if loop_state.playing_from_buffer {
                self.frames_consumed = loop_state.end_frame;
                if matches!(self.state, DeckState::Playing | DeckState::Crossfading) {
                    self.arm_play_ramp_ms(4);
                }
            }
        }
    }

    pub fn stop_with_completion(&mut self) {
        let completion = self.song_id.map(|song_id| TrackCompletion {
            song_id,
            queue_id: self.queue_id,
            from_rotation: self.from_rotation,
        });
        self.stop();
        self.completion_pending = completion;
    }

    pub fn mark_eof_stop(&mut self) {
        self.stop_with_completion();
        self.ended_naturally = true;
    }

    pub fn take_completion(&mut self) -> Option<TrackCompletion> {
        self.completion_pending.take()
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
        let decoded = self.decoder.as_ref().map(|d| d.duration_ms()).unwrap_or(0);
        if decoded > 0 {
            decoded
        } else {
            self.declared_duration_ms.unwrap_or(0)
        }
    }

    /// How many frames remain (approximately)
    pub fn remaining_frames(&self) -> u64 {
        let total = self
            .decoder
            .as_ref()
            .map(|d| d.total_frames.load(Ordering::Relaxed))
            .unwrap_or(0);
        if total > self.frames_consumed {
            total - self.frames_consumed
        } else {
            0
        }
    }

    /// Remaining time in ms
    pub fn remaining_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        self.remaining_frames() * 1000 / self.sample_rate as u64
    }

    /// Approximate decoded audio buffered ahead in ms.
    pub fn decoder_buffered_ms(&self) -> u64 {
        let Some(decoder) = &self.decoder else {
            return 0;
        };
        if self.sample_rate == 0 {
            return 0;
        }
        let samples = decoder.consumer.occupied_len() as u64;
        let frames = samples / 2;
        frames * 1000 / self.sample_rate as u64
    }

    /// Whether the decoder ring buffer is exhausted and the track has ended
    pub fn is_eof(&self) -> bool {
        match &self.decoder {
            Some(d) => {
                // EOF when decoder has written all frames and ring buffer is empty
                let written = d.frames_written.load(Ordering::Relaxed);
                let total = d.total_frames.load(Ordering::Relaxed);
                let done = d.decode_done.load(Ordering::Relaxed);
                ((total > 0 && written >= total) || done) && d.consumer.is_empty()
            }
            None => true,
        }
    }

    /// Fill `output` with interleaved stereo f32 samples, scaled by `self.gain`.
    ///
    /// `device_sr` is the CPAL output device's sample rate. When it differs from
    /// the track's native sample rate (`self.sample_rate`), linear interpolation
    /// resampling is applied to correct pitch and speed.
    ///
    /// Zeros are written for any frames the ring buffer cannot supply (underrun).
    ///
    /// Called on the real-time audio thread — **no allocations, no locks**.
    pub fn fill_buffer(&mut self, output: &mut [f32], device_sr: u32) {
        if self.paused || !matches!(self.state, DeckState::Playing | DeckState::Crossfading) {
            output.fill(0.0);
            self.rms_db_pre_fader = -96.0;
            return;
        }

        if self.decoder.is_none() {
            output.fill(0.0);
            self.rms_db_pre_fader = -96.0;
            return;
        }

        let file_sr = self.sample_rate;
        let out_frames = output.len() / 2;
        let mut rms_sum_sq = 0.0_f64;
        let mut rms_samples = 0_u64;
        self.maybe_begin_pending_swap();
        self.ensure_play_ramp(device_sr);
        self.ensure_swap_out(device_sr);

        use ringbuf::traits::Consumer as _;

        let use_fast_path = (file_sr == device_sr || file_sr == 0 || device_sr == 0)
            && (self.playback_rate - 1.0).abs() < 1e-6;

        if use_fast_path {
            // ── Fast path: rates match, direct copy ──────────────────────
            // Reset any prior resampler state so switching back to non-1.0
            // playback starts from the current decoder position.
            self.resample_seeded = false;
            self.resample_phase = 0.0;
            let mut out_i = 0usize;
            while out_i < output.len() {
                if self.swap_out_total_frames > 0
                    && self.swap_out_remaining_frames == 0
                    && self.pending_swap.is_some()
                {
                    self.apply_pending_swap();
                }
                let pair = if let Some(loop_pair) = self.next_loop_buffer_frame() {
                    Some(loop_pair)
                } else {
                    let decoder = self.decoder.as_mut().unwrap();
                    if decoder.consumer.occupied_len() < 2 {
                        None
                    } else {
                        Some((
                            decoder.consumer.try_pop().unwrap_or(0.0),
                            decoder.consumer.try_pop().unwrap_or(0.0),
                        ))
                    }
                };
                let Some((l, r)) = pair else {
                    output[out_i..].fill(0.0);
                    break;
                };
                if !self
                    .loop_state
                    .as_ref()
                    .is_some_and(|s| s.playing_from_buffer)
                {
                    let frame_index = self.frames_consumed;
                    self.frames_consumed = self.frames_consumed.saturating_add(1);
                    self.capture_loop_frame(frame_index, l, r);
                }
                let start_gain = self.next_play_ramp_gain();
                let swap_gain = self.next_swap_out_gain();
                let gain = self.channel_gain * self.xfade_gain * start_gain * swap_gain;
                output[out_i] = l * gain;
                output[out_i + 1] = r * gain;
                let l64 = l as f64;
                let r64 = r as f64;
                rms_sum_sq += l64 * l64 + r64 * r64;
                rms_samples += 2;
                out_i += 2;
            }
        } else {
            // ── Resampling path: linear interpolation ────────────────────
            //
            // We maintain a fractional phase [0, 1) representing how far we
            // are between two consecutive source frames (prev, next).
            // For each output frame we interpolate between prev and next, then
            // advance phase by `ratio = file_sr / device_sr`.
            // Each time phase crosses 1.0 we consume the next source frame.
            //
            // Example: file=44100, device=48000 → ratio≈0.919
            //   Each output frame advances phase by 0.919; a new source frame
            //   is consumed roughly every 1.088 output frames.
            if !self.resample_seeded {
                let seeded = {
                    let decoder = self.decoder.as_mut().unwrap();
                    if decoder.consumer.occupied_len() >= 4 {
                        let l0 = decoder.consumer.try_pop().unwrap_or(0.0);
                        let r0 = decoder.consumer.try_pop().unwrap_or(0.0);
                        let l1 = decoder.consumer.try_pop().unwrap_or(0.0);
                        let r1 = decoder.consumer.try_pop().unwrap_or(0.0);
                        Some((l0, r0, l1, r1))
                    } else {
                        None
                    }
                };
                if let Some((l0, r0, l1, r1)) = seeded {
                    self.resample_prev_l = l0;
                    self.resample_prev_r = r0;
                    self.resample_next_l = l1;
                    self.resample_next_r = r1;
                    self.resample_phase = 0.0;
                    self.resample_seeded = true;
                } else {
                    output.fill(0.0);
                    self.rms_db_pre_fader = -96.0;
                    return;
                }
            }

            let ratio = file_sr as f64 * self.playback_rate as f64 / device_sr as f64;

            for out_i in 0..out_frames {
                if self.swap_out_total_frames > 0
                    && self.swap_out_remaining_frames == 0
                    && self.pending_swap.is_some()
                {
                    self.apply_pending_swap();
                }
                let t = self.resample_phase as f32;

                // Interpolate L and R channels
                let out_l =
                    self.resample_prev_l + t * (self.resample_next_l - self.resample_prev_l);
                let out_r =
                    self.resample_prev_r + t * (self.resample_next_r - self.resample_prev_r);
                let out_l64 = out_l as f64;
                let out_r64 = out_r as f64;
                rms_sum_sq += out_l64 * out_l64 + out_r64 * out_r64;
                rms_samples += 2;
                let start_gain = self.next_play_ramp_gain();
                let swap_gain = self.next_swap_out_gain();
                let gain = self.channel_gain * self.xfade_gain * start_gain * swap_gain;
                output[out_i * 2] = out_l * gain;
                output[out_i * 2 + 1] = out_r * gain;

                // Advance fractional phase
                self.resample_phase += ratio;

                // Consume as many source frames as the phase advance requires.
                // Usually 0–1 per output frame; occasionally 2 when ratio > 1.
                while self.resample_phase >= 1.0 {
                    self.resample_prev_l = self.resample_next_l;
                    self.resample_prev_r = self.resample_next_r;

                    let next_pair = {
                        let decoder = self.decoder.as_mut().unwrap();
                        if decoder.consumer.occupied_len() >= 2 {
                            Some((
                                decoder.consumer.try_pop().unwrap_or(0.0),
                                decoder.consumer.try_pop().unwrap_or(0.0),
                            ))
                        } else {
                            None
                        }
                    };
                    if let Some((next_l, next_r)) = next_pair {
                        self.resample_next_l = next_l;
                        self.resample_next_r = next_r;
                        self.frames_consumed += 1;
                    }
                    // On underrun: keep next == prev (repeat last frame).
                    // This is a gentle hold — better than a hard silence click.

                    self.resample_phase -= 1.0;
                }
            }
        }

        if rms_samples > 0 {
            let rms = (rms_sum_sq / rms_samples as f64).sqrt() as f32;
            self.rms_db_pre_fader = linear_to_db(rms.max(1e-10));
        } else {
            self.rms_db_pre_fader = -96.0;
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn stop_decoder(&mut self) {
        if let Some(d) = self.decoder.take() {
            d.stop_flag.store(true, Ordering::Relaxed);
            // Thread will exit on its own after seeing stop_flag
        }
    }

    /// Reset linear-interpolation resampler state. Call on every load/seek so
    /// we don't carry stale samples from a previous track into the new one.
    fn reset_resampler(&mut self) {
        self.resample_phase = 0.0;
        self.resample_seeded = false;
        self.resample_prev_l = 0.0;
        self.resample_prev_r = 0.0;
        self.resample_next_l = 0.0;
        self.resample_next_r = 0.0;
    }

    fn apply_prepared(&mut self, prepared: PreparedTrack, op: AttachOp) {
        let was_paused = self.state == DeckState::Paused;
        self.stop_decoder();
        self.decoder = Some(prepared.decoder);
        self.file_path = Some(prepared.file_path);
        self.song_id = prepared.song_id;
        self.queue_id = prepared.queue_id;
        self.from_rotation = prepared.from_rotation;
        self.declared_duration_ms = prepared.declared_duration_ms;
        self.sample_rate = self
            .decoder
            .as_ref()
            .map(|d| d.sample_rate)
            .unwrap_or(self.sample_rate);
        self.frames_consumed = prepared.initial_frames_consumed;
        self.ended_naturally = false;
        self.completion_pending = None;
        self.reset_resampler();
        self.swap_out_armed = false;
        self.swap_out_total_frames = 0;
        self.swap_out_remaining_frames = 0;
        if matches!(op, AttachOp::Load) {
            self.clear_loop();
            // Fresh track loads should not inherit old transport offsets.
            self.pitch_pct = 0.0;
            self.tempo_pct = 0.0;
            self.playback_rate = 1.0;
        }

        if matches!(self.state, DeckState::Playing | DeckState::Crossfading) {
            self.paused = false;
            self.arm_play_ramp_ms(12);
        } else if was_paused && matches!(op, AttachOp::Seek) {
            self.paused = true;
            self.state = DeckState::Paused;
            self.reset_play_ramp();
        } else {
            self.paused = false;
            self.state = DeckState::Ready;
            self.reset_play_ramp();
        }

        log::info!(
            "deck_swap_apply deck={} op={:?} state={:?} start_ms={}",
            self.id,
            op,
            self.state,
            self.position_ms()
        );
    }

    fn apply_pending_swap(&mut self) {
        if let Some(pending) = self.pending_swap.take() {
            self.apply_prepared(pending.prepared, pending.op);
        }
    }

    fn arm_swap_out(&mut self) {
        self.swap_out_armed = true;
        self.swap_out_total_frames = 0;
        self.swap_out_remaining_frames = 0;
    }

    fn maybe_begin_pending_swap(&mut self) {
        if self.pending_swap.is_none() || self.swap_out_armed || self.swap_out_remaining_frames > 0
        {
            return;
        }
        if self.pending_swap_ready() {
            self.arm_swap_out();
        }
    }

    fn pending_swap_ready(&self) -> bool {
        let Some(pending) = self.pending_swap.as_ref() else {
            return false;
        };
        let sr = pending.prepared.decoder.sample_rate.max(1) as u64;
        let needed_frames = ((sr * SWAP_PREROLL_MS) / 1000).max(32);
        let buffered_frames = pending.prepared.decoder.consumer.occupied_len() as u64 / 2;
        buffered_frames >= needed_frames
    }

    fn ensure_swap_out(&mut self, device_sr: u32) {
        if !self.swap_out_armed {
            return;
        }
        let frames = ((device_sr as u64 * SWAP_OUT_MS) / 1000).max(1);
        self.swap_out_total_frames = frames.min(u32::MAX as u64) as u32;
        self.swap_out_remaining_frames = self.swap_out_total_frames;
        self.swap_out_armed = false;
        log::info!(
            "deck_swap_start deck={} frames={}",
            self.id,
            self.swap_out_total_frames
        );
    }

    fn reset_swap_state(&mut self) {
        self.swap_out_armed = false;
        self.swap_out_total_frames = 0;
        self.swap_out_remaining_frames = 0;
        self.pending_swap = None;
    }

    fn arm_play_ramp(&mut self) {
        self.arm_play_ramp_ms(8);
    }

    fn arm_play_ramp_ms(&mut self, ramp_ms: u64) {
        self.play_ramp_armed = true;
        self.play_ramp_ms = ramp_ms.max(1);
        self.play_ramp_total_frames = 0;
        self.play_ramp_remaining_frames = 0;
    }

    fn reset_play_ramp(&mut self) {
        self.play_ramp_armed = false;
        self.play_ramp_ms = 8;
        self.play_ramp_total_frames = 0;
        self.play_ramp_remaining_frames = 0;
    }

    fn ensure_play_ramp(&mut self, device_sr: u32) {
        if !self.play_ramp_armed {
            return;
        }
        let frames = ((device_sr as u64 * self.play_ramp_ms) / 1000).max(1);
        self.play_ramp_total_frames = frames.min(u32::MAX as u64) as u32;
        self.play_ramp_remaining_frames = self.play_ramp_total_frames;
        self.play_ramp_armed = false;
    }

    #[inline]
    fn next_play_ramp_gain(&mut self) -> f32 {
        if self.play_ramp_remaining_frames == 0 || self.play_ramp_total_frames == 0 {
            return 1.0;
        }
        let progressed = self.play_ramp_total_frames - self.play_ramp_remaining_frames;
        let gain = (progressed as f32 / self.play_ramp_total_frames as f32).clamp(0.0, 1.0);
        self.play_ramp_remaining_frames -= 1;
        gain
    }

    #[inline]
    fn next_swap_out_gain(&mut self) -> f32 {
        if self.swap_out_remaining_frames == 0 || self.swap_out_total_frames == 0 {
            return 1.0;
        }
        let gain = ((self.swap_out_remaining_frames - 1) as f32
            / self.swap_out_total_frames as f32)
            .clamp(0.0, 1.0);
        self.swap_out_remaining_frames -= 1;
        gain
    }

    fn capture_loop_frame(&mut self, frame_index: u64, l: f32, r: f32) {
        let Some(loop_state) = self.loop_state.as_mut() else {
            return;
        };
        if loop_state.playing_from_buffer {
            return;
        }
        if frame_index < loop_state.start_frame || frame_index >= loop_state.end_frame {
            return;
        }
        let rel = frame_index.saturating_sub(loop_state.start_frame);
        let idx = rel.saturating_mul(2) as usize;
        if idx + 1 < loop_state.buffer.len() {
            loop_state.buffer[idx] = l;
            loop_state.buffer[idx + 1] = r;
            loop_state.cached_frames = loop_state.cached_frames.max(rel + 1);
        }
        if frame_index + 1 >= loop_state.end_frame && loop_state.cached_frames >= 2 {
            loop_state.playing_from_buffer = true;
            loop_state.play_frame = 0;
            self.frames_consumed = loop_state.start_frame;
        }
    }

    fn next_loop_buffer_frame(&mut self) -> Option<(f32, f32)> {
        let sr = self.sample_rate.max(1) as u64;
        let loop_state = self.loop_state.as_mut()?;
        if !loop_state.playing_from_buffer || loop_state.cached_frames == 0 {
            return None;
        }
        let len = loop_state.cached_frames;
        let play = loop_state.play_frame % len;
        let target_wrap_blend = (sr.saturating_mul(3) / 1000)
            .clamp(LOOP_WRAP_MIN_XFADE_FRAMES, LOOP_WRAP_MAX_XFADE_FRAMES);
        let wrap_blend = target_wrap_blend
            .min(len.saturating_sub(1))
            .min((len / 4).max(1));
        let idx = play.saturating_mul(2) as usize;
        if idx + 1 >= loop_state.buffer.len() {
            return None;
        }
        let mut l = loop_state.buffer[idx];
        let mut r = loop_state.buffer[idx + 1];
        // Crossfade tail->head across a short region to smooth the loop seam.
        // After wrapping, skip already blended head frames to avoid replaying
        // the same transient twice at the boundary.
        if play >= len.saturating_sub(wrap_blend) && wrap_blend > 0 {
            let blend_pos = play.saturating_sub(len.saturating_sub(wrap_blend));
            let head_idx = blend_pos.saturating_mul(2) as usize;
            if head_idx + 1 < loop_state.buffer.len() {
                let t = blend_pos as f32 / wrap_blend as f32;
                let theta = t * std::f32::consts::FRAC_PI_2;
                let w_tail = theta.cos();
                let w_head = theta.sin();
                let hl = loop_state.buffer[head_idx];
                let hr = loop_state.buffer[head_idx + 1];
                l = l * w_tail + hl * w_head;
                r = r * w_tail + hr * w_head;
            }
        }
        self.frames_consumed = loop_state.start_frame + play;
        let next_play = play + 1;
        loop_state.play_frame = if next_play >= len {
            if wrap_blend > 0 {
                wrap_blend % len
            } else {
                0
            }
        } else {
            next_play
        };
        Some((l, r))
    }
}

impl Drop for Deck {
    fn drop(&mut self) {
        self.stop_decoder();
    }
}

#[inline]
fn linear_to_db(linear: f32) -> f32 {
    if linear <= 1e-10 {
        -96.0
    } else {
        20.0 * linear.log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::crossfade::DeckId;

    #[test]
    fn play_ramp_starts_at_zero_and_finishes_at_unity() {
        let mut deck = Deck::new(DeckId::DeckA);
        deck.play_ramp_total_frames = 4;
        deck.play_ramp_remaining_frames = 4;

        assert_eq!(deck.next_play_ramp_gain(), 0.0);
        assert_eq!(deck.next_play_ramp_gain(), 0.25);
        assert_eq!(deck.next_play_ramp_gain(), 0.5);
        assert_eq!(deck.next_play_ramp_gain(), 0.75);
        assert_eq!(deck.next_play_ramp_gain(), 1.0);
    }

    #[test]
    fn swap_out_ramp_reaches_zero_before_swap() {
        let mut deck = Deck::new(DeckId::DeckA);
        deck.swap_out_total_frames = 4;
        deck.swap_out_remaining_frames = 4;

        assert_eq!(deck.next_swap_out_gain(), 0.75);
        assert_eq!(deck.next_swap_out_gain(), 0.5);
        assert_eq!(deck.next_swap_out_gain(), 0.25);
        assert_eq!(deck.next_swap_out_gain(), 0.0);
        assert_eq!(deck.next_swap_out_gain(), 1.0);
    }
}
