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

use super::{
    crossfade::{CrossfadeConfig, CrossfadeState, DeckId},
    deck::{Deck, DeckState},
    dsp::pipeline::{ChannelPipeline, PipelineSettings},
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
    sample_rate: u32,
    // Per-channel scratch buffers (avoid alloc in callback)
    buf_deck_a: Vec<f32>,
    buf_deck_b: Vec<f32>,
    buf_sound_fx: Vec<f32>,
    buf_aux1: Vec<f32>,
    buf_voice_fx: Vec<f32>,
    // Encoder ring buffer producer (to stream/icecast thread)
    encoder_prod: ringbuf::HeapProd<f32>,
}

/// Commands sent from the main thread → real-time thread via a lock-free channel.
/// Kept small; heavy state lives in `AudioEngine` behind the Mutex.
enum EngineCmd {
    LoadTrack { deck: DeckId, path: PathBuf, song_id: Option<i64> },
    Play(DeckId),
    Pause(DeckId),
    Seek { deck: DeckId, position_ms: u64 },
    SetGain { deck: DeckId, gain: f32 },
    StartCrossfade { outgoing: DeckId, incoming: DeckId },
    SetCrossfadeConfig(CrossfadeConfig),
    SetChannelPipeline { deck: DeckId, settings: PipelineSettings },
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
                m.insert(DeckId::VoiceFx, Deck::new(DeckId::VoiceFx));
                m
            },
            pipelines: {
                let mut m = HashMap::new();
                for id in [DeckId::DeckA, DeckId::DeckB, DeckId::SoundFx, DeckId::Aux1, DeckId::VoiceFx] {
                    m.insert(id, ChannelPipeline::new(sample_rate as f32));
                }
                m
            },
            master_pipeline: ChannelPipeline::new(sample_rate as f32),
            mixer: Mixer::new(),
            crossfade: CrossfadeState::default(),
            crossfade_config: CrossfadeConfig::default(),
            sample_rate,
            buf_deck_a: Vec::new(),
            buf_deck_b: Vec::new(),
            buf_sound_fx: Vec::new(),
            buf_aux1: Vec::new(),
            buf_voice_fx: Vec::new(),
            encoder_prod: enc_prod,
        }));

        let rt_arc_cb = Arc::clone(&rt_arc);

        let stream = Self::build_stream(&device, &config.into(), rt_arc_cb, cmd_cons)?;
        stream.play().map_err(|e| format!("Stream play error: {e}"))?;

        Ok(Self {
            _stream: Some(stream),
            encoder_consumer: Some(enc_cons),
            cmd_tx: cmd_prod,
            rt_state: rt_arc,
            sample_rate,
        })
    }

    // ── Public control API ────────────────────────────────────────────────

    pub fn load_track(&mut self, deck: DeckId, path: PathBuf, song_id: Option<i64>) -> Result<(), String> {
        self.send_cmd(EngineCmd::LoadTrack { deck, path, song_id })
    }

    pub fn play(&mut self, deck: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::Play(deck))
    }

    pub fn pause(&mut self, deck: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::Pause(deck))
    }

    pub fn seek(&mut self, deck: DeckId, position_ms: u64) -> Result<(), String> {
        self.send_cmd(EngineCmd::Seek { deck, position_ms })
    }

    pub fn set_channel_gain(&mut self, deck: DeckId, gain: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetGain { deck, gain })
    }

    pub fn start_crossfade(&mut self, outgoing: DeckId, incoming: DeckId) -> Result<(), String> {
        self.send_cmd(EngineCmd::StartCrossfade { outgoing, incoming })
    }

    pub fn set_crossfade_config(&mut self, config: CrossfadeConfig) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetCrossfadeConfig(config))
    }

    pub fn set_channel_pipeline(&mut self, deck: DeckId, settings: PipelineSettings) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetChannelPipeline { deck, settings })
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
        })
    }

    pub fn get_vu_readings(&self) -> Vec<VuEvent> {
        let rt = self.rt_state.lock().unwrap();
        [DeckId::DeckA, DeckId::DeckB, DeckId::SoundFx, DeckId::Aux1, DeckId::VoiceFx]
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
        self.cmd_tx.try_push(cmd).map_err(|_| "Command queue full".to_string())
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
        rt.buf_voice_fx.resize(len, 0.0);
    }

    // ── Crossfade gain computation ──────────────────────────────────────
    let frames = (len / 2) as u64;
    let (xf_gain_out, xf_gain_in, xf_complete) = rt.crossfade.advance(frames);
    let (outgoing_id, incoming_id) = (rt.crossfade.outgoing(), rt.crossfade.incoming());

    // ── Fill per-deck buffers ────────────────────────────────────────────
    for (id, buf) in [
        (DeckId::DeckA, &mut rt.buf_deck_a as *mut Vec<f32>),
        (DeckId::DeckB, &mut rt.buf_deck_b as *mut Vec<f32>),
        (DeckId::SoundFx, &mut rt.buf_sound_fx as *mut Vec<f32>),
        (DeckId::Aux1, &mut rt.buf_aux1 as *mut Vec<f32>),
        (DeckId::VoiceFx, &mut rt.buf_voice_fx as *mut Vec<f32>),
    ] {
        let buf = unsafe { &mut *buf };
        if let Some(deck) = rt.decks.get_mut(&id) {
            // Apply crossfade gain to outgoing/incoming decks
            if Some(id) == outgoing_id {
                deck.gain = xf_gain_out;
            } else if Some(id) == incoming_id {
                deck.gain = xf_gain_in;
            }
            deck.fill_buffer(buf);
        } else {
            buf.fill(0.0);
        }
    }

    // ── Per-channel DSP (EQ → AGC → Compressor) ─────────────────────────
    for (id, buf) in [
        (DeckId::DeckA, &mut rt.buf_deck_a as *mut Vec<f32>),
        (DeckId::DeckB, &mut rt.buf_deck_b as *mut Vec<f32>),
        (DeckId::SoundFx, &mut rt.buf_sound_fx as *mut Vec<f32>),
        (DeckId::Aux1, &mut rt.buf_aux1 as *mut Vec<f32>),
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
    let (a, b, sfx, aux, vfx) = unsafe {
        (
            std::slice::from_raw_parts(rt.buf_deck_a.as_ptr(), rt.buf_deck_a.len()),
            std::slice::from_raw_parts(rt.buf_deck_b.as_ptr(), rt.buf_deck_b.len()),
            std::slice::from_raw_parts(rt.buf_sound_fx.as_ptr(), rt.buf_sound_fx.len()),
            std::slice::from_raw_parts(rt.buf_aux1.as_ptr(), rt.buf_aux1.len()),
            std::slice::from_raw_parts(rt.buf_voice_fx.as_ptr(), rt.buf_voice_fx.len()),
        )
    };
    rt.mixer.mix_into(output, a, b, sfx, aux, vfx);

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
                deck.gain = 1.0;
            }
        }
        if let Some(id) = outgoing_id {
            if let Some(deck) = rt.decks.get_mut(&id) {
                deck.stop();
            }
        }
    }

    // ── Auto-detect crossfade trigger ───────────────────────────────────
    if rt.crossfade.is_idle() && rt.crossfade_config.auto_detect_enabled {
        check_auto_crossfade(&mut rt);
    }
}

