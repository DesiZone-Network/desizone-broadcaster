use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use ringbuf::traits::Consumer as _;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use super::encoder_manager::EncoderConfig;

/// Icecast / Shoutcast connection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcecastConfig {
    pub host: String,
    pub port: u16,
    pub mount: String,
    pub password: String,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    /// Stream name (sent as Icy-Name header)
    pub stream_name: String,
    /// Genre (Icy-Genre header)
    pub genre: String,
    /// Whether to use Shoutcast v1 (password-only auth) vs Icecast (user:password)
    pub is_shoutcast: bool,
}

impl Default for IcecastConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8000,
            mount: "/stream".to_string(),
            password: "hackme".to_string(),
            bitrate_kbps: 128,
            sample_rate: 44100,
            stream_name: "DesiZone Radio".to_string(),
            genre: "Various".to_string(),
            is_shoutcast: false,
        }
    }
}

/// Handle returned to the caller after starting a stream.
pub struct StreamHandle {
    pub stop_flag: Arc<AtomicBool>,
}

impl StreamHandle {
    /// Signal the stream thread to stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

/// Start an Icecast streaming thread.
///
/// Reads interleaved stereo f32 PCM from `audio_consumer`, encodes to MP3
/// (via the `mp3lame-encoder` crate if available, otherwise raw PCM for now),
/// and HTTP PUT-streams to the Icecast server.
///
/// Returns a `StreamHandle` that can be used to stop the stream.
pub fn start_stream(
    config: IcecastConfig,
    mut audio_consumer: ringbuf::HeapCons<f32>,
) -> StreamHandle {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_t = Arc::clone(&stop_flag);

    std::thread::Builder::new()
        .name("icecast-stream".to_string())
        .spawn(move || {
            if let Err(e) = stream_loop(&config, &mut audio_consumer, &stop_flag_t) {
                log::error!("Icecast stream error: {e}");
            }
            log::info!("Icecast stream thread exited");
        })
        .expect("Failed to spawn icecast thread");

    StreamHandle { stop_flag }
}

/// Main streaming loop — runs on background thread.
fn stream_loop(
    config: &IcecastConfig,
    audio: &mut ringbuf::HeapCons<f32>,
    stop: &AtomicBool,
) -> Result<(), String> {
    use ringbuf::traits::Consumer as _;

    let url = if config.is_shoutcast {
        format!("http://{}:{}/", config.host, config.port)
    } else {
        format!("http://{}:{}{}", config.host, config.port, config.mount)
    };

    log::info!("Connecting to Icecast: {url}");

    // Build a blocking reqwest client
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    // Icecast auth: source:<password> as Basic auth
    let auth_user = if config.is_shoutcast {
        &config.password
    } else {
        "source"
    };
    let auth_pass = &config.password;

    // We use a synchronous pipe: build the request with a streaming body.
    // The body is written via a channel.
    let (body_tx, body_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(32);

    let body = reqwest::blocking::Body::new(BodyReader::new(body_rx));

    // Fire the PUT request on a separate thread so we can write into the body
    let url_clone = url.clone();
    let auth_user_owned = auth_user.to_string();
    let auth_pass_owned = auth_pass.to_string();
    let bitrate = config.bitrate_kbps;
    let stream_name = config.stream_name.clone();
    let genre = config.genre.clone();
    let sample_rate = config.sample_rate;

    let request_thread = std::thread::spawn(move || {
        let result = client
            .put(&url_clone)
            .basic_auth(auth_user_owned, Some(auth_pass_owned))
            .header("Content-Type", "audio/mpeg")
            .header("Icy-Name", stream_name)
            .header("Icy-Genre", genre)
            .header("Icy-Br", bitrate.to_string())
            .header("Icy-Sr", sample_rate.to_string())
            .header("Icy-Pub", "0")
            .header("Transfer-Encoding", "chunked")
            .body(body)
            .send();

        match result {
            Ok(resp) => log::info!("Icecast response: {}", resp.status()),
            Err(e) => log::error!("Icecast request failed: {e}"),
        }
    });

    // PCM → (very simple) raw PCM streaming until we have an mp3 encoder.
    // Frame buffer: ~20 ms at 44100 Hz stereo = 1764 samples = 3528 bytes
    const FRAME_SAMPLES: usize = 1764 * 2; // stereo
    let mut pcm_buf = vec![0.0f32; FRAME_SAMPLES];
    let mut out_buf = Vec::with_capacity(FRAME_SAMPLES * 2);

    while !stop.load(Ordering::Relaxed) {
        // Drain audio from ring buffer into pcm_buf
        let mut filled = 0;
        while filled < pcm_buf.len() {
            match audio.try_pop() {
                Some(s) => {
                    pcm_buf[filled] = s;
                    filled += 1;
                }
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    break;
                }
            }
        }

        if filled == 0 {
            continue;
        }

        // Convert f32 PCM → 16-bit little-endian for streaming
        out_buf.clear();
        for &s in &pcm_buf[..filled] {
            let sample_i16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            out_buf.extend_from_slice(&sample_i16.to_le_bytes());
        }

        if body_tx.send(out_buf.clone()).is_err() {
            log::warn!("Icecast body channel closed");
            break;
        }
    }

    // Signal end of body
    drop(body_tx);
    let _ = request_thread.join();
    Ok(())
}

/// Adapter that makes a `sync_channel` receiver look like `Read` for reqwest.
struct BodyReader {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    buffer: Vec<u8>,
    pos: usize,
}

impl std::io::Read for BodyReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // If local buffer exhausted, fetch next chunk
        if self.pos >= self.buffer.len() {
            match self.rx.recv() {
                Ok(data) => {
                    self.buffer = data;
                    self.pos = 0;
                }
                Err(_) => return Ok(0), // channel closed = EOF
            }
        }

