use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Row};

/// Initialise (or migrate) the local SQLite database at `db_path`.
/// Creates all tables if they don't exist.
pub async fn init_db(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    let url = format!("sqlite:{db_path}?mode=rwc");
    let pool = SqlitePool::connect(&url).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cue_points (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            song_id     INTEGER NOT NULL,
            name        TEXT    NOT NULL,
            position_ms INTEGER NOT NULL,
            UNIQUE(song_id, name)
        );

        CREATE TABLE IF NOT EXISTS song_fade_overrides (
            song_id             INTEGER PRIMARY KEY,
            fade_out_enabled    INTEGER,
            fade_out_curve      TEXT,
            fade_out_time_ms    INTEGER,
            fade_in_enabled     INTEGER,
            fade_in_curve       TEXT,
            fade_in_time_ms     INTEGER,
            crossfade_mode      TEXT,
            gain_db             REAL
        );

        CREATE TABLE IF NOT EXISTS channel_dsp_settings (
            channel             TEXT    PRIMARY KEY,
            eq_low_gain_db      REAL    DEFAULT 0.0,
            eq_low_freq_hz      REAL    DEFAULT 100.0,
            eq_mid_gain_db      REAL    DEFAULT 0.0,
            eq_mid_freq_hz      REAL    DEFAULT 1000.0,
            eq_mid_q            REAL    DEFAULT 0.7071,
            eq_high_gain_db     REAL    DEFAULT 0.0,
            eq_high_freq_hz     REAL    DEFAULT 8000.0,
            agc_enabled         INTEGER DEFAULT 0,
            agc_gate_db         REAL    DEFAULT -31.0,
            agc_max_gain_db     REAL    DEFAULT 5.0,
            agc_attack_ms       REAL    DEFAULT 100.0,
            agc_release_ms      REAL    DEFAULT 500.0,
            agc_pre_emphasis    TEXT    DEFAULT '75us',
            comp_enabled        INTEGER DEFAULT 0,
            comp_settings_json  TEXT
        );

        CREATE TABLE IF NOT EXISTS crossfade_config (
            id          INTEGER PRIMARY KEY DEFAULT 1,
            config_json TEXT    NOT NULL
        );

        -- Phase 3: Rotation / AutoDJ
        CREATE TABLE IF NOT EXISTS rotation_rules (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL,
            rule_type   TEXT    NOT NULL,
            config_json TEXT    NOT NULL,
            enabled     INTEGER DEFAULT 1,
            priority    INTEGER DEFAULT 0,
            created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS rotation_playlists (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL,
            description TEXT,
            is_active   INTEGER DEFAULT 0,
            config_json TEXT    NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS playlist_songs (
            playlist_id INTEGER NOT NULL,
            song_id     INTEGER NOT NULL,
            position    INTEGER,
            weight      REAL    DEFAULT 1.0,
            PRIMARY KEY (playlist_id, song_id)
        );

        -- Phase 3: Show Scheduler
        CREATE TABLE IF NOT EXISTS scheduled_shows (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            name             TEXT    NOT NULL,
            days_json        TEXT    NOT NULL DEFAULT '[]',
            start_time       TEXT    NOT NULL,
            duration_minutes INTEGER DEFAULT 0,
            actions_json     TEXT    NOT NULL DEFAULT '[]',
            enabled          INTEGER DEFAULT 1,
            created_at       DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Phase 3: Request Policy
        CREATE TABLE IF NOT EXISTS request_policy (
            id          INTEGER PRIMARY KEY DEFAULT 1,
            policy_json TEXT    NOT NULL
        );

        CREATE TABLE IF NOT EXISTS request_log (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            song_id            INTEGER NOT NULL,
            song_title         TEXT,
            artist             TEXT,
            requester_name     TEXT,
            requester_platform TEXT,
            requester_ip       TEXT,
            requested_at       INTEGER DEFAULT (strftime('%s', 'now')),
            status             TEXT    DEFAULT 'pending',
            rejection_reason   TEXT,
            played_at          INTEGER
        );

        -- Phase 3: GAP Killer config
        CREATE TABLE IF NOT EXISTS gap_killer_config (
            id               INTEGER PRIMARY KEY DEFAULT 1,
            gap_killer_json  TEXT    NOT NULL DEFAULT '{"mode":"smart","threshold_db":-50.0,"min_silence_ms":500}'
        );

        -- Phase 6: Gateway connection settings
        CREATE TABLE IF NOT EXISTS gateway_config (
            id              INTEGER PRIMARY KEY DEFAULT 1,
            url             TEXT,
            token           TEXT,
            auto_connect    INTEGER DEFAULT 0,
            sync_queue      INTEGER DEFAULT 1,
            sync_vu         INTEGER DEFAULT 1,
            vu_throttle_ms  INTEGER DEFAULT 200
        );

        -- Phase 6: Remote DJ permissions
        CREATE TABLE IF NOT EXISTS remote_dj_permissions (
            user_id                 TEXT    PRIMARY KEY,
            can_load_track          INTEGER DEFAULT 0,
            can_play_pause          INTEGER DEFAULT 1,
            can_seek                INTEGER DEFAULT 0,
            can_set_volume          INTEGER DEFAULT 1,
            can_queue_add           INTEGER DEFAULT 1,
            can_queue_remove        INTEGER DEFAULT 0,
            can_trigger_crossfade   INTEGER DEFAULT 0,
            can_set_autopilot       INTEGER DEFAULT 0
        );

        -- Phase 6: Remote DJ session log
        CREATE TABLE IF NOT EXISTS remote_sessions_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id      TEXT    NOT NULL,
            user_id         TEXT    NOT NULL,
            display_name    TEXT,
            connected_at    INTEGER NOT NULL,
            disconnected_at INTEGER,
            commands_sent   INTEGER DEFAULT 0
        );
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

// ── Cue points ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuePoint {
    pub id: Option<i64>,
    pub song_id: i64,
    /// "start" | "end" | "intro" | "outro" | "fade" | "xfade" | "custom_0" … "custom_9"
    pub name: String,
    pub position_ms: i64,
}

pub async fn get_cue_points(pool: &SqlitePool, song_id: i64) -> Result<Vec<CuePoint>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, song_id, name, position_ms FROM cue_points WHERE song_id = ? ORDER BY position_ms",
    )
    .bind(song_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| CuePoint {
            id: r.get("id"),
            song_id: r.get("song_id"),
            name: r.get("name"),
            position_ms: r.get("position_ms"),
        })
        .collect())
}