/// Drain pending commands from the ring buffer and apply them to `rt`.
fn process_commands(rt: &mut RtState, cmd_cons: &mut ringbuf::HeapCons<EngineCmd>) {
    use ringbuf::traits::Consumer as _;

    while let Some(cmd) = cmd_cons.try_pop() {
        match cmd {
            EngineCmd::LoadTrack { deck, path, song_id } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    if let Err(e) = d.load(path, song_id) {
                        log::warn!("Load error on {deck}: {e}");
                    }
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
            EngineCmd::Seek { deck, position_ms } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    if let Err(e) = d.seek(position_ms) {
                        log::warn!("Seek error: {e}");
                    }
                }
            }
            EngineCmd::SetGain { deck, gain } => {
                if let Some(d) = rt.decks.get_mut(&deck) {
                    d.gain = gain;
                }
            }
            EngineCmd::StartCrossfade { outgoing, incoming } => {
                let config = rt.crossfade_config.clone();
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
            EngineCmd::SetChannelPipeline { deck, settings } => {
                if let Some(p) = rt.pipelines.get_mut(&deck) {
                    *p = ChannelPipeline::from_settings(rt.sample_rate as f32, settings);
                }
            }
        }
    }
}

/// Check if the active deck's RMS has dropped below the auto-detect threshold.
fn check_auto_crossfade(rt: &mut RtState) {
    let cfg = &rt.crossfade_config;
    if !cfg.auto_detect_enabled {
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
        if rt.decks.get(&incoming).map(|d| d.state == DeckState::Ready).unwrap_or(false) {
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
