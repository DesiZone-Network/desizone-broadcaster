/// `shoutcast.rs` — SHOUTcast v1/v2 source protocol
///
/// SHOUTcast v1 auth: connects to HOST:PORT, sends password as first line,
/// then streams raw MP3 data (no proper HTTP headers on the source side).
/// SHOUTcast v2 is HTTP-based (like Icecast) with the `Icy-` headers.
///
/// This module handles v1 (password-first) by default; v2 can be treated
/// identically to Icecast and routed there.
use std::time::Duration;

use ringbuf::traits::Consumer as _;
use tokio::sync::oneshot;

use super::encoder_manager::EncoderConfig;

/// Shoutcast v1 streaming loop (async, tokio-based).
///
/// Connects via TCP, sends the password, then streams 16-bit LE PCM frames
/// (until a proper MP3 encoder is integrated).
pub async fn stream_loop_async(
    config: &EncoderConfig,
    consumer: &mut ringbuf::HeapCons<f32>,
    stop_rx: &mut oneshot::Receiver<()>,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    let host = config.server_host.as_deref().unwrap_or("localhost");
    let port = config.server_port.unwrap_or(8000);
    let password = config.server_password.as_deref().unwrap_or("");
    let stream_name = config.stream_name.as_deref().unwrap_or("DesiZone");
    let genre = config.stream_genre.as_deref().unwrap_or("Various");
    let bitrate = config.bitrate_kbps.unwrap_or(128);

    log::info!("SHOUTcast: connecting to {host}:{port}");

    let mut stream = TcpStream::connect(format!("{host}:{port}"))
        .await
        .map_err(|e| format!("SHOUTcast TCP connect failed: {e}"))?;

    // v1 handshake: send password line, read OK2 response
    let handshake = format!("{password}\r\n");
    stream
        .write_all(handshake.as_bytes())
        .await
        .map_err(|e| format!("SHOUTcast handshake write failed: {e}"))?;

    // Send source headers
    let headers = format!(
        "icy-name:{stream_name}\r\n\
         icy-genre:{genre}\r\n\
         icy-br:{bitrate}\r\n\
         icy-pub:0\r\n\
         content-type:audio/mpeg\r\n\
         \r\n"
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .map_err(|e| format!("SHOUTcast headers write failed: {e}"))?;

    log::info!("SHOUTcast: streaming started");

    const FRAME_SAMPLES: usize = 1764 * 2; // ~20ms @ 44100 stereo
    let mut pcm_buf = vec![0.0f32; FRAME_SAMPLES];
    let mut out_buf = Vec::with_capacity(FRAME_SAMPLES * 2);

    loop {
        if stop_rx.try_recv().is_ok() {
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

        stream
            .write_all(&out_buf)
            .await
            .map_err(|e| format!("SHOUTcast stream write failed: {e}"))?;

        tokio::task::yield_now().await;
    }
}