pub async fn upsert_cue_point(pool: &SqlitePool, cue: &CuePoint) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cue_points (song_id, name, position_ms)
        VALUES (?, ?, ?)
        ON CONFLICT(song_id, name) DO UPDATE SET position_ms = excluded.position_ms
        "#,
    )
    .bind(cue.song_id)
    .bind(&cue.name)
    .bind(cue.position_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_cue_point(pool: &SqlitePool, song_id: i64, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cue_points WHERE song_id = ? AND name = ?")
        .bind(song_id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Song fade overrides ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SongFadeOverrideRow {
    pub song_id: i64,
    pub fade_out_enabled: Option<bool>,
    pub fade_out_curve: Option<String>,
    pub fade_out_time_ms: Option<i64>,
    pub fade_in_enabled: Option<bool>,
    pub fade_in_curve: Option<String>,
    pub fade_in_time_ms: Option<i64>,
    pub crossfade_mode: Option<String>,
    pub gain_db: Option<f64>,
}

pub async fn get_song_fade_override(
    pool: &SqlitePool,
    song_id: i64,
) -> Result<Option<SongFadeOverrideRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT * FROM song_fade_overrides WHERE song_id = ?",
    )
    .bind(song_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| SongFadeOverrideRow {
        song_id: r.get("song_id"),
        fade_out_enabled: r.get::<Option<i64>, _>("fade_out_enabled").map(|v| v != 0),
        fade_out_curve: r.get("fade_out_curve"),
        fade_out_time_ms: r.get("fade_out_time_ms"),
        fade_in_enabled: r.get::<Option<i64>, _>("fade_in_enabled").map(|v| v != 0),
        fade_in_curve: r.get("fade_in_curve"),
        fade_in_time_ms: r.get("fade_in_time_ms"),
        crossfade_mode: r.get("crossfade_mode"),
        gain_db: r.get("gain_db"),
    }))
}

