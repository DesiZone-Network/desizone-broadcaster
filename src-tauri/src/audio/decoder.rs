use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
};

use ringbuf::{
    traits::{Observer as _, Producer as _, Split},
    HeapRb,
};
use symphonia::core::{
    audio::AudioBufferRef,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymphoniaError,
    formats::{FormatOptions, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

/// Stereo f32 samples buffered ahead of the playback thread (~3 s at 44.1 kHz)
const RING_CAPACITY: usize = 44100 * 2 * 3;

/// Consumer-side handle owned by the audio render thread.
pub struct DecoderHandle {
    pub consumer: ringbuf::HeapCons<f32>,
    pub stop_flag: Arc<AtomicBool>,
    /// Set true when decode thread reaches terminal end (EOF or fatal error).
    pub decode_done: Arc<AtomicBool>,
    /// Total frames written by decoder (used for position estimates)
    pub frames_written: Arc<AtomicU64>,
    /// Total frames in the file (0 until probed)
    pub total_frames: Arc<AtomicU64>,
    pub sample_rate: u32,
    pub channels: u32,
}

impl DecoderHandle {
    pub fn duration_ms(&self) -> u64 {
        let frames = self.total_frames.load(Ordering::Relaxed);
        if self.sample_rate == 0 || frames == 0 {
            return 0;
        }
        frames * 1000 / self.sample_rate as u64
    }
}

/// Spawn a background Symphonia decode thread for `path`.
/// Returns a `DecoderHandle` the audio thread uses to pull PCM.
pub fn spawn_decoder(path: PathBuf, seek_ms: Option<u64>) -> Result<DecoderHandle, String> {
    let rb = HeapRb::<f32>::new(RING_CAPACITY);
    let (mut producer, consumer) = rb.split();

    let stop_flag = Arc::new(AtomicBool::new(false));
    let decode_done = Arc::new(AtomicBool::new(false));
    let frames_written = Arc::new(AtomicU64::new(0));
    let total_frames = Arc::new(AtomicU64::new(0));

    let (sample_rate, channels) = probe_metadata(&path)?;

    let handle = DecoderHandle {
        consumer,
        stop_flag: Arc::clone(&stop_flag),
        decode_done: Arc::clone(&decode_done),
        frames_written: Arc::clone(&frames_written),
        total_frames: Arc::clone(&total_frames),
        sample_rate,
        channels,
    };

    let stop_flag_t = Arc::clone(&stop_flag);
    let decode_done_t = Arc::clone(&decode_done);
    let fw_t = Arc::clone(&frames_written);
    let tf_t = Arc::clone(&total_frames);

    thread::Builder::new()
        .name(format!(
            "dec:{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ))
        .spawn(move || {
            if let Err(e) = decode_loop(path, seek_ms, &mut producer, &stop_flag_t, &fw_t, &tf_t) {
                log::warn!("Decoder exited: {e}");
            }
            decode_done_t.store(true, Ordering::Relaxed);
        })
        .map_err(|e| format!("Failed to spawn decoder thread: {e}"))?;

    Ok(handle)
}

fn probe_metadata(path: &PathBuf) -> Result<(u32, u32), String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
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
        .ok_or("No audio track found")?;
    let sr = track.codec_params.sample_rate.unwrap_or(44100);
    let ch = track
        .codec_params
        .channels
        .map(|c| c.count() as u32)
        .unwrap_or(2);
    Ok((sr, ch))
}

fn decode_loop(
    path: PathBuf,
    seek_ms: Option<u64>,
    producer: &mut ringbuf::HeapProd<f32>,
    stop_flag: &AtomicBool,
    frames_written: &AtomicU64,
    total_frames: &AtomicU64,
) -> Result<(), String> {
    let file =
        std::fs::File::open(&path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
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
    if let Some(n) = track.codec_params.n_frames {
        total_frames.store(n, Ordering::Relaxed);
    }
    let n_channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Codec init: {e}"))?;

    if let Some(ms) = seek_ms {
        let time = Time::from(ms as f64 / 1000.0);
        let _ = probed.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(track_id),
            },
        );
    }

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
        let packet = match probed.format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => {
                log::warn!("Format read: {e}");
                break;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(e)) => {
                log::warn!("Decode error (skip): {e}");
                continue;
            }
            Err(e) => {
                log::warn!("Fatal decode: {e}");
                break;
            }
        };
        let n = push_decoded(decoded, n_channels, producer, stop_flag);
        frames_written.fetch_add(n as u64, Ordering::Relaxed);
    }
    Ok(())
}

