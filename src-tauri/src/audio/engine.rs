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
    device_manager::{self, AudioOutputMode, AudioOutputRoutingConfig, AudioOutputStatus},
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
    pub channel_gain: f32,
    pub bass_db: f32,
    pub filter_amount: f32,
    pub master_level: f32,
    pub decoder_buffer_ms: u64,
    pub rms_db_pre_fader: f32,
    pub cue_preview_enabled: bool,
    pub loop_enabled: bool,
    pub loop_start_ms: Option<u64>,
    pub loop_end_ms: Option<u64>,
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
    deck_bass_db: HashMap<DeckId, f32>,
    deck_filter_amount: HashMap<DeckId, f32>,
    cue_preview_enabled: HashMap<DeckId, bool>,
    cue_split_active: bool,
    cue_available: bool,
    cue_level: f32,
    headphone_mix: f32,
    master_level: f32,
    local_monitor_muted: bool,
    sample_rate: u32,
    output_channels: usize,
    // Per-channel scratch buffers (avoid alloc in callback)
    buf_deck_a: Vec<f32>,
    buf_deck_b: Vec<f32>,
    buf_deck_a_cue_tap: Vec<f32>,
    buf_deck_b_cue_tap: Vec<f32>,
    buf_sound_fx: Vec<f32>,
    buf_aux1: Vec<f32>,
    buf_aux2: Vec<f32>,
    buf_voice_fx: Vec<f32>,
    buf_silence: Vec<f32>,
    buf_master: Vec<f32>,
    buf_cue: Vec<f32>,
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
    SetDeckBass {
        deck: DeckId,
        bass_db: f32,
    },
    SetDeckFilter {
        deck: DeckId,
        amount: f32,
    },
    SetMasterLevel {
        level: f32,
    },
    SetLocalMonitorMuted {
        muted: bool,
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
    SetHeadphoneMix {
        value: f32,
    },
    SetHeadphoneLevel {
        value: f32,
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
    routing_config: AudioOutputRoutingConfig,
    output_status: AudioOutputStatus,
    #[allow(dead_code)]
    sample_rate: u32,
}

impl AudioEngine {
    const ENCODER_RING_SIZE: usize = 44100 * 2 * 10; // 10 s encoder buffer
    const CMD_RING_SIZE: usize = 64;

    /// Initialise and start the CPAL output stream.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let default_name = host
            .default_output_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();
        let device = host
            .default_output_device()
            .ok_or("No default audio output device found")?;

        let config = device
            .default_output_config()
            .map_err(|e| format!("Default config error: {e}"))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let device_name = device.name().unwrap_or_default();
        let device_id = device_manager::list_audio_output_devices()
            .ok()
            .and_then(|devices| {
                devices
                    .into_iter()
                    .find(|d| d.name == device_name || (d.is_default && d.name == default_name))
                    .map(|d| d.id)
            });

        log::info!(
            "Audio device: {} | sample rate: {} | channels: {}",
            device_name,
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
            deck_bass_db: {
                let mut m = HashMap::new();
                m.insert(DeckId::DeckA, 0.0);
                m.insert(DeckId::DeckB, 0.0);
                m
            },
            deck_filter_amount: {
                let mut m = HashMap::new();
                m.insert(DeckId::DeckA, 0.0);
                m.insert(DeckId::DeckB, 0.0);
                m
            },
            cue_preview_enabled: {
                let mut m = HashMap::new();
                m.insert(DeckId::DeckA, false);
                m.insert(DeckId::DeckB, false);
                m
            },
            cue_split_active: false,
            cue_available: channels >= 4,
            cue_level: 1.0,
            headphone_mix: -1.0,
            master_level: 1.0,
            local_monitor_muted: false,
            sample_rate,
            output_channels: channels.max(2),
            buf_deck_a: Vec::new(),
            buf_deck_b: Vec::new(),
            buf_deck_a_cue_tap: Vec::new(),
            buf_deck_b_cue_tap: Vec::new(),
            buf_sound_fx: Vec::new(),
            buf_aux1: Vec::new(),
            buf_aux2: Vec::new(),
            buf_voice_fx: Vec::new(),
            buf_silence: Vec::new(),
            buf_master: Vec::new(),
            buf_cue: Vec::new(),
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
            routing_config: AudioOutputRoutingConfig::default(),
            output_status: AudioOutputStatus {
                active_mode: if channels >= 4 {
                    AudioOutputMode::SingleDeviceFourChannel
                } else {
                    AudioOutputMode::SingleDeviceStereo
                },
                master_device_id: device_id.clone(),
                master_device_name: Some(device_name),
                cue_device_id: device_id,
                cue_available: channels >= 4,
                fallback_active: false,
                last_error: None,
            },
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

    pub fn set_deck_bass(&mut self, deck: DeckId, bass_db: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetDeckBass {
            deck,
            bass_db: bass_db.clamp(-12.0, 12.0),
        })
    }

    pub fn set_deck_filter(&mut self, deck: DeckId, amount: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetDeckFilter {
            deck,
            amount: amount.clamp(-1.0, 1.0),
        })
    }

    pub fn set_master_level(&mut self, level: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetMasterLevel {
            level: level.clamp(0.0, 1.0),
        })
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

    pub fn set_headphone_mix(&mut self, value: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetHeadphoneMix {
            value: value.clamp(-1.0, 1.0),
        })
    }

    pub fn set_headphone_level(&mut self, value: f32) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetHeadphoneLevel {
            value: value.clamp(0.0, 1.0),
        })
    }

    pub fn set_monitor_routing_config(&mut self, config: MonitorRoutingConfig) {
        self.routing_config.mode = match config.cue_mix_mode.as_str() {
            "single_device_four_channel" => AudioOutputMode::SingleDeviceFourChannel,
            "dual_device_split" => AudioOutputMode::DualDeviceSplit,
            _ => AudioOutputMode::SingleDeviceStereo,
        };
        self.routing_config.master_device_id = config.master_device_id.clone();
        self.routing_config.cue_device_id = config.cue_device_id.clone();
        self.routing_config.auto_fallback = config.auto_fallback;
        let _ = self.send_cmd(EngineCmd::SetMonitorRoutingConfig(config));
    }

    pub fn list_audio_output_devices() -> Result<Vec<device_manager::AudioOutputDevice>, String> {
        device_manager::list_audio_output_devices()
    }

    pub fn get_audio_output_status(&self) -> AudioOutputStatus {
        self.output_status.clone()
    }

    pub fn apply_audio_output_routing(
        &mut self,
        mut config: AudioOutputRoutingConfig,
    ) -> Result<AudioOutputStatus, String> {
        let original = config.clone();
        let selected = device_manager::select_output_stream(&config);
        let (selection, warning, had_explicit_selection) = match selected {
            Ok(ok) => ok,
            Err(err) => {
                if !config.auto_fallback {
                    return Err(err);
                }
                config.mode = AudioOutputMode::SingleDeviceStereo;
                config.master_device_id = None;
                let (fallback_sel, warn, _) = device_manager::select_output_stream(&config)?;
                let fallback_warning = Some(format!("{err}; fallback engaged"));
                (
                    fallback_sel,
                    fallback_warning.or(warn),
                    had_explicit_selection(&original),
                )
            }
        };

        let current_channels = {
            let rt = self.rt_state.lock().unwrap();
            rt.output_channels
        };
        let should_rebuild = self.output_status.master_device_id.as_deref()
            != Some(selection.device_id.as_str())
            || self.sample_rate != selection.config.sample_rate.0
            || current_channels != selection.config.channels as usize;

        if should_rebuild {
            self.rebuild_stream(selection.device, &selection.config)?;
        }

        {
            let mut rt = self.rt_state.lock().unwrap();
            rt.sample_rate = selection.config.sample_rate.0;
            rt.output_channels = selection.config.channels as usize;
            rt.cue_available = selection.cue_available;
            let wants_split = matches!(
                config.mode,
                AudioOutputMode::SingleDeviceFourChannel | AudioOutputMode::DualDeviceSplit
            );
            rt.cue_split_active = wants_split && rt.cue_available;
            if !rt.cue_available {
                rt.cue_preview_enabled.insert(DeckId::DeckA, false);
                rt.cue_preview_enabled.insert(DeckId::DeckB, false);
            }
        }

        self.sample_rate = selection.config.sample_rate.0;
        self.routing_config = config.clone();
        let status = AudioOutputStatus {
            active_mode: selection.active_mode,
            master_device_id: Some(selection.device_id.clone()),
            master_device_name: Some(selection.device_name.clone()),
            cue_device_id: if selection.cue_available {
                Some(selection.device_id.clone())
            } else {
                None
            },
            cue_available: selection.cue_available,
            fallback_active: warning.is_some()
                || (had_explicit_selection && !selection.cue_available),
            last_error: warning,
        };
        self.output_status = status.clone();
        Ok(status)
    }

    pub fn maybe_auto_fallback_output(&mut self) -> Option<AudioOutputStatus> {
        if !self.routing_config.auto_fallback {
            return None;
        }
        let Some(active_id) = self.output_status.master_device_id.clone() else {
            return None;
        };
        let devices = device_manager::list_audio_output_devices().ok()?;
        if devices.iter().any(|d| d.id == active_id) {
            return None;
        }
        let mut fallback_cfg = self.routing_config.clone();
        fallback_cfg.mode = AudioOutputMode::SingleDeviceStereo;
        fallback_cfg.master_device_id = None;
        match self.apply_audio_output_routing(fallback_cfg) {
            Ok(mut status) => {
                status.fallback_active = true;
                status.last_error = Some(
                    "Active output device disconnected; switched to default output".to_string(),
                );
                self.output_status = status.clone();
                Some(status)
            }
            Err(_) => None,
        }
    }

    pub fn get_crossfade_config(&self) -> CrossfadeConfig {
        self.rt_state.lock().unwrap().crossfade_config.clone()
    }

    pub fn get_deck_state(&self, deck: DeckId) -> Option<DeckStateEvent> {
        let rt = self.rt_state.lock().unwrap();
        rt.decks.get(&deck).map(|d| {
            let loop_range = d.loop_range_ms();
            let (bass_db, filter_amount) = (
                rt.deck_bass_db.get(&deck).copied().unwrap_or(0.0),
                rt.deck_filter_amount.get(&deck).copied().unwrap_or(0.0),
            );
            DeckStateEvent {
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
                channel_gain: d.channel_gain,
                bass_db,
                filter_amount,
                master_level: rt.master_level,
                decoder_buffer_ms: d.decoder_buffered_ms(),
                rms_db_pre_fader: d.rms_db_pre_fader,
                cue_preview_enabled: rt.cue_preview_enabled.get(&deck).copied().unwrap_or(false),
                loop_enabled: loop_range.is_some(),
                loop_start_ms: loop_range.map(|(start, _)| start),
                loop_end_ms: loop_range.map(|(_, end)| end),
            }
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

    pub fn get_master_level(&self) -> f32 {
        self.rt_state.lock().unwrap().master_level
    }

    pub fn set_local_monitor_muted(&mut self, muted: bool) -> Result<(), String> {
        self.send_cmd(EngineCmd::SetLocalMonitorMuted { muted })
    }

    pub fn get_local_monitor_muted(&self) -> bool {
        self.rt_state.lock().unwrap().local_monitor_muted
    }

    pub fn get_output_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn get_headphone_mix(&self) -> f32 {
        self.rt_state.lock().unwrap().headphone_mix
    }

    pub fn get_headphone_level(&self) -> f32 {
        self.rt_state.lock().unwrap().cue_level
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
        let mut events: Vec<VuEvent> = [
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
        .collect();

        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        for frame in rt.buf_master.chunks_exact(2) {
            peak_l = peak_l.max(frame[0].abs());
            peak_r = peak_r.max(frame[1].abs());
        }
        if rt.buf_master.len() % 2 == 1 {
            peak_l = peak_l.max(rt.buf_master[rt.buf_master.len() - 1].abs());
        }

        let to_db = |linear: f32| {
            if linear < 1e-10 {
                -96.0
            } else {
                20.0 * linear.log10()
            }
        };

        events.push(VuEvent {
            channel: "master".to_string(),
            left_db: to_db(peak_l),
            right_db: to_db(peak_r),
        });

        events
    }

    // ── Private helpers ───────────────────────────────────────────────────

    fn send_cmd(&mut self, cmd: EngineCmd) -> Result<(), String> {
        use ringbuf::traits::Producer as _;
        self.cmd_tx
            .try_push(cmd)
            .map_err(|_| "Command queue full".to_string())
    }

    fn rebuild_stream(&mut self, device: Device, config: &StreamConfig) -> Result<(), String> {
        let cmd_rb = HeapRb::<EngineCmd>::new(Self::CMD_RING_SIZE);
        let (cmd_prod, cmd_cons) = cmd_rb.split();
        self.cmd_tx = cmd_prod;

        let rt_arc_cb = Arc::clone(&self.rt_state);
        let stream = Self::build_stream(&device, config, rt_arc_cb, cmd_cons)?;
        stream
            .play()
            .map_err(|e| format!("Stream play error: {e}"))?;
        self._stream = Some(stream);
        Ok(())
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

fn had_explicit_selection(config: &AudioOutputRoutingConfig) -> bool {
    config.master_device_id.is_some()
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

    let out_channels = rt.output_channels.max(2);
    if output.is_empty() || output.len() % out_channels != 0 {
        output.fill(0.0);
        return;
    }
    let render_frames = output.len() / out_channels;
    let stereo_len = render_frames * 2;

    // Resize scratch buffers if needed (only happens on first call or config change)
    if rt.buf_deck_a.len() != stereo_len {
        rt.buf_deck_a.resize(stereo_len, 0.0);
        rt.buf_deck_b.resize(stereo_len, 0.0);
        rt.buf_deck_a_cue_tap.resize(stereo_len, 0.0);
        rt.buf_deck_b_cue_tap.resize(stereo_len, 0.0);
        rt.buf_sound_fx.resize(stereo_len, 0.0);
        rt.buf_aux1.resize(stereo_len, 0.0);
        rt.buf_aux2.resize(stereo_len, 0.0);
        rt.buf_voice_fx.resize(stereo_len, 0.0);
        rt.buf_silence.resize(stereo_len, 0.0);
        rt.buf_master.resize(stereo_len, 0.0);
        rt.buf_cue.resize(stereo_len, 0.0);
    }
    rt.buf_silence.fill(0.0);
    rt.buf_master.fill(0.0);
    rt.buf_cue.fill(0.0);
    rt.buf_deck_a_cue_tap.fill(0.0);
    rt.buf_deck_b_cue_tap.fill(0.0);

    // ── Crossfade gain computation ──────────────────────────────────────
    let frames = render_frames as u64;
    // Capture endpoints before advance so completion handling can still stop
    // outgoing / promote incoming on the exact callback where fade reaches 100%.
    let (outgoing_id, incoming_id) = (rt.crossfade.outgoing(), rt.crossfade.incoming());
    let crossfade_active = rt.crossfade.is_fading();
    let (xf_gain_out, xf_gain_in, mut xf_complete) = rt.crossfade.advance(frames);
    let manual_pos = rt.manual_crossfade_pos.clamp(-1.0, 1.0);
    let manual_gain_a = ((1.0 - manual_pos) * 0.5).clamp(0.0, 1.0);
    let manual_gain_b = ((1.0 + manual_pos) * 0.5).clamp(0.0, 1.0);

    // ── Fill per-deck buffers ────────────────────────────────────────────
    // Cache device_sr before the loop — borrowing rt.sample_rate while
    // rt.decks is mutably borrowed triggers E0502.
    let device_sr = rt.sample_rate;
    let mut force_crossfade_complete = false;
    for (id, buf, cue_tap) in [
        (
            DeckId::DeckA,
            &mut rt.buf_deck_a as *mut Vec<f32>,
            Some(&mut rt.buf_deck_a_cue_tap as *mut Vec<f32>),
        ),
        (
            DeckId::DeckB,
            &mut rt.buf_deck_b as *mut Vec<f32>,
            Some(&mut rt.buf_deck_b_cue_tap as *mut Vec<f32>),
        ),
        (DeckId::SoundFx, &mut rt.buf_sound_fx as *mut Vec<f32>, None),
        (DeckId::Aux1, &mut rt.buf_aux1 as *mut Vec<f32>, None),
        (DeckId::Aux2, &mut rt.buf_aux2 as *mut Vec<f32>, None),
        (DeckId::VoiceFx, &mut rt.buf_voice_fx as *mut Vec<f32>, None),
    ] {
        let buf = unsafe { &mut *buf };
        let cue_tap = cue_tap.map(|ptr| unsafe { &mut *ptr });
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
                match id {
                    DeckId::DeckA => {
                        deck.xfade_gain = manual_gain_a;
                    }
                    DeckId::DeckB => {
                        deck.xfade_gain = manual_gain_b;
                    }
                    _ => deck.xfade_gain = 1.0,
                }
            }
            match cue_tap {
                Some(tap) => deck.fill_buffer_with_tap(buf, device_sr, Some(tap.as_mut_slice())),
                None => deck.fill_buffer(buf, device_sr),
            }
            if matches!(deck.state, DeckState::Playing | DeckState::Crossfading) && deck.is_eof() {
                if crossfade_active && Some(id) == outgoing_id {
                    force_crossfade_complete = true;
                }
                deck.mark_eof_stop();
            }
        } else {
            buf.fill(0.0);
            if let Some(tap) = cue_tap {
                tap.fill(0.0);
            }
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
    let (a, b, a_cue_tap, b_cue_tap, sfx, aux1, aux2, vfx, silence) = unsafe {
        (
            std::slice::from_raw_parts(rt.buf_deck_a.as_ptr(), rt.buf_deck_a.len()),
            std::slice::from_raw_parts(rt.buf_deck_b.as_ptr(), rt.buf_deck_b.len()),
            std::slice::from_raw_parts(rt.buf_deck_a_cue_tap.as_ptr(), rt.buf_deck_a_cue_tap.len()),
            std::slice::from_raw_parts(rt.buf_deck_b_cue_tap.as_ptr(), rt.buf_deck_b_cue_tap.len()),
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
        .unwrap_or(false);
    let cue_b = rt
        .cue_preview_enabled
        .get(&DeckId::DeckB)
        .copied()
        .unwrap_or(false);
    let split_available = rt.cue_split_active && rt.cue_available && out_channels >= 4;
    let a_mix = if !split_available && cue_a {
        silence
    } else {
        a
    };
    let b_mix = if !split_available && cue_b {
        silence
    } else {
        b
    };
    // SAFETY: mixer and buf_master are disjoint RtState fields.
    unsafe {
        let mixer = &mut *(&mut rt.mixer as *mut Mixer);
        let master = &mut *(&mut rt.buf_master as *mut Vec<f32>);
        mixer.mix_into(master, a_mix, b_mix, sfx, aux1, aux2, vfx);
    }
    let master_level = rt.master_level;
    if (master_level - 1.0).abs() > 1e-6 {
        for s in rt.buf_master.iter_mut() {
            *s *= master_level;
        }
    }

    // ── Master DSP (limiter / output chain) ─────────────────────────────
    // SAFETY: master_pipeline and buf_master are disjoint RtState fields.
    unsafe {
        let pipeline = &mut *(&mut rt.master_pipeline as *mut ChannelPipeline);
        let master = &mut *(&mut rt.buf_master as *mut Vec<f32>);
        pipeline.process(master);
    }

    // Build cue bus only when split output is available.
    if split_available {
        if cue_a {
            accumulate_stereo(&mut rt.buf_cue, a_cue_tap);
        }
        if cue_b {
            accumulate_stereo(&mut rt.buf_cue, b_cue_tap);
        }
        let master_blend = ((rt.headphone_mix + 1.0) * 0.5).clamp(0.0, 1.0);
        let cue_blend = 1.0 - master_blend;
        let cue_level = rt.cue_level;
        // SAFETY: immutable master slice and mutable cue slice point to disjoint buffers.
        let (master, cue) = unsafe {
            (
                std::slice::from_raw_parts(rt.buf_master.as_ptr(), rt.buf_master.len()),
                std::slice::from_raw_parts_mut(rt.buf_cue.as_mut_ptr(), rt.buf_cue.len()),
            )
        };
        for i in 0..cue.len() {
            cue[i] = (cue[i] * cue_blend + master[i] * master_blend) * cue_level;
        }
    }

    if rt.local_monitor_muted {
        output.fill(0.0);
    } else if split_available {
        for frame in 0..render_frames {
            let out_i = frame * out_channels;
            let src_i = frame * 2;
            output[out_i] = rt.buf_master[src_i];
            if out_channels > 1 {
                output[out_i + 1] = rt.buf_master[src_i + 1];
            }
            if out_channels > 2 {
                output[out_i + 2] = rt.buf_cue[src_i];
            }
            if out_channels > 3 {
                output[out_i + 3] = rt.buf_cue[src_i + 1];
            }
            for ch in 4..out_channels {
                output[out_i + ch] = 0.0;
            }
        }
    } else {
        for frame in 0..render_frames {
            let out_i = frame * out_channels;
            let src_i = frame * 2;
            output[out_i] = rt.buf_master[src_i];
            if out_channels > 1 {
                output[out_i + 1] = rt.buf_master[src_i + 1];
            }
            for ch in 2..out_channels {
                output[out_i + ch] = 0.0;
            }
        }
    }

    // ── Feed encoder ring buffer ─────────────────────────────────────────
    use ringbuf::traits::Producer as _;
    let master_ptr = rt.buf_master.as_ptr();
    let master_len = rt.buf_master.len();
    for i in 0..master_len {
        // SAFETY: master_ptr is valid for master_len for this callback scope.
        let s = unsafe { *master_ptr.add(i) };
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
            EngineCmd::SetDeckBass { deck, bass_db } => {
                rt.deck_bass_db.insert(deck, bass_db.clamp(-12.0, 12.0));
                apply_deck_tone(rt, deck);
            }
            EngineCmd::SetDeckFilter { deck, amount } => {
                rt.deck_filter_amount.insert(deck, amount.clamp(-1.0, 1.0));
                apply_deck_tone(rt, deck);
            }
            EngineCmd::SetMasterLevel { level } => {
                rt.master_level = level.clamp(0.0, 1.0);
            }
            EngineCmd::SetLocalMonitorMuted { muted } => {
                rt.local_monitor_muted = muted;
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
                apply_deck_tone(rt, deck);
            }
            EngineCmd::SetMasterPipeline { settings } => {
                rt.master_pipeline =
                    ChannelPipeline::from_settings(rt.sample_rate as f32, settings);
            }
            EngineCmd::SetDeckCuePreview { deck, enabled } => {
                if matches!(deck, DeckId::DeckA | DeckId::DeckB) {
                    let effective = if rt.cue_split_active || rt.cue_available {
                        enabled
                    } else {
                        false
                    };
                    rt.cue_preview_enabled.insert(deck, effective);
                }
            }
            EngineCmd::SetHeadphoneMix { value } => {
                rt.headphone_mix = value.clamp(-1.0, 1.0);
            }
            EngineCmd::SetHeadphoneLevel { value } => {
                rt.cue_level = value.clamp(0.0, 1.0);
            }
            EngineCmd::SetMonitorRoutingConfig(config) => {
                let wants_split = matches!(
                    config.cue_mix_mode.as_str(),
                    "single_device_four_channel" | "dual_device_split" | "split"
                ) || config
                    .cue_device_id
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty());
                rt.cue_split_active = wants_split && rt.cue_available;
                rt.cue_level = config.cue_level.clamp(0.0, 1.0);
                rt.master_level = config.master_level.clamp(0.0, 1.0);
                if !rt.cue_split_active {
                    rt.cue_preview_enabled.insert(DeckId::DeckA, false);
                    rt.cue_preview_enabled.insert(DeckId::DeckB, false);
                }
            }
        }
    }
}

fn apply_deck_tone(rt: &mut RtState, deck: DeckId) {
    let Some(pipeline) = rt.pipelines.get_mut(&deck) else {
        return;
    };
    let bass_db = rt.deck_bass_db.get(&deck).copied().unwrap_or(0.0);
    let filter = rt.deck_filter_amount.get(&deck).copied().unwrap_or(0.0);

    let low_cut_db = if filter > 0.0 { -18.0 * filter } else { 0.0 };
    let high_cut_db = if filter < 0.0 { -18.0 * (-filter) } else { 0.0 };

    let mut eq = pipeline.eq.config().clone();
    eq.low_gain_db = (bass_db + low_cut_db).clamp(-24.0, 12.0);
    eq.high_gain_db = high_cut_db.clamp(-24.0, 12.0);
    pipeline.eq.set_config(eq);
}

#[inline]
fn accumulate_stereo(dest: &mut [f32], src: &[f32]) {
    for (d, s) in dest.iter_mut().zip(src.iter()) {
        *d += *s;
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
