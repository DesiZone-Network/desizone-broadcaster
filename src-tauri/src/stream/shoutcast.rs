/// `shoutcast.rs` — SHOUTcast v1/v2 source protocol handling.
///
/// Notes:
/// - v1 uses legacy source login (password-first over TCP).
/// - v2 in this app uses a compatibility source login format:
///   `<user>:<password>:#<sid>` over the same legacy source transport.
use std::time::Duration;

use ringbuf::traits::Consumer as _;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use super::encoder_manager::{EncoderConfig, EncoderManager, ShoutcastVersion};
use super::mp3::Mp3Encoder;

#[derive(Debug, Deserialize)]
struct ShoutcastV2StatsResponse {
    streams: Option<Vec<ShoutcastV2Stream>>,
}

#[derive(Debug, Deserialize)]
struct ShoutcastV2Stream {
    id: Option<u32>,
}

fn source_ports(port: u16, with_legacy_fallback: bool) -> Vec<u16> {
    if with_legacy_fallback {
        let next = port.saturating_add(1);
        if next != port {
            return vec![port, next];
        }
    }
    vec![port]
}

fn source_password_for_v2(config: &EncoderConfig, sid: u32, user: &str) -> String {
    let password = config.server_password.as_deref().unwrap_or("");
    if user.trim().is_empty() {
        format!("{password}:#{sid}")
    } else {
        format!("{user}:{password}:#{sid}")
    }
}

async fn connect_legacy_source(
    host: &str,
    port: u16,
    password_line: &str,
    label: &str,
) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect(format!("{host}:{port}"))
        .await
        .map_err(|e| format!("{label}: TCP connect {host}:{port} failed: {e}"))?;

    let handshake = format!("{password_line}\r\n");
    stream
        .write_all(handshake.as_bytes())
        .await
        .map_err(|e| format!("{label}: handshake write failed on {host}:{port}: {e}"))?;

    let mut probe = [0u8; 512];
    match tokio::time::timeout(Duration::from_millis(1200), stream.read(&mut probe)).await {
        Ok(Ok(0)) => Err(format!(
            "{label}: server closed connection after source login on {host}:{port}"
        )),
        Ok(Ok(n)) => {
            let response = String::from_utf8_lossy(&probe[..n]).replace('\r', "");
            let response_lc = response.to_ascii_lowercase();
            log::info!(
                "{}: source handshake response from {}:{} => {}",
                label,
                host,
                port,
                response.replace('\n', "\\n")
            );
            if response_lc.contains("invalid")
                || response_lc.contains("error")
                || response_lc.contains("authentication")
                || response_lc.contains("bad password")
            {
                Err(format!(
                    "{label}: source login rejected on {host}:{port}: {}",
                    response.trim()
                ))
            } else {
                Ok(stream)
            }
        }
        Ok(Err(e)) => Err(format!(
            "{label}: failed reading handshake response from {host}:{port}: {e}"
        )),
        Err(_) => {
            // No immediate reply is acceptable for some servers; continue.
            log::info!(
                "{}: no immediate handshake response from {}:{} (continuing)",
                label,
                host,
                port
            );
            Ok(stream)
        }
    }
}