/// Convert `AudioBufferRef` to interleaved f32 stereo, pushing into the ring buffer.
/// Returns number of frames pushed.
fn push_decoded(
    buf: AudioBufferRef<'_>,
    n_channels: usize,
    producer: &mut ringbuf::HeapProd<f32>,
    stop_flag: &AtomicBool,
) -> usize {
    use symphonia::core::audio::Signal;

    let frames = buf.frames();
    let mut written = 0;

    macro_rules! push_frame {
        ($b:expr, $norm:expr) => {{
            let chan0 = $b.chan(0);
            let chan1 = if n_channels > 1 {
                $b.chan(1)
            } else {
                $b.chan(0)
            };
            for i in 0..frames {
                if stop_flag.load(Ordering::Relaxed) {
                    return written;
                }
                let l = chan0[i] as f32 * $norm;
                let r = chan1[i] as f32 * $norm;
                // Spin until space for BOTH L and R samples is available.
                //
                // BUG FIX: the previous `try_push(l).is_ok() && try_push(r).is_ok()`
                // pattern was racy — if L pushed successfully but R failed, L was
                // already in the ring buffer. On the next retry L would be pushed
                // again, corrupting the interleaved stereo stream (LLLR LLLR…).
                //
                // Fix: check vacant_len() >= 2 atomically before pushing either.
                loop {
                    if stop_flag.load(Ordering::Relaxed) {
                        return written;
                    }
                    if producer.vacant_len() >= 2 {
                        let _ = producer.try_push(l);
                        let _ = producer.try_push(r);
                        break;
                    }
                    thread::yield_now();
                }
                written += 1;
            }
        }};
    }

    match buf {
        AudioBufferRef::F32(b) => push_frame!(b, 1.0_f32),
        AudioBufferRef::F64(b) => {
            // f64 samples — handle separately to avoid type mismatch in macro
            let chan0 = b.chan(0);
            let chan1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                if stop_flag.load(Ordering::Relaxed) {
                    return written;
                }
                let l = chan0[i] as f32;
                let r = chan1[i] as f32;
                loop {
                    if stop_flag.load(Ordering::Relaxed) {
                        return written;
                    }
                    if producer.vacant_len() >= 2 {
                        let _ = producer.try_push(l);
                        let _ = producer.try_push(r);
                        break;
                    }
                    thread::yield_now();
                }
                written += 1;
            }
        }
        AudioBufferRef::S32(b) => push_frame!(b, 1.0 / i32::MAX as f32),
        AudioBufferRef::S16(b) => push_frame!(b, 1.0 / i16::MAX as f32),
        AudioBufferRef::U8(b) => {
            // U8 is unsigned 0-255 centred at 128
            let chan0 = b.chan(0);
            let chan1 = if n_channels > 1 { b.chan(1) } else { b.chan(0) };
            for i in 0..frames {
                if stop_flag.load(Ordering::Relaxed) {
                    return written;
                }
                let l = (chan0[i] as f32 - 128.0) / 128.0;
                let r = (chan1[i] as f32 - 128.0) / 128.0;
                loop {
                    if stop_flag.load(Ordering::Relaxed) {
                        return written;
                    }
                    if producer.vacant_len() >= 2 {
                        let _ = producer.try_push(l);
                        let _ = producer.try_push(r);
                        break;
                    }
                    thread::yield_now();
                }
                written += 1;
            }
        }
        _ => {
            // Unsupported format — push silence
            for _ in 0..frames {
                let _ = producer.try_push(0.0);
                let _ = producer.try_push(0.0);
                written += 1;
            }
        }
    }

    written
}
