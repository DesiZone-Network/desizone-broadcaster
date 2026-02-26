use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use ringbuf::{traits::Split, HeapRb};
use serde::{Deserialize, Serialize};

use crate::db::local::MonitorRoutingConfig;

use super::{
    crossfade::{CrossfadeConfig, CrossfadeState, CrossfadeTriggerMode, DeckId},
    deck::{AttachOp, Deck, DeckState, PreparedTrack, TrackCompletion},
    dsp::{
        pipeline::{ChannelPipeline, PipelineSettings},
        stem_filter::{StemFilterConfig, StemFilterMode},
    },
    mixer::Mixer,
};

// ── VU event ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VuEvent {
    pub channel: String,
    pub left_db: f32,
    pub right_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossfadeProgressEvent {
    pub progress: f32,
    pub outgoing_deck: String,
    pub incoming_deck: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckStateEvent {
    pub deck: String,
    pub state: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub song_id: Option<i64>,
    pub file_path: Option<String>,
    pub playback_rate: f32,
    pub pitch_pct: f32,
    pub tempo_pct: f32,
    pub decoder_buffer_ms: u64,
    pub rms_db_pre_fader: f32,
    pub cue_preview_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackCompletionEvent {
    pub deck: String,
    pub song_id: i64,
    pub queue_id: Option<i64>,
    pub from_rotation: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualFadeDirection {
    AtoB,
    BtoA,
}

// ── Engine ───────────────────────────────────────────────────────────────────

/// Callback-safe shared state — no Mutex, all accessed only from the CPAL thread.
struct RtState {
    decks: HashMap<DeckId, Deck>,
    pipelines: HashMap<DeckId, ChannelPipeline>,
    master_pipeline: ChannelPipeline,
    mixer: Mixer,
    crossfade: CrossfadeState,
    crossfade_config: CrossfadeConfig,
    manual_crossfade_pos: f32,
    cue_preview_enabled: HashMap<DeckId, bool>,
    cue_split_active: bool,
    cue_level: f32,
    master_level: f32,
    sample_rate: u32,
    // Per-channel scratch buffers (avoid alloc in callback)
    buf_deck_a: Vec<f32>,
    buf_deck_b: Vec<f32>,
    buf_sound_fx: Vec<f32>,
    buf_aux1: Vec<f32>,
    buf_aux2: Vec<f32>,
    buf_voice_fx: Vec<f32>,
    buf_silence: Vec<f32>,
    // Encoder ring buffer producer (to stream/icecast thread)
    encoder_prod: ringbuf::HeapProd<f32>,
}

/// Commands sent from the main thread → real-time thread via a lock-free channel.
/// Kept small; heavy state lives in `AudioEngine` behind the Mutex.
enum EngineCmd {
    AttachPreparedTrack {
        deck: DeckId,
        prepared: PreparedTrack,
        op: AttachOp,
    },
    Play(DeckId),
    Pause(DeckId),
    StopWithCompletion(DeckId),
    SetGain {
        deck: DeckId,
        gain: f32,
    },
    SetDeckPitch {
        deck: DeckId,
        pct: f32,
    },
    SetDeckTempo {
        deck: DeckId,
        pct: f32,
    },
    SetDeckLoop {
        deck: DeckId,
        start_ms: u64,
        end_ms: u64,
    },
    ClearDeckLoop(DeckId),
    StartCrossfade {
        outgoing: DeckId,
        incoming: DeckId,
    },
    SetManualCrossfade {
        position: f32,
    },
    TriggerManualFade {
        direction: ManualFadeDirection,
        duration_ms: u32,
    },
    SetCrossfadeConfig(CrossfadeConfig),
    SetChannelPipeline {
        deck: DeckId,
        settings: PipelineSettings,
    },
    SetMasterPipeline {
        settings: PipelineSettings,
    },
    SetDeckCuePreview {
        deck: DeckId,
        enabled: bool,
    },
    SetMonitorRoutingConfig(MonitorRoutingConfig),
}

/// The main audio engine — lives behind `Arc<Mutex<AudioEngine>>` in `AppState`.
pub struct AudioEngine {
    _stream: Option<Stream>,
    // Encoder consumer (icecast thread reads from here)
    pub encoder_consumer: Option<ringbuf::HeapCons<f32>>,
    // Command sender to the RT thread
    cmd_tx: ringbuf::HeapProd<EngineCmd>,
    // Shared state accessible from both the main thread (for queries) and
    // the CPAL callback (for audio).
    rt_state: Arc<Mutex<RtState>>,
    #[allow(dead_code)]
    sample_rate: u32,
}

impl AudioEngine {
    const ENCODER_RING_SIZE: usize = 44100 * 2 * 10; // 10 s encoder buffer
    const CMD_RING_SIZE: usize = 64;

    /// Initialise and start the CPAL output stream.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default audio output device found")?;

        let config = device
            .default_output_config()
            .map_err(|e| format!("Default config error: {e}"))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        log::info!(
            "Audio device: {} | sample rate: {} | channels: {}",
            device.name().unwrap_or_default(),
            sample_rate,
            channels
        );

        // Encoder ring buffer
        let enc_rb = HeapRb::<f32>::new(Self::ENCODER_RING_SIZE);
        let (enc_prod, enc_cons) = enc_rb.split();

        // Command ring buffer (main → RT)
        let cmd_rb = HeapRb::<EngineCmd>::new(Self::CMD_RING_SIZE);
        let (cmd_prod, cmd_cons) = cmd_rb.split();

        // Shared RT state (wrapped in Arc<Mutex> so the main thread can query it)
        let rt_arc = Arc::new(Mutex::new(RtState {
            decks: {
                let mut m = HashMap::new();
                m.insert(DeckId::DeckA, Deck::new(DeckId::DeckA));
                m.insert(DeckId::DeckB, Deck::new(DeckId::DeckB));
                m.insert(DeckId::SoundFx, Deck::new(DeckId::SoundFx));
                m.insert(DeckId::Aux1, Deck::new(DeckId::Aux1));
                m.insert(DeckId::Aux2, Deck::new(DeckId::Aux2));
                m.insert(DeckId::VoiceFx, Deck::new(DeckId::VoiceFx));
                m
            },
            pipelines: {
                let mut m = HashMap::new();
                for id in [
                    DeckId::DeckA,
                    DeckId::DeckB,
                    DeckId::SoundFx,
                    DeckId::Aux1,
                    DeckId::Aux2,
                    DeckId::VoiceFx,
                ] {
                    let mut pipeline = ChannelPipeline::new(sample_rate as f32);
                    // Tuned defaults per channel type (mode remains OFF).
                    match id {
                        DeckId::DeckA | DeckId::DeckB => {
                            pipeline.stem_filter.set_config(StemFilterConfig {
                                mode: StemFilterMode::Off,
                                amount: 0.82,
                            });
                        }
                        DeckId::VoiceFx => {
                            pipeline.stem_filter.set_config(StemFilterConfig {
                                mode: StemFilterMode::Off,
                                amount: 0.55,
                            });
                        }
                        _ => {}
                    }
                    m.insert(id, pipeline);
                }
                m
            },
            master_pipeline: ChannelPipeline::new(sample_rate as f32),
            mixer: Mixer::new(),
            crossfade: CrossfadeState::default(),
            crossfade_config: CrossfadeConfig::default(),
            manual_crossfade_pos: -1.0,
            cue_preview_enabled: {
                let mut m = HashMap::new();
                m.insert(DeckId::DeckA, false);
                m.insert(DeckId::DeckB, false);
                m
            },
            cue_split_active: false,
            cue_level: 1.0,
            master_level: 1.0,
            sample_rate,
            buf_deck_a: Vec::new(),
            buf_deck_b: Vec::new(),
            buf_sound_fx: Vec::new(),
            buf_aux1: Vec::new(),
            buf_aux2: Vec::new(),
            buf_voice_fx: Vec::new(),
            buf_silence: Vec::new(),
            encoder_prod: enc_prod,
        }));

        let rt_arc_cb = Arc::clone(&rt_arc);

        let stream = Self::build_stream(&device, &config.into(), rt_arc_cb, cmd_cons)?;
        stream
            .play()
            .map_err(|e| format!("Stream play error: {e}"))?;

        Ok(Self {
            _stream: Some(stream),
            encoder_consumer: Some(enc_cons),
            cmd_tx: cmd_prod,
            rt_state: rt_arc,
            sample_rate,
        })
    }

    // ── Public control API ────────────────────────────────────────────────

    pub fn load_track(
        &mut self,
        deck: DeckId,
        path: PathBuf,
        song_id: Option<i64>,
    ) -> Result<(), String> {
        self.load_track_with_source(deck, path, song_id, None, false, None)
    }

    pub fn load_track_with_source(
        &mut self,
        deck: DeckId,
        path: PathBuf,
        song_id: Option<i64>,
        queue_id: Option<i64>,
        from_rotation: bool,
        declared_duration_ms: Option<u64>,
    ) -> Result<(), String> {
        let prepared =
            Deck::prepare_load(path, song_id, queue_id, from_rotation, declared_duration_ms)?;
        self.send_cmd(EngineCmd::AttachPreparedTrack {
            deck,
            prepared,
            op: AttachOp::Load,
        })
    }

    pub fn play(&mut self, deck: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::Play(deck))
    }

    pub fn pause(&mut self, deck: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::Pause(deck))
    }

    pub fn stop_with_completion(&mut self, deck: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::StopWithCompletion(deck))
    }

    pub fn seek(&mut self, deck: DeckId, position_ms: u64) -> Result<(), String> {
        let (path, song_id, queue_id, from_rotation, declared_duration_ms) = {
            let rt = self.rt_state.lock().unwrap();
            let d = rt.decks.get(&deck).ok_or("Unknown deck")?;
            let path = d.file_path.clone().ok_or("No track loaded")?;
            (
                path,
                d.song_id,
                d.queue_id,
                d.from_rotation,
                d.declared_duration_ms,
            )
        };
        let prepared = Deck::prepare_seek(
            path,
            song_id,
            queue_id,
            from_rotation,
            declared_duration_ms,
            position_ms,
        )?;
        self.send_cmd(EngineCmd::AttachPreparedTrack {
            deck,
            prepared,
            op: AttachOp::Seek,
        })
    }

    pub fn switch_deck_track_source(
        &mut self,
        deck: DeckId,
        new_path: PathBuf,
    ) -> Result<(), String> {
        if !new_path.exists() {
            return Err(format!("File not found: {}", new_path.display()));
        }
        if !new_path.is_file() {
            return Err(format!("Path is not a file: {}", new_path.display()));
        }

        let (current_path, song_id, queue_id, from_rotation, declared_duration_ms, position_ms) = {
            let rt = self.rt_state.lock().unwrap();
            let d = rt.decks.get(&deck).ok_or("Unknown deck")?;
            let current_path = d.file_path.clone().ok_or("No track loaded")?;
            (
                current_path,
                d.song_id,
                d.queue_id,
                d.from_rotation,
                d.declared_duration_ms,
                d.position_ms(),
            )
        };

        if current_path == new_path {
            return Ok(());
        }

        let prepared = Deck::prepare_seek(
            new_path,
            song_id,
            queue_id,
            from_rotation,
            declared_duration_ms,
            position_ms,
        )?;
        self.send_cmd(EngineCmd::AttachPreparedTrack {
            deck,
            prepared,
            op: AttachOp::Seek,
        })
    }

    pub fn set_channel_gain(&mut self, deck: DeckId, gain: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetGain { deck, gain })
    }

    pub fn set_deck_pitch(&mut self, deck: DeckId, pitch_pct: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetDeckPitch {
            deck,
            pct: pitch_pct,
        })
    }

    pub fn set_deck_tempo(&mut self, deck: DeckId, tempo_pct: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetDeckTempo {
            deck,
            pct: tempo_pct,
        })
    }

    pub fn set_deck_loop(
        &mut self,
        deck: DeckId,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetDeckLoop {
            deck,
            start_ms,
            end_ms,
        })
    }

    pub fn clear_deck_loop(&mut self, deck: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::ClearDeckLoop(deck))
    }

    pub fn start_crossfade(&mut self, outgoing: DeckId, incoming: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::StartCrossfade { outgoing, incoming })
    }

    pub fn set_crossfade_config(&mut self, config: CrossfadeConfig) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetCrossfadeConfig(config))
    }

    pub fn set_manual_crossfade(&mut self, position: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetManualCrossfade { position })
    }

    pub fn trigger_manual_fade(
        &mut self,
        direction: ManualFadeDirection,
        duration_ms: u32,
    ) -> Result<(), String> {
        self.send_cmd(EngineCmd::TriggerManualFade {
            direction,
            duration_ms,
        })
    }

    pub fn set_channel_pipeline(
        &mut self,
        deck: DeckId,
        settings: PipelineSettings,
    ) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetChannelPipeline { deck, settings })
    }

    pub fn set_master_pipeline(&mut self, settings: PipelineSettings) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetMasterPipeline { settings })
    }

    pub fn set_deck_cue_preview_enabled(
        &mut self,
        deck: DeckId,
        enabled: bool,
    ) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetDeckCuePreview { deck, enabled })
    }

    pub fn set_monitor_routing_config(&mut self, config: MonitorRoutingConfig) {
        let _ = self.send_cmd(EngineCmd::SetMonitorRoutingConfig(config));
    }

    pub fn get_crossfade_config(&self) -> CrossfadeConfig {
        self.rt_state.lock().unwrap().crossfade_config.clone()
    }

    pub fn get_deck_state(&self, deck: DeckId) -> Option<DeckStateEvent> {
        let rt = self.rt_state.lock().unwrap();
        rt.decks.get(&deck).map(|d| DeckStateEvent {
            deck: deck.to_string(),
            state: format!("{:?}", d.state).to_lowercase(),
            position_ms: d.position_ms(),
            duration_ms: d.duration_ms(),
            song_id: d.song_id,
            file_path: d
                .file_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            playback_rate: d.playback_rate,
            pitch_pct: d.pitch_pct,
            tempo_pct: d.tempo_pct,
            decoder_buffer_ms: d.decoder_buffered_ms(),
            rms_db_pre_fader: d.rms_db_pre_fader,
            cue_preview_enabled: rt.cue_preview_enabled.get(&deck).copied().unwrap_or(false),
        })
    }

    pub fn get_crossfade_progress_event(&self) -> Option<CrossfadeProgressEvent> {
        let rt = self.rt_state.lock().unwrap();
        let progress = rt.crossfade.progress()?;
        let outgoing = rt.crossfade.outgoing()?;
        let incoming = rt.crossfade.incoming()?;
        Some(CrossfadeProgressEvent {
            progress,
            outgoing_deck: outgoing.to_string(),
            incoming_deck: incoming.to_string(),
        })
    }

    pub fn get_manual_crossfade_pos(&self) -> f32 {
        self.rt_state.lock().unwrap().manual_crossfade_pos
    }

    pub fn take_track_completions(&self) -> Vec<TrackCompletionEvent> {
        let mut rt = self.rt_state.lock().unwrap();
        let mut out = Vec::new();
        for id in [
            DeckId::DeckA,
            DeckId::DeckB,
            DeckId::SoundFx,
            DeckId::Aux1,
            DeckId::Aux2,
            DeckId::VoiceFx,
        ] {
            if let Some(deck) = rt.decks.get_mut(&id) {
                if let Some(TrackCompletion {
                    song_id,
                    queue_id,
                    from_rotation,
                }) = deck.take_completion()
                {
                    out.push(TrackCompletionEvent {
                        deck: id.to_string(),
                        song_id,
                        queue_id,
                        from_rotation,
                    });
                }
            }
        }
        out
    }

    pub fn get_vu_readings(&self) -> Vec<VuEvent> {
        let rt = self.rt_state.lock().unwrap();
        [
            DeckId::DeckA,
            DeckId::DeckB,
            DeckId::SoundFx,
            DeckId::Aux1,
            DeckId::Aux2,
            DeckId::VoiceFx,
        ]
        .iter()
        .map(|&id| {
            let ch = rt.mixer.channel(id);
            VuEvent {
                channel: id.to_string(),
                left_db: ch.vu_left_db,
                right_db: ch.vu_right_db,
            }
        })
        .collect()
    }

    // ── Private helpers ───────────────────────────────────────────────────

    fn send_cmd(&mut self, cmd: EngineCmd) -> Result<(), String> {
        use ringbuf::traits::Producer as _;
        self.cmd_tx
            .try_push(cmd)
            .map_err(|_| "Command queue full".to_string())
    }

    fn build_stream(
        device: &Device,
        config: &StreamConfig,
        rt_arc: Arc<Mutex<RtState>>,
        mut cmd_cons: ringbuf::HeapCons<EngineCmd>,
    ) -> Result<Stream, String> {
        let err_fn = |e| log::error!("CPAL stream error: {e}");

        let stream = device
            .build_output_stream(
                config,
                move |output: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    audio_callback(output, &rt_arc, &mut cmd_cons);
                },
                err_fn,
                None,
            )
            .map_err(|e| format!("Build stream error: {e}"))?;

        Ok(stream)
    }
}