pub async fn upsert_song_fade_override(
    pool: &SqlitePool,
    row: &SongFadeOverrideRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO song_fade_overrides
            (song_id, fade_out_enabled, fade_out_curve, fade_out_time_ms,
             fade_in_enabled, fade_in_curve, fade_in_time_ms, crossfade_mode, gain_db)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(song_id) DO UPDATE SET
            fade_out_enabled = excluded.fade_out_enabled,
            fade_out_curve   = excluded.fade_out_curve,
            fade_out_time_ms = excluded.fade_out_time_ms,
            fade_in_enabled  = excluded.fade_in_enabled,
            fade_in_curve    = excluded.fade_in_curve,
            fade_in_time_ms  = excluded.fade_in_time_ms,
            crossfade_mode   = excluded.crossfade_mode,
            gain_db          = excluded.gain_db
        "#,
    )
    .bind(row.song_id)
    .bind(row.fade_out_enabled.map(|v| v as i64))
    .bind(&row.fade_out_curve)
    .bind(row.fade_out_time_ms)
    .bind(row.fade_in_enabled.map(|v| v as i64))
    .bind(&row.fade_in_curve)
    .bind(row.fade_in_time_ms)
    .bind(&row.crossfade_mode)
    .bind(row.gain_db)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Channel DSP settings ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDspRow {
    pub channel: String,
    pub eq_low_gain_db: f64,
    pub eq_low_freq_hz: f64,
    pub eq_mid_gain_db: f64,
    pub eq_mid_freq_hz: f64,
    pub eq_mid_q: f64,
    pub eq_high_gain_db: f64,
    pub eq_high_freq_hz: f64,
    pub agc_enabled: bool,
    pub agc_gate_db: f64,
    pub agc_max_gain_db: f64,
    pub agc_attack_ms: f64,
    pub agc_release_ms: f64,
    pub agc_pre_emphasis: String,
    pub comp_enabled: bool,
    pub comp_settings_json: Option<String>,
}

pub async fn get_channel_dsp(
    pool: &SqlitePool,
    channel: &str,
) -> Result<Option<ChannelDspRow>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM channel_dsp_settings WHERE channel = ?")
        .bind(channel)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| ChannelDspRow {
        channel: r.get("channel"),
        eq_low_gain_db: r.get("eq_low_gain_db"),
        eq_low_freq_hz: r.get("eq_low_freq_hz"),
        eq_mid_gain_db: r.get("eq_mid_gain_db"),
        eq_mid_freq_hz: r.get("eq_mid_freq_hz"),
        eq_mid_q: r.get("eq_mid_q"),
        eq_high_gain_db: r.get("eq_high_gain_db"),
        eq_high_freq_hz: r.get("eq_high_freq_hz"),
        agc_enabled: r.get::<i64, _>("agc_enabled") != 0,
        agc_gate_db: r.get("agc_gate_db"),
        agc_max_gain_db: r.get("agc_max_gain_db"),
        agc_attack_ms: r.get("agc_attack_ms"),
        agc_release_ms: r.get("agc_release_ms"),
        agc_pre_emphasis: r.get("agc_pre_emphasis"),
        comp_enabled: r.get::<i64, _>("comp_enabled") != 0,
        comp_settings_json: r.get("comp_settings_json"),
    }))
}

