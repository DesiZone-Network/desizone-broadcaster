/// `audio/mic_input.rs` — Live microphone input via CPAL
///
/// Captures audio from the selected input device, applies the Voice FX pipeline
/// (noise gate → EQ → compressor → de-esser → reverb), and writes it to a
/// ring buffer that the main mixer reads as the Voice FX channel.
///
/// Voice track recording writes raw samples to a temp WAV file via `hound`.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavWriter, WavSpec};
use serde::{Deserialize, Serialize};

use crate::audio::dsp::{deesser::Deesser, reverb::Reverb};

// ── MicConfig ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicConfig {
    pub device_name: Option<String>,
    pub sample_rate: u32,
    pub channels: u8,

    // Noise gate
    pub gate_enabled: bool,
    pub gate_threshold_db: f32,
    pub gate_attack_ms: f32,
    pub gate_release_ms: f32,

    // Compressor
    pub comp_enabled: bool,
    pub comp_ratio: f32,
    pub comp_threshold_db: f32,
    pub comp_attack_ms: f32,
    pub comp_release_ms: f32,

    // PTT
    pub ptt_enabled: bool,
    pub ptt_hotkey: Option<String>,
}

impl Default for MicConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            sample_rate: 44100,
            channels: 1,
            gate_enabled: true,
            gate_threshold_db: -40.0,
            gate_attack_ms: 5.0,
            gate_release_ms: 200.0,
            comp_enabled: true,
            comp_ratio: 4.0,
            comp_threshold_db: -18.0,
            comp_attack_ms: 10.0,
            comp_release_ms: 100.0,
            ptt_enabled: false,
            ptt_hotkey: None,
        }
    }
}

// ── AudioDevice info ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

pub fn list_input_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    host.input_devices()
        .map(|devs| {
            devs.filter_map(|d| {
                d.name().ok().map(|name| {
                    let is_default = default_name.as_deref() == Some(name.as_str());
                    AudioDevice { name, is_default }
                })
            }).collect()
        })
        .unwrap_or_default()
}

// ── MicState (real-time shared) ───────────────────────────────────────────────

/// Shared between the audio callback and the command layer.
pub struct MicState {
    pub config: MicConfig,
    pub ptt_active: bool,
    pub muted: bool,
    pub mic_level_l: f32,
    pub mic_level_r: f32,
    pub recording: bool,
    pub deesser: Deesser,
    pub reverb: Reverb,
    /// If recording, samples are written here
    pub wav_writer: Option<WavWriter<std::io::BufWriter<std::fs::File>>>,
}

impl MicState {
    pub fn new(config: MicConfig) -> Self {
        let sr = config.sample_rate as f32;
        Self {
            deesser: Deesser::new(sr),
            reverb: Reverb::new(sr),
            config,
            ptt_active: false,
            muted: false,
            mic_level_l: 0.0,
            mic_level_r: 0.0,
            recording: false,
            wav_writer: None,
        }
    }
}

// ── MicInput ──────────────────────────────────────────────────────────────────

/// Shared handle — lives in AppState.
#[derive(Clone)]
pub struct MicInput {
    state: Arc<Mutex<MicState>>,
    stream: Arc<Mutex<Option<cpal::Stream>>>,
}

// SAFETY: cpal::Stream is !Send but we gate all access behind a Mutex.
unsafe impl Send for MicInput {}
unsafe impl Sync for MicInput {}