// SAFETY: AudioEngine holds a cpal::Stream which is !Send on some platforms.
// We ensure it is only accessed from the thread that created it (the main thread).
unsafe impl Send for AudioEngine {}

// ── Real-time audio callback ─────────────────────────────────────────────────
//
// This function runs on the CPAL real-time thread.
// Rules: no allocations (except first call to resize scratch bufs), no locks
// that could block, no I/O.
fn audio_callback(
    output: &mut [f32],
    rt_arc: &Arc<Mutex<RtState>>,
    cmd_cons: &mut ringbuf::HeapCons<EngineCmd>,
) {
    // Try to acquire the lock. If it's held by the main thread, output silence
    // to avoid glitching rather than blocking the RT thread.
    let mut rt = match rt_arc.try_lock() {
        Ok(g) => g,
        Err(_) => {
            output.fill(0.0);
            return;
        }
    };

    // Process pending commands (non-blocking)
    process_commands(&mut rt, cmd_cons);

    let len = output.len();

    // Resize scratch buffers if needed (only happens on first call or config change)
    if rt.buf_deck_a.len() != len {
        rt.buf_deck_a.resize(len, 0.0);
        rt.buf_deck_b.resize(len, 0.0);
        rt.buf_sound_fx.resize(len, 0.0);
        rt.buf_aux1.resize(len, 0.0);
        rt.buf_aux2.resize(len, 0.0);
        rt.buf_voice_fx.resize(len, 0.0);
        rt.buf_silence.resize(len, 0.0);
    }
    rt.buf_silence.fill(0.0);

    // ── Crossfade gain computation ──────────────────────────────────────
    let frames = (len / 2) as u64;
    // Capture endpoints before advance so completion handling can still stop
    // outgoing / promote incoming on the exact callback where fade reaches 100%.
    let (outgoing_id, incoming_id) = (rt.crossfade.outgoing(), rt.crossfade.incoming());
    let crossfade_active = rt.crossfade.is_fading();
    let (xf_gain_out, xf_gain_in, mut xf_complete) = rt.crossfade.advance(frames);
    let manual_pos = rt.manual_crossfade_pos.clamp(-1.0, 1.0);
    let manual_gain_a = ((1.0 - manual_pos) * 0.5).clamp(0.0, 1.0);
    let manual_gain_b = ((1.0 + manual_pos) * 0.5).clamp(0.0, 1.0);
    let a_live = rt
        .decks
        .get(&DeckId::DeckA)
        .map(|d| matches!(d.state, DeckState::Playing | DeckState::Crossfading))
        .unwrap_or(false);
    let b_live = rt
        .decks
        .get(&DeckId::DeckB)
        .map(|d| matches!(d.state, DeckState::Playing | DeckState::Crossfading))
        .unwrap_or(false);

    // ── Fill per-deck buffers ────────────────────────────────────────────
    // Cache device_sr before the loop — borrowing rt.sample_rate while
    // rt.decks is mutably borrowed triggers E0502.
    let device_sr = rt.sample_rate;
    let mut force_crossfade_complete = false;
    for (id, buf) in [
        (DeckId::DeckA, &mut rt.buf_deck_a as *mut Vec<f32>),
        (DeckId::DeckB, &mut rt.buf_deck_b as *mut Vec<f32>),
        (DeckId::SoundFx, &mut rt.buf_sound_fx as *mut Vec<f32>),
        (DeckId::Aux1, &mut rt.buf_aux1 as *mut Vec<f32>),
        (DeckId::Aux2, &mut rt.buf_aux2 as *mut Vec<f32>),
        (DeckId::VoiceFx, &mut rt.buf_voice_fx as *mut Vec<f32>),
    ] {
        let buf = unsafe { &mut *buf };
        if let Some(deck) = rt.decks.get_mut(&id) {
            if crossfade_active {
                // Active auto/timed fade owns Deck A/B crossfade gain.
                if Some(id) == outgoing_id {
                    deck.xfade_gain = xf_gain_out;
                } else if Some(id) == incoming_id {
                    deck.xfade_gain = xf_gain_in;
                } else if matches!(id, DeckId::DeckA | DeckId::DeckB) {
                    deck.xfade_gain = 0.0;
                } else {
                    deck.xfade_gain = 1.0;
                }
            } else {
                // Manual crossfader when no active auto/timed fade.
                // If only one deck is live, force that deck audible to avoid
                // accidental silence from stale slider position.
                match id {
                    DeckId::DeckA => {
                        deck.xfade_gain = if a_live ^ b_live {
                            if a_live {
                                1.0
                            } else {
                                0.0
                            }
                        } else {
                            manual_gain_a
                        };
                    }
                    DeckId::DeckB => {
                        deck.xfade_gain = if a_live ^ b_live {
                            if b_live {
                                1.0
                            } else {
                                0.0
                            }
                        } else {
                            manual_gain_b
                        };
                    }
                    _ => deck.xfade_gain = 1.0,
                }
            }
            deck.fill_buffer(buf, device_sr);
            if matches!(deck.state, DeckState::Playing | DeckState::Crossfading) && deck.is_eof() {
                if crossfade_active && Some(id) == outgoing_id {
                    force_crossfade_complete = true;
                }
                deck.mark_eof_stop();
            }
        } else {
            buf.fill(0.0);
        }
    }
    if force_crossfade_complete {
        xf_complete = true;
    }

    // ── Per-channel DSP (EQ → AGC → Compressor) ─────────────────────────
    for (id, buf) in [
        (DeckId::DeckA, &mut rt.buf_deck_a as *mut Vec<f32>),
        (DeckId::DeckB, &mut rt.buf_deck_b as *mut Vec<f32>),
        (DeckId::SoundFx, &mut rt.buf_sound_fx as *mut Vec<f32>),
        (DeckId::Aux1, &mut rt.buf_aux1 as *mut Vec<f32>),
        (DeckId::Aux2, &mut rt.buf_aux2 as *mut Vec<f32>),
        (DeckId::VoiceFx, &mut rt.buf_voice_fx as *mut Vec<f32>),
    ] {
        let buf = unsafe { &mut *buf };
        if let Some(pipeline) = rt.pipelines.get_mut(&id) {
            pipeline.process(buf);
        }
    }

    // ── Mix into master ──────────────────────────────────────────────────
    // SAFETY: we hold an exclusive &mut RtState from try_lock().
    // The buf_* fields and mixer.mix_into are disjoint fields; no aliasing.
    let (a, b, sfx, aux1, aux2, vfx, silence) = unsafe {
        (
            std::slice::from_raw_parts(rt.buf_deck_a.as_ptr(), rt.buf_deck_a.len()),
            std::slice::from_raw_parts(rt.buf_deck_b.as_ptr(), rt.buf_deck_b.len()),
            std::slice::from_raw_parts(rt.buf_sound_fx.as_ptr(), rt.buf_sound_fx.len()),
            std::slice::from_raw_parts(rt.buf_aux1.as_ptr(), rt.buf_aux1.len()),
            std::slice::from_raw_parts(rt.buf_aux2.as_ptr(), rt.buf_aux2.len()),
            std::slice::from_raw_parts(rt.buf_voice_fx.as_ptr(), rt.buf_voice_fx.len()),
            std::slice::from_raw_parts(rt.buf_silence.as_ptr(), rt.buf_silence.len()),
        )
    };
    let cue_a = rt
        .cue_preview_enabled
        .get(&DeckId::DeckA)
        .copied()
        .unwrap_or(false)
        && rt.cue_split_active;
    let cue_b = rt
        .cue_preview_enabled
        .get(&DeckId::DeckB)
        .copied()
        .unwrap_or(false)
        && rt.cue_split_active;
    let a_mix = if cue_a { silence } else { a };
    let b_mix = if cue_b { silence } else { b };
    rt.mixer
        .mix_into(output, a_mix, b_mix, sfx, aux1, aux2, vfx);
    if (rt.master_level - 1.0).abs() > 1e-6 {
        for s in output.iter_mut() {
            *s *= rt.master_level;
        }
    }

    // ── Master DSP (limiter / output chain) ─────────────────────────────
    rt.master_pipeline.process(output);

    // ── Feed encoder ring buffer ─────────────────────────────────────────
    use ringbuf::traits::Producer as _;
    for &s in output.iter() {
        let _ = rt.encoder_prod.try_push(s);
    }

    // ── Handle crossfade completion ──────────────────────────────────────
    if xf_complete {
        rt.crossfade.reset();
        // Restore full gain on the new active deck
        if let Some(id) = incoming_id {
            if let Some(deck) = rt.decks.get_mut(&id) {
                deck.xfade_gain = 1.0;
            }
        }
        if let Some(id) = outgoing_id {
            if let Some(deck) = rt.decks.get_mut(&id) {
                deck.stop_with_completion();
            }
        }
        if let Some(id) = incoming_id {
            rt.manual_crossfade_pos = if id == DeckId::DeckB { 1.0 } else { -1.0 };
        }
    }

    // ── Auto-detect crossfade trigger ───────────────────────────────────
    let autodj_mode = crate::scheduler::autodj::get_dj_mode();
    if rt.crossfade.is_idle()
        && rt.crossfade_config.auto_detect_enabled
        && autodj_mode != crate::scheduler::autodj::DjMode::AutoDj
    {
        check_auto_crossfade(&mut rt);
    }
}