async fn stream_legacy_source_async(
    config: &EncoderConfig,
    consumer: &mut ringbuf::HeapCons<f32>,
    stop_rx: &mut oneshot::Receiver<()>,
    host: &str,
    port: u16,
    password_line: &str,
    label: &str,
    manager: &EncoderManager,
    encoder_id: i64,
) -> Result<(), String> {
    let stream_name = config.stream_name.as_deref().unwrap_or("DesiZone");
    let genre = config.stream_genre.as_deref().unwrap_or("Various");
    let bitrate = config.bitrate_kbps.unwrap_or(128);
    let public_flag = if config.is_public { "1" } else { "0" };

    let mut stream = connect_legacy_source(host, port, password_line, label).await?;

    let headers = format!(
        "icy-name:{stream_name}\r\n\
         icy-genre:{genre}\r\n\
         icy-br:{bitrate}\r\n\
         icy-pub:{public_flag}\r\n\
         content-type:audio/mpeg\r\n\
         \r\n"
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .map_err(|e| format!("{label}: headers write failed on {host}:{port}: {e}"))?;

    log::info!(
        "{}: source connected host={} port={} bitrate={} public={}",
        label,
        host,
        port,
        bitrate,
        public_flag
    );

    let mut mp3 = Mp3Encoder::from_config(config)?;
    let mut pcm_buf = vec![0.0f32; mp3.frame_samples()];
    let silence = vec![0.0f32; mp3.frame_samples()];
    let channels = u64::from(config.channels.clamp(1, 2));
    let per_channel_samples = (mp3.frame_samples() as u64 / channels).max(1);
    let frame_interval =
        Duration::from_secs_f64(per_channel_samples as f64 / f64::from(config.sample_rate.max(1)));
    let keepalive_after = Duration::from_secs(2);
    let mut last_sent = std::time::Instant::now();
    let mut empty_since: Option<std::time::Instant> = None;
    let mut pending_bytes: u64 = 0;
    let mut last_flush = std::time::Instant::now();

    loop {
        if stop_rx.try_recv().is_ok() {
            if pending_bytes > 0 {
                manager.add_bytes_sent(encoder_id, pending_bytes);
            }
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
            let now = std::time::Instant::now();
            let empty_at = *empty_since.get_or_insert(now);
            let idle_for = now.saturating_duration_since(empty_at);
            if idle_for >= keepalive_after && last_sent.elapsed() >= frame_interval {
                let encoded = mp3.encode_f32_interleaved(&silence)?;
                if !encoded.is_empty() {
                    stream.write_all(encoded).await.map_err(|e| {
                        format!("{label}: stream write failed on {host}:{port}: {e}")
                    })?;
                    pending_bytes = pending_bytes.saturating_add(encoded.len() as u64);
                    last_sent = std::time::Instant::now();
                }
            } else {
                tokio::time::sleep(Duration::from_millis(3)).await;
            }
            continue;
        }
        empty_since = None;

        let encoded = mp3.encode_f32_interleaved(&pcm_buf[..filled])?;
        if encoded.is_empty() {
            tokio::task::yield_now().await;
            continue;
        }

        stream
            .write_all(encoded)
            .await
            .map_err(|e| format!("{label}: stream write failed on {host}:{port}: {e}"))?;
        pending_bytes = pending_bytes.saturating_add(encoded.len() as u64);
        last_sent = std::time::Instant::now();
        if pending_bytes >= 16 * 1024 || last_flush.elapsed() >= Duration::from_millis(500) {
            manager.add_bytes_sent(encoder_id, pending_bytes);
            pending_bytes = 0;
            last_flush = std::time::Instant::now();
        }

        tokio::task::yield_now().await;
    }
}

/// Shoutcast streaming loop (async, tokio-based).
pub async fn stream_loop_async(
    config: &EncoderConfig,
    consumer: &mut ringbuf::HeapCons<f32>,
    stop_rx: &mut oneshot::Receiver<()>,
    manager: &EncoderManager,
) -> Result<(), String> {
    let host = config.server_host.as_deref().unwrap_or("localhost");
    let port = config.server_port.unwrap_or(8000);
    let user = config
        .server_username
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("source");

    match config.shoutcast_version {
        ShoutcastVersion::V1 => {
            let password_line = config.server_password.as_deref().unwrap_or("").to_string();
            log::info!(
                "SHOUTcast v1 stream: host={} port={} (legacy source mode)",
                host,
                port
            );
            stream_legacy_source_async(
                config,
                consumer,
                stop_rx,
                host,
                port,
                &password_line,
                "SHOUTcast v1",
                manager,
                config.id,
            )
            .await
        }
        ShoutcastVersion::V2 => {
            let sid = config.shoutcast_sid.max(1);
            let password_line = source_password_for_v2(config, sid, user);
            let ports = source_ports(port, true);

            log::info!(
                "SHOUTcast v2 stream: host={} ports={:?} sid={} user={} mode=legacy_compat",
                host,
                ports,
                sid,
                user
            );

            let mut last_err = String::new();
            for (idx, p) in ports.iter().copied().enumerate() {
                let label = if idx == 0 {
                    "SHOUTcast v2"
                } else {
                    "SHOUTcast v2 fallback"
                };
                match stream_legacy_source_async(
                    config,
                    consumer,
                    stop_rx,
                    host,
                    p,
                    &password_line,
                    label,
                    manager,
                    config.id,
                )
                .await
                {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        last_err = e;
                        log::warn!(
                            "SHOUTcast v2 stream attempt failed on {}:{} (sid={}): {}",
                            host,
                            p,
                            sid,
                            last_err
                        );
                    }
                }
            }

            Err(if last_err.is_empty() {
                "SHOUTcast v2 all source attempts failed".to_string()
            } else {
                last_err
            })
        }
    }
}

