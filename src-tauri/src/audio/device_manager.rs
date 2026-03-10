use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, SampleFormat, StreamConfig,
};
use serde::{Deserialize, Serialize};

const STARLIGHT_HINT: &str = "djcontrol starlight";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioOutputMode {
    SingleDeviceStereo,
    SingleDeviceFourChannel,
    DualDeviceSplit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub channels: Vec<u16>,
    pub sample_rates: Vec<u32>,
    pub is_starlight: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioOutputRoutingConfig {
    pub mode: AudioOutputMode,
    pub master_device_id: Option<String>,
    pub cue_device_id: Option<String>,
    pub starlight_preferred: bool,
    pub auto_fallback: bool,
}

impl Default for AudioOutputRoutingConfig {
    fn default() -> Self {
        Self {
            mode: AudioOutputMode::SingleDeviceFourChannel,
            master_device_id: None,
            cue_device_id: None,
            starlight_preferred: true,
            auto_fallback: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioOutputStatus {
    pub active_mode: AudioOutputMode,
    pub master_device_id: Option<String>,
    pub master_device_name: Option<String>,
    pub cue_device_id: Option<String>,
    pub cue_available: bool,
    pub fallback_active: bool,
    pub last_error: Option<String>,
}

impl Default for AudioOutputStatus {
    fn default() -> Self {
        Self {
            active_mode: AudioOutputMode::SingleDeviceStereo,
            master_device_id: None,
            master_device_name: None,
            cue_device_id: None,
            cue_available: false,
            fallback_active: false,
            last_error: None,
        }
    }
}

pub struct OutputSelection {
    pub device_id: String,
    pub device_name: String,
    pub device: Device,
    pub config: StreamConfig,
    pub cue_available: bool,
    pub active_mode: AudioOutputMode,
}

pub fn list_audio_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let mut out = Vec::new();
    let devices = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;

    for (index, device) in devices.enumerate() {
        let name = device.name().unwrap_or_else(|_| format!("Output {index}"));
        let id = device_id(index, &name);

        let mut channels = Vec::new();
        let mut sample_rates = Vec::new();

        if let Ok(configs) = device.supported_output_configs() {
            for cfg in configs {
                if !channels.contains(&cfg.channels()) {
                    channels.push(cfg.channels());
                }
                let min = cfg.min_sample_rate().0;
                let max = cfg.max_sample_rate().0;
                if !sample_rates.contains(&min) {
                    sample_rates.push(min);
                }
                if !sample_rates.contains(&max) {
                    sample_rates.push(max);
                }
            }
        }

        channels.sort_unstable();
        sample_rates.sort_unstable();

        out.push(AudioOutputDevice {
            id,
            is_starlight: is_starlight_name(&name),
            is_default: !default_name.is_empty() && name == default_name,
            name,
            channels,
            sample_rates,
        });
    }

    Ok(out)
}

pub fn select_output_stream(
    routing: &AudioOutputRoutingConfig,
) -> Result<(OutputSelection, Option<String>, bool), String> {
    let host = cpal::default_host();
    let devices: Vec<(usize, Device, String)> = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate output devices: {e}"))?
        .enumerate()
        .filter_map(|(idx, d)| {
            let name = d.name().ok()?;
            Some((idx, d, name))
        })
        .collect();

    if devices.is_empty() {
        return Err("No output devices available".to_string());
    }

    let pick_by_id = |id: &str| {
        devices.iter().find_map(|(idx, dev, name)| {
            if device_id(*idx, name) == id {
                Some((idx.to_owned(), dev.to_owned(), name.to_owned()))
            } else {
                None
            }
        })
    };

    let mut selected = routing
        .master_device_id
        .as_deref()
        .and_then(pick_by_id)
        .or_else(|| {
            if routing.starlight_preferred {
                devices
                    .iter()
                    .find(|(_, _, name)| is_starlight_name(name))
                    .map(|(i, d, n)| (*i, d.to_owned(), n.to_owned()))
            } else {
                None
            }
        });

    if selected.is_none() {
        selected = host
            .default_output_device()
            .and_then(|default_dev| {
                let default_name = default_dev.name().ok()?;
                devices
                    .iter()
                    .find(|(_, _, name)| *name == default_name)
                    .map(|(i, d, n)| (*i, d.to_owned(), n.to_owned()))
            })
            .or_else(|| {
                devices
                    .first()
                    .map(|(i, d, n)| (*i, d.to_owned(), n.to_owned()))
            });
    }

    let Some((mut idx, mut device, mut name)) = selected else {
        return Err("Unable to select output device".to_string());
    };

    let desired_mode = match routing.mode {
        AudioOutputMode::DualDeviceSplit => AudioOutputMode::SingleDeviceFourChannel,
        ref m => m.clone(),
    };

    let (mut config, mut cue_available, mut mode, mut warn) =
        choose_stream_config(&device, &desired_mode)?;

    // If 4-channel mode is requested on a stereo device, auto-promote to a Starlight device
    // when available so users don't get stuck on stale/default speaker selection.
    if desired_mode == AudioOutputMode::SingleDeviceFourChannel
        && !cue_available
        && routing.starlight_preferred
    {
        if let Some((s_idx, s_dev, s_name, s_cfg)) = devices
            .iter()
            .filter(|(_, _, n)| is_starlight_name(n))
            .find_map(|(i, d, n)| {
                let Ok((cfg, cue_ok, active_mode, _)) =
                    choose_stream_config(d, &AudioOutputMode::SingleDeviceFourChannel)
                else {
                    return None;
                };
                if cue_ok && active_mode == AudioOutputMode::SingleDeviceFourChannel {
                    Some((*i, d.to_owned(), n.to_owned(), cfg))
                } else {
                    None
                }
            })
        {
            let previous = name.clone();
            idx = s_idx;
            device = s_dev;
            name = s_name.clone();
            config = s_cfg;
            cue_available = true;
            mode = AudioOutputMode::SingleDeviceFourChannel;
            warn = Some(format!(
                "Selected output '{previous}' is stereo; switched to '{s_name}' for 4-channel cue routing"
            ));
        }
    }

    Ok((
        OutputSelection {
            device_id: device_id(idx, &name),
            device_name: name,
            device,
            config,
            cue_available,
            active_mode: mode,
        },
        warn,
        routing.master_device_id.is_some(),
    ))
}

fn choose_stream_config(
    device: &Device,
    desired_mode: &AudioOutputMode,
) -> Result<(StreamConfig, bool, AudioOutputMode, Option<String>), String> {
    let default = device
        .default_output_config()
        .map_err(|e| format!("Failed to get default output config: {e}"))?;

    if default.sample_format() != SampleFormat::F32 {
        return Err(format!(
            "Unsupported sample format {:?}; only f32 is currently supported",
            default.sample_format()
        ));
    }

    let mut warning = None;

    if *desired_mode == AudioOutputMode::SingleDeviceFourChannel {
        if default.channels() >= 4 {
            let config = default.config();
            return Ok((config, true, AudioOutputMode::SingleDeviceFourChannel, None));
        }

        if let Ok(mut ranges) = device.supported_output_configs() {
            if let Some(range) =
                ranges.find(|cfg| cfg.sample_format() == SampleFormat::F32 && cfg.channels() >= 4)
            {
                let supported = range.with_max_sample_rate();
                return Ok((
                    supported.config(),
                    true,
                    AudioOutputMode::SingleDeviceFourChannel,
                    None,
                ));
            }
        }

        warning = Some(
            "Device does not expose a 4-channel output mode; using stereo fallback".to_string(),
        );
    }

    let config = default.config();
    Ok((config, false, AudioOutputMode::SingleDeviceStereo, warning))
}

pub fn device_id(index: usize, name: &str) -> String {
    format!("{index}:{name}")
}

pub fn is_starlight_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains(STARLIGHT_HINT)
}
