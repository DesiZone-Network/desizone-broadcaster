use std::{fs::File, path::Path};

use symphonia::core::{
    audio::{AudioBufferRef, Signal},
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymphoniaError,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_waveform_data(
    file_path: String,
    resolution: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<f32>, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }
    if !path.is_file() {
        return Err(format!("Path is not a file: {file_path}"));
    }

    let resolution = resolution.unwrap_or(1200).clamp(64, 6000) as i64;
    let mtime_ms = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    if let Some(local) = &state.local_db {
        if let Ok(Some(cached)) =
            crate::db::local::get_waveform_cache(local, &file_path, mtime_ms, resolution).await
        {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
    }

    let path_buf = path.to_path_buf();
    let resolution_usize = resolution as usize;
    let peaks = tauri::async_runtime::spawn_blocking(move || {
        let samples = decode_mono_abs(&path_buf)?;
        Ok::<Vec<f32>, String>(downsample_peaks(&samples, resolution_usize))
    })
    .await
    .map_err(|e| format!("Waveform worker join failed: {e}"))??;

    if let Some(local) = &state.local_db {
        let _ =
            crate::db::local::save_waveform_cache(local, &file_path, mtime_ms, resolution, &peaks)
                .await;
    }

    Ok(peaks)
}

fn decode_mono_abs(path: &Path) -> Result<Vec<f32>, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Probe failed: {e}"))?;

    let track = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No audio track found")?
        .clone();
    let track_id = track.id;
    let n_channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(2)
        .max(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Codec init failed: {e}"))?;

    let mut out = Vec::<f32>::new();

    loop {
        let packet = match probed.format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => return Err(format!("Read packet failed: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("Decode failed: {e}")),
        };

        push_abs_mono(decoded, n_channels, &mut out);
    }

    Ok(out)
}

fn push_abs_mono(buf: AudioBufferRef<'_>, n_channels: usize, out: &mut Vec<f32>) {
    let frames = buf.frames();
    match buf {
        AudioBufferRef::F32(b) => {
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push(((c0[i] + c1[i]) * 0.5).abs());
            }
        }
        AudioBufferRef::F64(b) => {
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push((((c0[i] + c1[i]) * 0.5) as f32).abs());
            }
        }
        AudioBufferRef::S32(b) => {
            let norm = 1.0 / i32::MAX as f32;
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push((((c0[i] as f32 + c1[i] as f32) * 0.5) * norm).abs());
            }
        }
        AudioBufferRef::S16(b) => {
            let norm = 1.0 / i16::MAX as f32;
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push((((c0[i] as f32 + c1[i] as f32) * 0.5) * norm).abs());
            }
        }
        AudioBufferRef::U8(b) => {
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                let l = (c0[i] as f32 - 128.0) / 128.0;
                let r = (c1[i] as f32 - 128.0) / 128.0;
                out.push(((l + r) * 0.5).abs());
            }
        }
        _ => {}
    }
}

fn downsample_peaks(samples: &[f32], resolution: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; resolution.max(1)];
    }
    if resolution <= 1 {
        return vec![samples.iter().copied().fold(0.0_f32, f32::max)];
    }

    let mut peaks = vec![0.0_f32; resolution];
    let chunk = (samples.len() as f64 / resolution as f64).max(1.0);

    for (i, peak) in peaks.iter_mut().enumerate() {
        let start = (i as f64 * chunk).floor() as usize;
        let end = (((i + 1) as f64 * chunk).floor() as usize).min(samples.len());
        let mut p = 0.0_f32;
        if start < end {
            for &v in &samples[start..end] {
                if v > p {
                    p = v;
                }
            }
        }
        *peak = p.clamp(0.0, 1.0);
    }

    peaks
}