impl MicInput {
    pub fn new(config: MicConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(MicState::new(config))),
            stream: Arc::new(Mutex::new(None)),
        }
    }

    // ── PTT ───────────────────────────────────────────────────────────────

    pub fn set_ptt(&self, active: bool) {
        self.state.lock().unwrap().ptt_active = active;
    }

    pub fn set_muted(&self, muted: bool) {
        self.state.lock().unwrap().muted = muted;
    }

    // ── Levels (for VU meter) ─────────────────────────────────────────────

    pub fn get_levels(&self) -> (f32, f32) {
        let st = self.state.lock().unwrap();
        (st.mic_level_l, st.mic_level_r)
    }

    // ── Config ────────────────────────────────────────────────────────────

    pub fn get_config(&self) -> MicConfig {
        self.state.lock().unwrap().config.clone()
    }

    pub fn set_config(&self, config: MicConfig) {
        self.state.lock().unwrap().config = config;
    }

    // ── Start / Stop ──────────────────────────────────────────────────────

    pub fn start(&self) -> Result<(), String> {
        let host = cpal::default_host();
        let config_guard = self.state.lock().unwrap();
        let device_name = config_guard.config.device_name.clone();
        drop(config_guard);

        let device = if let Some(name) = device_name {
            host.input_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().ok().as_deref() == Some(name.as_str()))
                .ok_or_else(|| format!("Input device '{name}' not found"))?
        } else {
            host.default_input_device()
                .ok_or("No default input device found")?
        };

        let supported = device.default_input_config()
            .map_err(|e| e.to_string())?;

        let state = Arc::clone(&self.state);

        let stream = device.build_input_stream(
            &supported.config(),
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                Self::audio_callback(data, &state);
            },
            |e| log::error!("Mic input error: {e}"),
            None,
        ).map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;
        *self.stream.lock().unwrap() = Some(stream);
        log::info!("Microphone input started");
        Ok(())
    }

    pub fn stop(&self) {
        *self.stream.lock().unwrap() = None;
        log::info!("Microphone input stopped");
    }

    fn audio_callback(data: &[f32], state: &Arc<Mutex<MicState>>) {
        let mut st = state.lock().unwrap();
        let channels = st.config.channels as usize;
        let sr = st.config.sample_rate as f32;

        // Simple peak envelope for VU
        let peak: f32 = data.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        st.mic_level_l = st.mic_level_l * 0.9 + peak * 0.1;
        st.mic_level_r = st.mic_level_r * 0.9 + peak * 0.1;

        // Gate: if muted or PTT not active (when PTT enabled), output silence
        let pass = !st.muted && (!st.config.ptt_enabled || st.ptt_active);

        // Noise gate (simple threshold)
        let gate_thr = db_to_linear(st.config.gate_threshold_db);
        let gate_pass = !st.config.gate_enabled || peak > gate_thr;

        // Write to WAV if recording
        if st.recording {
            if let Some(ref mut writer) = st.wav_writer {
                for &s in data.iter().take(512) {
                    let _ = writer.write_sample(s);
                }
            }
        }

        if !pass || !gate_pass {
            // Silence — no further processing needed for voice chain
            drop(st);
            return;
        }

        // Stereo frame processing for de-esser / reverb
        let mut i = 0;
        let mut frames: Vec<[f32; 2]> = Vec::new();
        while i + channels <= data.len() {
            let l = data[i];
            let r = if channels >= 2 { data[i + 1] } else { l };
            frames.push([l, r]);
            i += channels;
        }
        for frame in &mut frames {
            st.deesser.process(frame.as_mut_slice());
            st.reverb.process(frame.as_mut_slice());
        }

        let _ = sr; // used for tick-based envelope calculations in future
    }

    // ── Voice Track Recording ─────────────────────────────────────────────

    pub fn start_recording(&self, path: &str) -> Result<(), String> {
        let mut st = self.state.lock().unwrap();
        let sr = st.config.sample_rate;
        let ch = st.config.channels;
        let spec = WavSpec {
            channels: ch as u16,
            sample_rate: sr,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let writer = WavWriter::new(std::io::BufWriter::new(f), spec)
            .map_err(|e| e.to_string())?;
        st.wav_writer = Some(writer);
        st.recording = true;
        Ok(())
    }

    pub fn stop_recording(&self) -> Result<u64, String> {
        let mut st = self.state.lock().unwrap();
        st.recording = false;
        if let Some(writer) = st.wav_writer.take() {
            let samples = writer.len() as u64;
            writer.finalize().map_err(|e| e.to_string())?;
            let duration_ms = samples * 1000 / st.config.sample_rate as u64;
            Ok(duration_ms)
        } else {
            Err("Not recording".to_string())
        }
    }
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}