/// Drain pending commands from the ring buffer and apply them to `rt`.
fn process_commands(rt: &mut RtState, cmd_cons: &mut ringbuf::HeapCons<EngineCmd>) {
    use ringbuf::traits::Consumer as _;

    while let Some(cmd) = cmd_cons.try_pop() {
        match cmd {
            EngineCmd::AttachPreparedTrack { deck, prepared, op } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.request_attach(prepared, op);
                }
            }
            EngineCmd::Play(deck) => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.play();
                }
            }
            EngineCmd::Pause(deck) => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.pause();
                }
            }
            EngineCmd::StopWithCompletion(deck) => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.stop_with_completion();
                }
            }
            EngineCmd::SetGain { deck, gain } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.channel_gain = gain.clamp(0.0, 1.0);
                }
            }
            EngineCmd::SetDeckPitch { deck, pct } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.set_pitch_pct(pct);
                }
            }
            EngineCmd::SetDeckTempo { deck, pct } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.set_tempo_pct(pct);
                }
            }
            EngineCmd::SetDeckLoop {
                deck,
                start_ms,
                end_ms,
            } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    if let Err(err) = d.set_loop_range_ms(start_ms, end_ms) {
                        log::warn!("set_loop_range_ms failed for {deck}: {err}");
                    }
                }
            }
            EngineCmd::ClearDeckLoop(deck) => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.clear_loop();
                }
            }
            EngineCmd::StartCrossfade { outgoing, incoming } => {
                if rt.crossfade.is_fading() {
                    continue;
                }
                let Some((outgoing, incoming)) = resolve_crossfade_pair(rt, outgoing, incoming)
                else {
                    log::warn!("Ignoring start_crossfade: no valid outgoing/incoming deck pair");
                    continue;
                };
                let config = rt.crossfade_config.clone();
                let mut config = config;
                cap_fade_window_to_outgoing_remaining(rt, outgoing, &mut config);
                rt.crossfade = CrossfadeState::start(outgoing, incoming, config, rt.sample_rate);
                if let Some(d) = rt.decks.get_mut(&outgoing) {
                    d.set_crossfading();
                }
                if let Some(d) = rt.decks.get_mut(&incoming) {
                    d.play();
                }
            }
            EngineCmd::SetCrossfadeConfig(config) => {
                rt.crossfade_config = config;
            }
            EngineCmd::SetManualCrossfade { position } => {
                rt.manual_crossfade_pos = position.clamp(-1.0, 1.0);
            }
            EngineCmd::TriggerManualFade {
                direction,
                duration_ms,
            } => {
                if rt.crossfade.is_fading() {
                    continue;
                }
                let (requested_outgoing, requested_incoming) = match direction {
                    ManualFadeDirection::AtoB => (DeckId::DeckA, DeckId::DeckB),
                    ManualFadeDirection::BtoA => (DeckId::DeckB, DeckId::DeckA),
                };
                let Some((outgoing, incoming)) =
                    resolve_crossfade_pair(rt, requested_outgoing, requested_incoming)
                else {
                    log::warn!("Ignoring manual fade: no valid outgoing/incoming deck pair");
                    continue;
                };
                let mut config = rt.crossfade_config.clone();
                config.fade_out_time_ms = duration_ms.max(100);
                config.fade_in_time_ms = duration_ms.max(100);
                cap_fade_window_to_outgoing_remaining(rt, outgoing, &mut config);
                rt.crossfade = CrossfadeState::start(outgoing, incoming, config, rt.sample_rate);
                if let Some(d) = rt.decks.get_mut(&outgoing) {
                    d.set_crossfading();
                }
                if let Some(d) = rt.decks.get_mut(&incoming) {
                    d.play();
                }
            }
            EngineCmd::SetChannelPipeline { deck, settings } => {
                if let Some(p) = rt.pipelines.get_mut(&deck) {
                    *p = ChannelPipeline::from_settings(rt.sample_rate as f32, settings);
                }
            }
            EngineCmd::SetMasterPipeline { settings } => {
                rt.master_pipeline =
                    ChannelPipeline::from_settings(rt.sample_rate as f32, settings);
            }
            EngineCmd::SetDeckCuePreview { deck, enabled } => {
                if matches!(deck, DeckId::DeckA | DeckId::DeckB) {
                    let effective = if rt.cue_split_active { enabled } else { false };
                    rt.cue_preview_enabled.insert(deck, effective);
                }
            }
            EngineCmd::SetMonitorRoutingConfig(config) => {
                rt.cue_split_active = config
                    .cue_device_id
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty());
                rt.cue_level = config.cue_level.clamp(0.0, 2.0);
                rt.master_level = config.master_level.clamp(0.0, 2.0);
                if !rt.cue_split_active {
                    rt.cue_preview_enabled.insert(DeckId::DeckA, false);
                    rt.cue_preview_enabled.insert(DeckId::DeckB, false);
                }
            }
        }
    }
}

