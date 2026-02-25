/// `metadata_pusher.rs` — ICY metadata injection to Icecast / Shoutcast
///
/// When a track changes the operator UI calls `push_metadata` on EncoderManager,
/// which dispatches here per-encoder.
use super::encoder_manager::EncoderConfig;

/// Push ICY metadata to an Icecast 2.x server via the admin API.
/// Endpoint: GET /admin/metadata?mount=/stream&mode=updinfo&song=Artist+-+Title
pub async fn push_icecast_metadata(config: &EncoderConfig, song: &str) -> Result<(), String> {
    let host = config.server_host.as_deref().unwrap_or("localhost");
    let port = config.server_port.unwrap_or(8000);
    let mount = config.mount_point.as_deref().unwrap_or("/stream");
    let password = config.server_password.as_deref().unwrap_or("");

    let encoded_song = urlencoding_encode(song);
    let url = format!(
        "http://{host}:{port}/admin/metadata?mount={mount}&mode=updinfo&song={encoded_song}"
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .basic_auth("admin", Some(password))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Icecast metadata request failed: {e}"))?;

    if resp.status().is_success() {
        log::info!("Icecast metadata updated: {song}");
        Ok(())
    } else {
        Err(format!("Icecast metadata: HTTP {}", resp.status()))
    }
}

/// Push ICY metadata to a Shoutcast server.
/// Endpoint: GET /admin.cgi?pass=...&mode=updinfo&song=...
pub async fn push_shoutcast_metadata(config: &EncoderConfig, song: &str) -> Result<(), String> {
    let host = config.server_host.as_deref().unwrap_or("localhost");
    let port = config.server_port.unwrap_or(8000);
    let password = config.server_password.as_deref().unwrap_or("");
    let encoded_song = urlencoding_encode(song);

    let url =
        format!("http://{host}:{port}/admin.cgi?pass={password}&mode=updinfo&song={encoded_song}");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Shoutcast metadata request failed: {e}"))?;

    if resp.status().is_success() {
        log::info!("Shoutcast metadata updated: {song}");
        Ok(())
    } else {
        Err(format!("Shoutcast metadata: HTTP {}", resp.status()))
    }
}

/// Minimal percent-encoding: replace spaces with + and encode special chars.
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            ' ' => out.push('+'),
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            _ => {
                for b in c.to_string().bytes() {
                    out.push('%');
                    out.push_str(&format!("{b:02X}"));
                }
            }
        }
    }
    out
}