/// Protocol-aware connection test for SHOUTcast.
pub async fn test_shoutcast_connection(config: &EncoderConfig) -> Result<(), String> {
    let host = config.server_host.as_deref().unwrap_or("localhost");
    let port = config.server_port.unwrap_or(8000);
    let password = config.server_password.as_deref().unwrap_or("");
    let user = config
        .server_username
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("source");

    match config.shoutcast_version {
        ShoutcastVersion::V1 => {
            log::info!("SHOUTcast v1 test: host={} port={}", host, port);
            tokio::net::TcpStream::connect(format!("{host}:{port}"))
                .await
                .map(|_| ())
                .map_err(|e| format!("SHOUTcast v1 TCP connect failed: {e}"))
        }
        ShoutcastVersion::V2 => {
            let sid = config.shoutcast_sid.max(1);
            let stats_url =
                format!("http://{host}:{port}/statistics?json=1&sid={sid}&pass={password}");

            log::info!(
                "SHOUTcast v2 test: host={} port={} sid={} user={} mode=legacy_compat",
                host,
                port,
                sid,
                user
            );

            let stats_resp = reqwest::Client::new()
                .get(&stats_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .map_err(|e| format!("SHOUTcast v2 stats request failed: {e}"))?;

            let status = stats_resp.status();
            let body = stats_resp.text().await.unwrap_or_default();
            let snippet: String = body.chars().take(240).collect();
            log::info!(
                "SHOUTcast v2 test stats status={} body_snippet={}",
                status,
                snippet
            );
            if !status.is_success() {
                return Err(format!(
                    "SHOUTcast v2 stats returned HTTP {} body: {}",
                    status, snippet
                ));
            }

            let parsed: ShoutcastV2StatsResponse = serde_json::from_str(&body).map_err(|e| {
                format!("SHOUTcast v2 stats parse failed: {} (body: {})", e, snippet)
            })?;
            let sid_found = parsed
                .streams
                .unwrap_or_default()
                .iter()
                .any(|s| s.id == Some(sid));
            if !sid_found {
                return Err(format!(
                    "SHOUTcast v2 SID {} not found in server streams (body: {})",
                    sid, snippet
                ));
            }

            let password_line = source_password_for_v2(config, sid, user);
            let mut last_err = String::new();
            for p in source_ports(port, true) {
                match connect_legacy_source(host, p, &password_line, "SHOUTcast v2 test").await {
                    Ok(_) => {
                        log::info!("SHOUTcast v2 test source login accepted on {}:{}", host, p);
                        return Ok(());
                    }
                    Err(e) => {
                        last_err = e;
                    }
                }
            }

            Err(if last_err.is_empty() {
                "SHOUTcast v2 source login failed on all candidate ports".to_string()
            } else {
                last_err
            })
        }
    }
}
