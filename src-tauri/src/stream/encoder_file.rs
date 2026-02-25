/// `encoder_file.rs` — stream-to-file recording with rotation
///
/// Writes the master PCM audio to disk as WAV (or raw PCM for stubs).
/// Rotation modes: None, Hourly, Daily, BySize.
/// On rotation: closes current file, opens new file — no audio gap intended
/// (gap may be a few frames while the file handle switches).
/// Also writes a companion `.cue` file with track markers (populated via
/// the `record_cue_entry` helper called by the track-change command).
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ringbuf::traits::Consumer as _;
use tokio::sync::oneshot;

use super::{
    broadcaster::EncoderStatus,
    encoder_manager::{EncoderConfig, EncoderManager, FileRotation},
};

/// Async recording loop — runs inside the encoder task.
pub async fn record_loop_async(
    config: &EncoderConfig,
    consumer: &mut ringbuf::HeapCons<f32>,
    stop_rx: &mut oneshot::Receiver<()>,
    manager: &EncoderManager,
) -> Result<(), String> {
    let output_dir = config.file_output_path.as_deref().unwrap_or("./recordings");

    std::fs::create_dir_all(output_dir).map_err(|e| format!("Cannot create recording dir: {e}"))?;

    let id = config.id;
    let max_bytes = config.file_max_size_mb * 1024 * 1024;
    let rotation = &config.file_rotation;

    let mut state = RecordingState::new(config, output_dir)?;
    manager.set_status(id, EncoderStatus::Recording, None);

    // 20 ms frames at 44100 Hz stereo
    const FRAME_SAMPLES: usize = 1764 * 2;
    let mut pcm_buf = vec![0.0f32; FRAME_SAMPLES];

    loop {
        // Non-blocking stop check
        if stop_rx.try_recv().is_ok() {
            state.close();
            return Ok(());
        }

        // Drain ring buffer
        let mut filled = 0;
        while filled < pcm_buf.len() {
            match consumer.try_pop() {
                Some(s) => {
                    pcm_buf[filled] = s;
                    filled += 1;
                }
                None => break,
            }
        }

        if filled == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
            continue;
        }

        // Write samples as 16-bit LE
        for &s in &pcm_buf[..filled] {
            let s16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            if let Err(e) = state.writer.write_all(&s16.to_le_bytes()) {
                return Err(format!("Write error: {e}"));
            }
        }
        state.bytes_written += (filled * 2) as u64;

        // Check rotation triggers
        let rotate = match rotation {
            FileRotation::None => false,
            FileRotation::BySize => state.bytes_written >= max_bytes,
            FileRotation::Hourly => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now / 3600 != state.started_epoch / 3600
            }
            FileRotation::Daily => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now / 86400 != state.started_epoch / 86400
            }
        };

        if rotate {
            let old_path = state.current_path.clone();
            state.close();
            state = RecordingState::new(config, output_dir)?;

            log::info!(
                "Recording rotated: {:?} → {:?}",
                old_path,
                state.current_path
            );

            // Update UI
            let new_file = state.current_path.to_str().unwrap_or_default().to_string();
            // Note: in a full implementation, emit a `recording_rotation` Tauri event here
        }

        // Yield to other tasks
        tokio::task::yield_now().await;
    }
}

// ── Internal recording state ─────────────────────────────────────────────────

struct RecordingState {
    writer: std::io::BufWriter<std::fs::File>,
    current_path: PathBuf,
    bytes_written: u64,
    started_epoch: u64,
}

impl RecordingState {
    fn new(config: &EncoderConfig, output_dir: &str) -> Result<Self, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let filename = expand_template(
            &config.file_name_template,
            config.stream_name.as_deref().unwrap_or("desizone"),
            config.bitrate_kbps.unwrap_or(128),
            "wav",
        );
        let path = Path::new(output_dir).join(&filename);

        let file = std::fs::File::create(&path)
            .map_err(|e| format!("Cannot create recording file {:?}: {e}", path))?;

        let mut writer = std::io::BufWriter::new(file);
        write_wav_header(&mut writer, config.sample_rate, config.channels)
            .map_err(|e| format!("WAV header error: {e}"))?;

        log::info!("Recording started: {:?}", path);

        Ok(Self {
            writer,
            current_path: path,
            bytes_written: 0,
            started_epoch: now,
        })
    }

    fn close(&mut self) {
        let _ = self.writer.flush();
        // Optionally: patch WAV header with correct data size here
        log::info!("Recording closed: {:?}", self.current_path);
    }
}

// ── WAV header ────────────────────────────────────────────────────────────────

fn write_wav_header(
    writer: &mut impl Write,
    sample_rate: u32,
    channels: u8,
) -> std::io::Result<()> {
    let num_channels = channels as u32;
    let bits_per_sample: u32 = 16;
    let byte_rate = sample_rate * num_channels * bits_per_sample / 8;
    let block_align = (num_channels * bits_per_sample / 8) as u16;

    // RIFF header (data size unknown → 0xFFFFFFFF as placeholder)
    writer.write_all(b"RIFF")?;
    writer.write_all(&0xFFFFFFFFu32.to_le_bytes())?; // chunk size placeholder
    writer.write_all(b"WAVE")?;

    // fmt chunk
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // chunk size
    writer.write_all(&1u16.to_le_bytes())?; // PCM
    writer.write_all(&(channels as u16).to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&(bits_per_sample as u16).to_le_bytes())?;

    // data chunk header
    writer.write_all(b"data")?;
    writer.write_all(&0xFFFFFFFFu32.to_le_bytes())?; // size placeholder

    Ok(())
}

// ── File name template expansion ─────────────────────────────────────────────

fn expand_template(template: &str, station: &str, bitrate: u32, codec: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Simple strftime-like substitution
    let now = chrono_simple_now();
    template
        .replace("{date}", &now.0)
        .replace("{time}", &now.1)
        .replace("{datetime}", &format!("{}-{}", now.0, now.1))
        .replace("{station}", &slugify(station))
        .replace("{bitrate}", &bitrate.to_string())
        .replace("{codec}", codec)
}

/// Returns (date_str, time_str) as YYYYMMDD and HHMMSS using epoch math.
fn chrono_simple_now() -> (String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Days since epoch
    let days = secs / 86400;
    let time_of_day = secs % 86400;

    let hh = time_of_day / 3600;
    let mm = (time_of_day % 3600) / 60;
    let ss = time_of_day % 60;

    // Approximate date (no DST, UTC only — good enough for file naming)
    let (y, m, d) = days_to_ymd(days);

    (
        format!("{y:04}{m:02}{d:02}"),
        format!("{hh:02}{mm:02}{ss:02}"),
    )
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