#[inline]
fn is_playing_like(state: &DeckState) -> bool {
    matches!(state, DeckState::Playing | DeckState::Crossfading)
}

#[inline]
fn is_loaded_like(state: &DeckState) -> bool {
    matches!(
        state,
        DeckState::Ready | DeckState::Paused | DeckState::Playing | DeckState::Crossfading
    )
}

// Resolve stale or inverted crossfade requests against actual deck runtime state.
// This keeps direction consistent when UI/backend are briefly out of sync.
fn resolve_crossfade_pair(
    rt: &RtState,
    outgoing: DeckId,
    incoming: DeckId,
) -> Option<(DeckId, DeckId)> {
    if outgoing == incoming {
        return None;
    }

    let out_state = rt.decks.get(&outgoing).map(|d| d.state.clone())?;
    let in_state = rt.decks.get(&incoming).map(|d| d.state.clone())?;

    if is_playing_like(&out_state) && is_loaded_like(&in_state) {
        return Some((outgoing, incoming));
    }
    if is_playing_like(&in_state) && is_loaded_like(&out_state) {
        return Some((incoming, outgoing));
    }

    let a_state = rt.decks.get(&DeckId::DeckA).map(|d| d.state.clone())?;
    let b_state = rt.decks.get(&DeckId::DeckB).map(|d| d.state.clone())?;

    if is_playing_like(&a_state) && is_loaded_like(&b_state) {
        Some((DeckId::DeckA, DeckId::DeckB))
    } else if is_playing_like(&b_state) && is_loaded_like(&a_state) {
        Some((DeckId::DeckB, DeckId::DeckA))
    } else {
        None
    }
}

