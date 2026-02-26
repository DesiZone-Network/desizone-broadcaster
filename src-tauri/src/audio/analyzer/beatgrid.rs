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

use crate::db::local::CueQuantize;

#[derive(Debug, Clone)]
pub struct BeatGridComputed {
    pub bpm: f32,
    pub first_beat_ms: i64,
    pub confidence: f32,
    pub beat_times_ms: Vec<i64>,
}

pub fn analyze_file(path: &Path) -> Result<BeatGridComputed, String> {
    let (samples, sample_rate) = decode_mono(path)?;
    if samples.len() < 2048 || sample_rate == 0 {
        return Ok(BeatGridComputed {
            bpm: 120.0,
            first_beat_ms: 0,
            confidence: 0.0,
            beat_times_ms: vec![],
        });
    }

    let env_sr = 200.0_f32;
    let hop = ((sample_rate as f32 / env_sr).round() as usize).max(1);
    let envelope = build_envelope(&samples, hop);
    if envelope.len() < 64 {
        return Ok(BeatGridComputed {
            bpm: 120.0,
            first_beat_ms: 0,
            confidence: 0.0,
            beat_times_ms: vec![],
        });
    }

    let onset = onset_curve(&envelope);
    let min_bpm = 70.0_f32;
    let max_bpm = 180.0_f32;

    let min_lag = (env_sr * 60.0 / max_bpm).round().max(1.0) as usize;
    let max_lag = (env_sr * 60.0 / min_bpm).round().max(min_lag as f32) as usize;

    let mut best_lag = min_lag;
    let mut best_score = f32::MIN;
    for lag in min_lag..=max_lag {
        let score = autocorr_at_lag(&onset, lag);
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    let bpm = (env_sr * 60.0 / best_lag as f32).clamp(min_bpm, max_bpm);

    let mut best_phase = 0usize;
    let mut phase_score = f32::MIN;
    for phase in 0..best_lag {
        let mut s = 0.0_f32;
        let mut i = phase;
        while i < onset.len() {
            s += onset[i];
            i += best_lag;
        }
        if s > phase_score {
            phase_score = s;
            best_phase = phase;
        }
    }

    let first_beat_ms = ((best_phase as f32 / env_sr) * 1000.0).round() as i64;
    let beat_period_ms = ((best_lag as f32 / env_sr) * 1000.0).max(1.0);
    let duration_ms = (samples.len() as f32 / sample_rate as f32 * 1000.0).round() as i64;

    let mut beat_times_ms = Vec::new();
    let mut t = first_beat_ms.max(0) as f32;
    while t <= duration_ms as f32 {
        beat_times_ms.push(t.round() as i64);
        t += beat_period_ms;
    }

    let denom = onset.iter().map(|v| v * v).sum::<f32>().max(1e-6);
    let confidence = (best_score / denom).clamp(0.0, 1.0);

    Ok(BeatGridComputed {
        bpm,
        first_beat_ms,
        confidence,
        beat_times_ms,
    })
}

pub fn quantize_position_ms(position_ms: i64, beat_times_ms: &[i64], mode: CueQuantize) -> i64 {
    if beat_times_ms.is_empty() || matches!(mode, CueQuantize::Off) {
        return position_ms.max(0);
    }

    let candidates = grid_candidates(beat_times_ms, mode);
    nearest_value(position_ms.max(0), &candidates)
}

fn grid_candidates(beat_times_ms: &[i64], mode: CueQuantize) -> Vec<i64> {
    match mode {
        CueQuantize::Off | CueQuantize::Beat1 => beat_times_ms.to_vec(),
        CueQuantize::BeatHalf => subdivide(beat_times_ms, 2),
        CueQuantize::BeatQuarter => subdivide(beat_times_ms, 4),
    }
}

fn subdivide(beat_times_ms: &[i64], n: i64) -> Vec<i64> {
    if beat_times_ms.len() < 2 || n <= 1 {
        return beat_times_ms.to_vec();
    }

    let mut out = Vec::with_capacity(beat_times_ms.len() * n as usize);
    for window in beat_times_ms.windows(2) {
        let a = window[0];
        let b = window[1];
        let step = (b - a) as f32 / n as f32;
        for i in 0..n {
            out.push((a as f32 + step * i as f32).round() as i64);
        }
    }
    if let Some(&last) = beat_times_ms.last() {
        out.push(last);
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn nearest_value(target: i64, sorted_values: &[i64]) -> i64 {
    if sorted_values.is_empty() {
        return target;
    }
    let mut best = sorted_values[0];
    let mut best_dist = (best - target).abs();
    for &v in sorted_values.iter().skip(1) {
        let d = (v - target).abs();
        if d < best_dist {
            best = v;
            best_dist = d;
        }
    }
    best
}

fn build_envelope(samples: &[f32], hop: usize) -> Vec<f32> {
    let mut env = Vec::with_capacity(samples.len() / hop + 1);
    for chunk in samples.chunks(hop) {
        let mut peak = 0.0_f32;
        for &s in chunk {
            peak = peak.max(s.abs());
        }
        env.push(peak);
    }
    env
}

fn onset_curve(env: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0_f32; env.len()];
    for i in 1..env.len() {
        let d = env[i] - env[i - 1];
        out[i] = if d > 0.0 { d } else { 0.0 };
    }
    out
}

fn autocorr_at_lag(signal: &[f32], lag: usize) -> f32 {
    if lag == 0 || lag >= signal.len() {
        return 0.0;
    }
    let mut s = 0.0_f32;
    for i in lag..signal.len() {
        s += signal[i] * signal[i - lag];
    }
    s
}

fn decode_mono(path: &Path) -> Result<(Vec<f32>, u32), String> {
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
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);

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

        push_mono(decoded, n_channels, &mut out);
    }

    Ok((out, sample_rate))
}

fn push_mono(buf: AudioBufferRef<'_>, n_channels: usize, out: &mut Vec<f32>) {
    let frames = buf.frames();
    match buf {
        AudioBufferRef::F32(b) => {
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push((c0[i] + c1[i]) * 0.5);
            }
        }
        AudioBufferRef::F64(b) => {
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push(((c0[i] + c1[i]) * 0.5) as f32);
            }
        }
        AudioBufferRef::S32(b) => {
            let norm = 1.0 / i32::MAX as f32;
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push(((c0[i] as f32 + c1[i] as f32) * 0.5) * norm);
            }
        }
        AudioBufferRef::S16(b) => {
            let norm = 1.0 / i16::MAX as f32;
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                out.push(((c0[i] as f32 + c1[i] as f32) * 0.5) * norm);
            }
        }
        AudioBufferRef::U8(b) => {
            let c0 = b.chan(0);
            let c1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                let l = (c0[i] as f32 - 128.0) / 128.0;
                let r = (c1[i] as f32 - 128.0) / 128.0;
                out.push((l + r) * 0.5);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_beat_half_picks_nearest_subdivision() {
        let beats = vec![0, 1000, 2000, 3000];
        let snapped = quantize_position_ms(740, &beats, CueQuantize::BeatHalf);
        assert_eq!(snapped, 500);
    }

    #[test]
    fn quantize_beat_quarter_picks_quarter_step() {
        let beats = vec![0, 1000, 2000];
        let snapped = quantize_position_ms(380, &beats, CueQuantize::BeatQuarter);
        assert_eq!(snapped, 500);
    }
}