pub async fn upsert_channel_dsp(pool: &SqlitePool, row: &ChannelDspRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO channel_dsp_settings
            (channel, eq_low_gain_db, eq_low_freq_hz, eq_mid_gain_db, eq_mid_freq_hz, eq_mid_q,
             eq_high_gain_db, eq_high_freq_hz, agc_enabled, agc_gate_db, agc_max_gain_db,
             agc_attack_ms, agc_release_ms, agc_pre_emphasis, comp_enabled, comp_settings_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(channel) DO UPDATE SET
            eq_low_gain_db   = excluded.eq_low_gain_db,
            eq_low_freq_hz   = excluded.eq_low_freq_hz,
            eq_mid_gain_db   = excluded.eq_mid_gain_db,
            eq_mid_freq_hz   = excluded.eq_mid_freq_hz,
            eq_mid_q         = excluded.eq_mid_q,
            eq_high_gain_db  = excluded.eq_high_gain_db,
            eq_high_freq_hz  = excluded.eq_high_freq_hz,
            agc_enabled      = excluded.agc_enabled,
            agc_gate_db      = excluded.agc_gate_db,
            agc_max_gain_db  = excluded.agc_max_gain_db,
            agc_attack_ms    = excluded.agc_attack_ms,
            agc_release_ms   = excluded.agc_release_ms,
            agc_pre_emphasis = excluded.agc_pre_emphasis,
            comp_enabled     = excluded.comp_enabled,
            comp_settings_json = excluded.comp_settings_json
        "#,
    )
    .bind(&row.channel)
    .bind(row.eq_low_gain_db)
    .bind(row.eq_low_freq_hz)
    .bind(row.eq_mid_gain_db)
    .bind(row.eq_mid_freq_hz)
    .bind(row.eq_mid_q)
    .bind(row.eq_high_gain_db)
    .bind(row.eq_high_freq_hz)
    .bind(row.agc_enabled as i64)
    .bind(row.agc_gate_db)
    .bind(row.agc_max_gain_db)
    .bind(row.agc_attack_ms)
    .bind(row.agc_release_ms)
    .bind(&row.agc_pre_emphasis)
    .bind(row.comp_enabled as i64)
    .bind(&row.comp_settings_json)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Crossfade config ─────────────────────────────────────────────────────────

pub async fn load_crossfade_config(pool: &SqlitePool) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query("SELECT config_json FROM crossfade_config WHERE id = 1")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>("config_json")))
}

pub async fn save_crossfade_config(pool: &SqlitePool, json: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO crossfade_config (id, config_json) VALUES (1, ?)
        ON CONFLICT(id) DO UPDATE SET config_json = excluded.config_json
        "#,
    )
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Phase 6: Gateway config ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub url: Option<String>,
    pub token: Option<String>,
    pub auto_connect: bool,
    pub sync_queue: bool,
    pub sync_vu: bool,
    pub vu_throttle_ms: i64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            url: None,
            token: None,
            auto_connect: false,
            sync_queue: true,
            sync_vu: true,
            vu_throttle_ms: 200,
        }
    }
}

pub async fn get_gateway_config(pool: &SqlitePool) -> Result<GatewayConfig, sqlx::Error> {
    let row = sqlx::query(
        "SELECT url, token, auto_connect, sync_queue, sync_vu, vu_throttle_ms FROM gateway_config WHERE id = 1"
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(GatewayConfig {
            url: r.get("url"),
            token: r.get("token"),
            auto_connect: r.get::<i64, _>("auto_connect") != 0,
            sync_queue: r.get::<i64, _>("sync_queue") != 0,
            sync_vu: r.get::<i64, _>("sync_vu") != 0,
            vu_throttle_ms: r.get("vu_throttle_ms"),
        }),
        None => Ok(GatewayConfig::default()),
    }
}