// Prevent long fade windows from outlasting the outgoing deck's remaining time.
// This avoids "incoming only appears at the very end" behavior when the trigger
// fires late in the song.
fn cap_fade_window_to_outgoing_remaining(
    rt: &RtState,
    outgoing: DeckId,
    config: &mut CrossfadeConfig,
) {
    let remaining_ms = rt
        .decks
        .get(&outgoing)
        .map(|d| d.remaining_ms())
        .unwrap_or(0);
    if remaining_ms == 0 {
        return;
    }
    let cap_ms = remaining_ms.min(u32::MAX as u64) as u32;
    config.fade_out_time_ms = config.fade_out_time_ms.min(cap_ms).max(1);
    config.fade_in_time_ms = config.fade_in_time_ms.min(cap_ms).max(1);
    config.min_fade_time_ms = config.min_fade_time_ms.min(cap_ms).max(1);
    config.max_fade_time_ms = config.max_fade_time_ms.min(cap_ms);
    if config.max_fade_time_ms < config.min_fade_time_ms {
        config.max_fade_time_ms = config.min_fade_time_ms;
    }
}

/// Check if the active deck's RMS has dropped below the auto-detect threshold.
fn check_auto_crossfade(rt: &mut RtState) {
    let cfg = &rt.crossfade_config;
    if !cfg.auto_detect_enabled || cfg.trigger_mode != CrossfadeTriggerMode::AutoDetectDb {
        return;
    }

    // Find the currently playing deck with the lowest remaining time within window
    let trigger_deck = [DeckId::DeckA, DeckId::DeckB]
        .iter()
        .find(|&&id| {
            let deck = match rt.decks.get(&id) {
                Some(d) => d,
                None => return false,
            };
            if deck.state != DeckState::Playing {
                return false;
            }
            let remaining = deck.remaining_ms();
            remaining > 0
                && remaining <= cfg.auto_detect_max_ms as u64
                && deck.position_ms() >= cfg.auto_detect_min_ms as u64
        })
        .copied();

    if let Some(outgoing) = trigger_deck {
        let incoming = match outgoing {
            DeckId::DeckA => DeckId::DeckB,
            _ => DeckId::DeckA,
        };
        if rt
            .decks
            .get(&incoming)
            .map(|d| d.state == DeckState::Ready)
            .unwrap_or(false)
        {
            let config = rt.crossfade_config.clone();
            rt.crossfade = CrossfadeState::start(outgoing, incoming, config, rt.sample_rate);
            if let Some(d) = rt.decks.get_mut(&outgoing) {
                d.set_crossfading();
            }
            if let Some(d) = rt.decks.get_mut(&incoming) {
                d.play();
            }
        }
    }
}