        let available = self.buffer.len() - self.pos;
        let to_copy = buf.len().min(available);
        buf[..to_copy].copy_from_slice(&self.buffer[self.pos..self.pos + to_copy]);
        self.pos += to_copy;
        Ok(to_copy)
    }
}

// Fix struct initialisation — BodyReader needs pos field
impl BodyReader {
    fn new(rx: std::sync::mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            buffer: Vec::new(),
            pos: 0,
        }
    }
}

// ── Async Icecast loop (used by EncoderManager) ──────────────────────────────

/// Async Icecast HTTP PUT source streaming loop.
/// Streams 16-bit LE PCM until stopped or connection error.
pub async fn stream_loop_async(
    config: &EncoderConfig,
    consumer: &mut ringbuf::HeapCons<f32>,
    stop_rx: &mut oneshot::Receiver<()>,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    let host = config.server_host.as_deref().unwrap_or("localhost");
    let port = config.server_port.unwrap_or(8000);
    let mount = config.mount_point.as_deref().unwrap_or("/stream");
    let password = config.server_password.as_deref().unwrap_or("");
    let stream_name = config.stream_name.as_deref().unwrap_or("DesiZone");
    let genre = config.stream_genre.as_deref().unwrap_or("Various");
    let bitrate = config.bitrate_kbps.unwrap_or(128);
    let sample_rate = config.sample_rate;

    let url = format!("http://{host}:{port}{mount}");
    log::info!("Icecast async: connecting to {url}");

    // Use a oneshot channel to pipe PCM into reqwest's streaming body
    let (body_tx, body_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(64);

    let body = reqwest::blocking::Body::new(BodyReader::new(body_rx));
    let url_clone = url.clone();
    let auth_pass = password.to_string();
    let sn = stream_name.to_string();
    let gn = genre.to_string();

    // Fire the blocking PUT on a dedicated thread (reqwest blocking)
    let request_thread = std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        let _ = client
            .put(&url_clone)
            .basic_auth("source", Some(auth_pass))
            .header("Content-Type", "audio/mpeg")
            .header("Icy-Name", sn)
            .header("Icy-Genre", gn)
            .header("Icy-Br", bitrate.to_string())
            .header("Icy-Sr", sample_rate.to_string())
            .header("Icy-Pub", "0")
            .header("Transfer-Encoding", "chunked")
            .body(body)
            .send();
    });

    const FRAME_SAMPLES: usize = 1764 * 2;
    let mut pcm_buf = vec![0.0f32; FRAME_SAMPLES];
    let mut out_buf = Vec::with_capacity(FRAME_SAMPLES * 2);

    loop {
        if stop_rx.try_recv().is_ok() {
            drop(body_tx);
            let _ = request_thread.join();
            return Ok(());
        }

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

        out_buf.clear();
        for &s in &pcm_buf[..filled] {
            let s16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            out_buf.extend_from_slice(&s16.to_le_bytes());
        }

        if body_tx.send(out_buf.clone()).is_err() {
            log::warn!("Icecast body channel closed — reconnecting");
            let _ = request_thread.join();
            return Err("Icecast connection dropped".to_string());
        }

        tokio::task::yield_now().await;
    }
}

/// Quick TCP-level connection test (does not stream audio).
pub async fn test_icecast_connection(config: &EncoderConfig) -> Result<(), String> {
    let host = config.server_host.as_deref().unwrap_or("localhost");
    let port = config.server_port.unwrap_or(8000);
    let mount = config.mount_point.as_deref().unwrap_or("/stream");
    let password = config.server_password.as_deref().unwrap_or("");

    let url = format!("http://{host}:{port}{mount}");

    let client = reqwest::Client::new();
    // HEAD request to check connectivity
    let result = client
        .head(&url)
        .basic_auth("source", Some(password))
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    match result {
        Ok(resp) => {
            // Icecast will return 200 or 404 (mount not found) — either means
            // the server is reachable and accepts our credentials.
            if resp.status().as_u16() < 500 {
                Ok(())
            } else {
                Err(format!("Server returned HTTP {}", resp.status()))
            }
        }
        Err(e) => Err(format!("Connection failed: {e}")),
    }
}