pub async fn save_gateway_config(pool: &SqlitePool, config: &GatewayConfig) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO gateway_config (id, url, token, auto_connect, sync_queue, sync_vu, vu_throttle_ms)
        VALUES (1, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            token = excluded.token,
            auto_connect = excluded.auto_connect,
            sync_queue = excluded.sync_queue,
            sync_vu = excluded.sync_vu,
            vu_throttle_ms = excluded.vu_throttle_ms
        "#,
    )
    .bind(&config.url)
    .bind(&config.token)
    .bind(config.auto_connect as i64)
    .bind(config.sync_queue as i64)
    .bind(config.sync_vu as i64)
    .bind(config.vu_throttle_ms)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Phase 6: Remote DJ permissions ───────────────────────────────────────────

use crate::gateway::remote_dj::DjPermissions;

pub async fn get_dj_permissions(pool: &SqlitePool, user_id: &str) -> Result<DjPermissions, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT can_load_track, can_play_pause, can_seek, can_set_volume,
               can_queue_add, can_queue_remove, can_trigger_crossfade, can_set_autopilot
        FROM remote_dj_permissions WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(DjPermissions {
            can_load_track: r.get::<i64, _>("can_load_track") != 0,
            can_play_pause: r.get::<i64, _>("can_play_pause") != 0,
            can_seek: r.get::<i64, _>("can_seek") != 0,
            can_set_volume: r.get::<i64, _>("can_set_volume") != 0,
            can_queue_add: r.get::<i64, _>("can_queue_add") != 0,
            can_queue_remove: r.get::<i64, _>("can_queue_remove") != 0,
            can_trigger_crossfade: r.get::<i64, _>("can_trigger_crossfade") != 0,
            can_set_autopilot: r.get::<i64, _>("can_set_autopilot") != 0,
        }),
        None => Ok(DjPermissions::default()),
    }
}

pub async fn save_dj_permissions(
    pool: &SqlitePool,
    user_id: &str,
    perms: &DjPermissions,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO remote_dj_permissions (
            user_id, can_load_track, can_play_pause, can_seek, can_set_volume,
            can_queue_add, can_queue_remove, can_trigger_crossfade, can_set_autopilot
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(user_id) DO UPDATE SET
            can_load_track = excluded.can_load_track,
            can_play_pause = excluded.can_play_pause,
            can_seek = excluded.can_seek,
            can_set_volume = excluded.can_set_volume,
            can_queue_add = excluded.can_queue_add,
            can_queue_remove = excluded.can_queue_remove,
            can_trigger_crossfade = excluded.can_trigger_crossfade,
            can_set_autopilot = excluded.can_set_autopilot
        "#,
    )
    .bind(user_id)
    .bind(perms.can_load_track as i64)
    .bind(perms.can_play_pause as i64)
    .bind(perms.can_seek as i64)
    .bind(perms.can_set_volume as i64)
    .bind(perms.can_queue_add as i64)
    .bind(perms.can_queue_remove as i64)
    .bind(perms.can_trigger_crossfade as i64)
    .bind(perms.can_set_autopilot as i64)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Phase 6: Remote session log ──────────────────────────────────────────────

pub async fn log_remote_session_start(
    pool: &SqlitePool,
    session_id: &str,
    user_id: &str,
    display_name: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    sqlx::query(
        r#"
        INSERT INTO remote_sessions_log (session_id, user_id, display_name, connected_at)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(display_name)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn log_remote_session_end(
    pool: &SqlitePool,
    session_id: &str,
    commands_sent: u32,
) -> Result<(), sqlx::Error> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    sqlx::query(
        r#"
        UPDATE remote_sessions_log
        SET disconnected_at = ?, commands_sent = ?
        WHERE session_id = ? AND disconnected_at IS NULL
        "#,
    )
    .bind(now_ms)
    .bind(commands_sent as i64)
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

