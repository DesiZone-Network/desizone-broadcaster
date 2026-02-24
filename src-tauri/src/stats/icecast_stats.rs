/// `stats/icecast_stats.rs` — real-time listener stats collector
///
/// Polls Icecast/Shoutcast admin APIs every 30 seconds and stores snapshots
/// in the local SQLite database. The Tauri frontend reads these via
/// `get_listener_stats` / `get_current_listeners`.
use serde::{Deserialize, Serialize};

// ── Snapshot model ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerSnapshot {
    pub id: Option<i64>,
    pub encoder_id: i64,
    pub snapshot_at: i64, // Unix timestamp
    pub current_listeners: u32,
    pub peak_listeners: u32,
    pub unique_listeners: u32,
    pub stream_bitrate: Option<u32>,
}

// ── Icecast JSON response shapes ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct IcecastStatusResponse {
    icestats: IceStats,
}

#[derive(Debug, Deserialize)]
struct IceStats {
    #[serde(rename = "source")]
    sources: Option<IceSourceList>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IceSourceList {
    One(IceSource),
    Many(Vec<IceSource>),
}

#[derive(Debug, Deserialize)]
struct IceSource {
    listenurl: Option<String>,
    listeners: Option<u32>,
    listener_peak: Option<u32>,
    #[serde(rename = "bitrate")]
    bitrate: Option<u32>,
}

// ── Shoutcast JSON response shape ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ShoutcastStats {
    currentlisteners: Option<u32>,
    peaklisteners: Option<u32>,
    uniquelisteners: Option<u32>,
    bitrate: Option<u32>,
}

// ── Polling helpers ───────────────────────────────────────────────────────────

/// Poll an Icecast 2.x server for listener stats on a given mount.
pub async fn poll_icecast(
    host: &str,
    port: u16,
    password: &str,
    mount: &str,
    encoder_id: i64,
) -> Result<ListenerSnapshot, String> {
    let url = format!("http://{host}:{port}/status-json.xsl");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .basic_auth("admin", Some(password))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| format!("Icecast poll failed: {e}"))?;

    let body = resp
        .json::<IcecastStatusResponse>()
        .await
        .map_err(|e| format!("Icecast JSON parse error: {e}"))?;

    // Find the source matching our mount
    let sources: Vec<IceSource> = match body.icestats.sources {
        None => Vec::new(),
        Some(IceSourceList::One(s)) => vec![s],
        Some(IceSourceList::Many(v)) => v,
    };

    let source = sources
        .iter()
        .find(|s| {
            s.listenurl
                .as_deref()
                .map(|u| u.contains(mount))
                .unwrap_or(false)
        })
        .or_else(|| sources.first());

    let now = now_ts();
    Ok(ListenerSnapshot {
        id: None,
        encoder_id,
        snapshot_at: now,
        current_listeners: source.and_then(|s| s.listeners).unwrap_or(0),
        peak_listeners: source.and_then(|s| s.listener_peak).unwrap_or(0),
        unique_listeners: 0, // Icecast does not expose unique count
        stream_bitrate: source.and_then(|s| s.bitrate),
    })
}

/// Poll a SHOUTcast server for listener stats.
pub async fn poll_shoutcast(
    host: &str,
    port: u16,
    password: &str,
    encoder_id: i64,
) -> Result<ListenerSnapshot, String> {
    let url = format!("http://{host}:{port}/statistics?json=1&sid=1&pass={password}");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| format!("Shoutcast poll failed: {e}"))?;

    let stats = resp
        .json::<ShoutcastStats>()
        .await
        .map_err(|e| format!("Shoutcast JSON parse error: {e}"))?;

    let now = now_ts();
    Ok(ListenerSnapshot {
        id: None,
        encoder_id,
        snapshot_at: now,
        current_listeners: stats.currentlisteners.unwrap_or(0),
        peak_listeners: stats.peaklisteners.unwrap_or(0),
        unique_listeners: stats.uniquelisteners.unwrap_or(0),
        stream_bitrate: stats.bitrate,
    })
}

// ── SQLite persistence helpers ────────────────────────────────────────────────

/// Ensure the `listener_snapshots` table exists in the local SQLite DB.
pub async fn ensure_table(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS listener_snapshots (
            id              INTEGER PRIMARY KEY,
            encoder_id      INTEGER NOT NULL,
            snapshot_at     INTEGER DEFAULT (strftime('%s','now')),
            current_listeners INTEGER DEFAULT 0,
            peak_listeners    INTEGER DEFAULT 0,
            unique_listeners  INTEGER DEFAULT 0,
            stream_bitrate    INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_ls_encoder_time
            ON listener_snapshots (encoder_id, snapshot_at);
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert a snapshot into SQLite and return its id.
pub async fn insert_snapshot(
    pool: &sqlx::SqlitePool,
    snap: &ListenerSnapshot,
) -> Result<i64, String> {
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO listener_snapshots
            (encoder_id, snapshot_at, current_listeners, peak_listeners, unique_listeners, stream_bitrate)
        VALUES (?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(snap.encoder_id)
    .bind(snap.snapshot_at)
    .bind(snap.current_listeners as i64)
    .bind(snap.peak_listeners as i64)
    .bind(snap.unique_listeners as i64)
    .bind(snap.stream_bitrate.map(|b| b as i64))
    .fetch_one(pool)
    .await
    .map_err(|e| format!("insert_snapshot: {e}"))?;
    Ok(id)
}

/// Fetch snapshots for an encoder within the requested period (seconds back).
pub async fn get_snapshots(
    pool: &sqlx::SqlitePool,
    encoder_id: i64,
    period_secs: i64,
) -> Result<Vec<ListenerSnapshot>, String> {
    let cutoff = now_ts() - period_secs;
    let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, Option<i64>)>(
        r#"
        SELECT id, encoder_id, snapshot_at,
               current_listeners, peak_listeners, unique_listeners, stream_bitrate
        FROM listener_snapshots
        WHERE encoder_id = ? AND snapshot_at >= ?
        ORDER BY snapshot_at ASC
        "#,
    )
    .bind(encoder_id)
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("get_snapshots: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|(id, enc_id, snap_at, cur, peak, uniq, bitrate)| ListenerSnapshot {
            id: Some(id),
            encoder_id: enc_id,
            snapshot_at: snap_at,
            current_listeners: cur as u32,
            peak_listeners: peak as u32,
            unique_listeners: uniq as u32,
            stream_bitrate: bitrate.map(|b| b as u32),
        })
        .collect())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
