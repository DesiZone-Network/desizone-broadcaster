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
            cue_kind    TEXT    NOT NULL DEFAULT 'memory',
            slot        INTEGER,
            label       TEXT    NOT NULL DEFAULT '',
            color_hex   TEXT    NOT NULL DEFAULT '#f59e0b',
            updated_at  INTEGER NOT NULL DEFAULT (strftime('%s','now')),
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
            comp_settings_json  TEXT,
            pipeline_settings_json TEXT
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

        -- Runtime DJ mode (manual/assisted/autodj)
        CREATE TABLE IF NOT EXISTS dj_runtime_config (
            id           INTEGER PRIMARY KEY DEFAULT 1,
            dj_mode      TEXT    NOT NULL DEFAULT 'manual',
            updated_at   INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );

        -- AutoDJ transition mode/config
        CREATE TABLE IF NOT EXISTS autodj_transition_config (
            id           INTEGER PRIMARY KEY DEFAULT 1,
            config_json  TEXT    NOT NULL
        );

        -- SAM-style clockwheel config + runtime cursor
        CREATE TABLE IF NOT EXISTS autodj_clockwheel_config (
            id           INTEGER PRIMARY KEY DEFAULT 1,
            config_json  TEXT    NOT NULL
        );

        CREATE TABLE IF NOT EXISTS autodj_clockwheel_state (
            id           INTEGER PRIMARY KEY DEFAULT 1,
            next_index   INTEGER NOT NULL DEFAULT 0,
            updated_at   INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );

        -- Cached waveform peaks for deck visualisation
        CREATE TABLE IF NOT EXISTS waveform_cache (
            file_path    TEXT    NOT NULL,
            mtime_ms     INTEGER NOT NULL,
            resolution   INTEGER NOT NULL,
            peaks_json   TEXT    NOT NULL,
            updated_at   INTEGER NOT NULL,
            PRIMARY KEY (file_path, mtime_ms, resolution)
        );

        CREATE TABLE IF NOT EXISTS beatgrid_analysis (
            song_id        INTEGER PRIMARY KEY,
            file_path      TEXT    NOT NULL,
            mtime_ms       INTEGER NOT NULL,
            bpm            REAL    NOT NULL,
            first_beat_ms  INTEGER NOT NULL,
            confidence     REAL    NOT NULL,
            beat_times_json TEXT   NOT NULL,
            updated_at     INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS stem_analysis (
            song_id               INTEGER PRIMARY KEY,
            source_file_path      TEXT    NOT NULL,
            source_mtime_ms       INTEGER NOT NULL,
            vocals_file_path      TEXT    NOT NULL,
            instrumental_file_path TEXT   NOT NULL,
            model_name            TEXT    NOT NULL,
            updated_at            INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS monitor_routing_config (
            id               INTEGER PRIMARY KEY DEFAULT 1,
            master_device_id TEXT,
            cue_device_id    TEXT,
            cue_mix_mode     TEXT    NOT NULL DEFAULT 'split',
            cue_level        REAL    NOT NULL DEFAULT 1.0,
            master_level     REAL    NOT NULL DEFAULT 1.0
        );

        CREATE TABLE IF NOT EXISTS controller_config (
            id                  INTEGER PRIMARY KEY DEFAULT 1,
            enabled             INTEGER NOT NULL DEFAULT 1,
            auto_connect        INTEGER NOT NULL DEFAULT 1,
            preferred_device_id TEXT,
            profile             TEXT    NOT NULL DEFAULT 'hercules_djcontrol_starlight',
            updated_at          INTEGER NOT NULL DEFAULT (strftime('%s','now'))
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

        -- Phase 7: Play statistics cache
        CREATE TABLE IF NOT EXISTS play_stats_cache (
            song_id         INTEGER NOT NULL,
            period          TEXT    NOT NULL,
            play_count      INTEGER DEFAULT 0,
            total_played_ms INTEGER DEFAULT 0,
            last_played_at  INTEGER,
            skip_count      INTEGER DEFAULT 0,
            PRIMARY KEY (song_id, period)
        );

        -- Phase 7: Hourly play counts
        CREATE TABLE IF NOT EXISTS hourly_play_counts (
            date            TEXT    NOT NULL,
            hour            INTEGER NOT NULL,
            play_count      INTEGER DEFAULT 0,
            unique_songs    INTEGER DEFAULT 0,
            PRIMARY KEY (date, hour)
        );

        -- Phase 7: Event log
        CREATE TABLE IF NOT EXISTS event_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp       INTEGER NOT NULL,
            level           TEXT    NOT NULL,
            category        TEXT    NOT NULL,
            event           TEXT    NOT NULL,
            message         TEXT    NOT NULL,
            metadata_json   TEXT,
            deck            TEXT,
            song_id         INTEGER,
            encoder_id      INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_event_log_timestamp ON event_log(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_event_log_category ON event_log(category);
        CREATE INDEX IF NOT EXISTS idx_event_log_level ON event_log(level);

        -- Phase 7: System health snapshots
        CREATE TABLE IF NOT EXISTS system_health_snapshots (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp               INTEGER NOT NULL,
            cpu_pct                 REAL,
            memory_mb               REAL,
            ring_buffer_fill_deck_a REAL,
            ring_buffer_fill_deck_b REAL,
            decoder_latency_ms      REAL,
            stream_connected        INTEGER,
            mysql_connected         INTEGER,
            active_encoders         INTEGER
        );

        -- SAM Broadcaster MySQL connection settings
        CREATE TABLE IF NOT EXISTS sam_db_config (
            id               INTEGER PRIMARY KEY DEFAULT 1,
            host             TEXT    NOT NULL DEFAULT '127.0.0.1',
            port             INTEGER NOT NULL DEFAULT 3306,
            username         TEXT    NOT NULL DEFAULT '',
            password         TEXT    NOT NULL DEFAULT '',
            database_name    TEXT    NOT NULL DEFAULT 'samdb',
            auto_connect     INTEGER NOT NULL DEFAULT 0,
            path_prefix_from TEXT    NOT NULL DEFAULT '',
            path_prefix_to   TEXT    NOT NULL DEFAULT ''
        );
        "#,
    )
    .execute(pool)
    .await?;

    // Backward-compat migration for older DBs created before `pipeline_settings_json`.
    let _ = sqlx::query("ALTER TABLE channel_dsp_settings ADD COLUMN pipeline_settings_json TEXT")
        .execute(pool)
        .await;
    // Backward-compat migrations for cue_points schema expansion.
    let _ =
        sqlx::query("ALTER TABLE cue_points ADD COLUMN cue_kind TEXT NOT NULL DEFAULT 'memory'")
            .execute(pool)
            .await;
    let _ = sqlx::query("ALTER TABLE cue_points ADD COLUMN slot INTEGER")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE cue_points ADD COLUMN label TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ =
        sqlx::query("ALTER TABLE cue_points ADD COLUMN color_hex TEXT NOT NULL DEFAULT '#f59e0b'")
            .execute(pool)
            .await;
    let _ = sqlx::query(
        "ALTER TABLE cue_points ADD COLUMN updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_cue_points_song_kind_slot ON cue_points(song_id, cue_kind, slot) WHERE slot IS NOT NULL",
    )
    .execute(pool)
    .await;

    Ok(())
}

// ── Cue points ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Hotcue,
    Memory,
    Transition,
}

impl CueKind {
    fn from_db(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "hotcue" => Self::Hotcue,
            "transition" => Self::Transition,
            _ => Self::Memory,
        }
    }

    fn as_db(self) -> &'static str {
        match self {
            Self::Hotcue => "hotcue",
            Self::Memory => "memory",
            Self::Transition => "transition",
        }
    }
}

impl Default for CueKind {
    fn default() -> Self {
        Self::Memory
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CueQuantize {
    Off,
    Beat1,
    BeatHalf,
    BeatQuarter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuePoint {
    pub id: Option<i64>,
    pub song_id: i64,
    /// "start" | "end" | "intro" | "outro" | "fade" | "xfade" | "custom_0" … "custom_9"
    pub name: String,
    pub position_ms: i64,
    pub cue_kind: CueKind,
    pub slot: Option<i64>,
    pub label: String,
    pub color_hex: String,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotCue {
    pub song_id: i64,
    pub slot: u8,
    pub position_ms: i64,
    pub label: String,
    pub color_hex: String,
    pub quantized: bool,
}

pub async fn get_cue_points(pool: &SqlitePool, song_id: i64) -> Result<Vec<CuePoint>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, song_id, name, position_ms, cue_kind, slot, label, color_hex, updated_at
         FROM cue_points WHERE song_id = ? ORDER BY position_ms",
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
            cue_kind: CueKind::from_db(r.get::<String, _>("cue_kind").as_str()),
            slot: r.get("slot"),
            label: r.get("label"),
            color_hex: r.get("color_hex"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

pub async fn upsert_cue_point(pool: &SqlitePool, cue: &CuePoint) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cue_points (song_id, name, position_ms, cue_kind, slot, label, color_hex, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, strftime('%s','now'))
        ON CONFLICT(song_id, name) DO UPDATE SET
            position_ms = excluded.position_ms,
            cue_kind = excluded.cue_kind,
            slot = excluded.slot,
            label = excluded.label,
            color_hex = excluded.color_hex,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(cue.song_id)
    .bind(&cue.name)
    .bind(cue.position_ms)
    .bind(cue.cue_kind.as_db())
    .bind(cue.slot)
    .bind(if cue.label.is_empty() {
        cue.name.clone()
    } else {
        cue.label.clone()
    })
    .bind(if cue.color_hex.is_empty() {
        "#f59e0b".to_string()
    } else {
        cue.color_hex.clone()
    })
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_cue_point(
    pool: &SqlitePool,
    song_id: i64,
    name: &str,
) -> Result<(), sqlx::Error> {
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
    let row = sqlx::query("SELECT * FROM song_fade_overrides WHERE song_id = ?")
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

pub async fn get_hot_cues(pool: &SqlitePool, song_id: i64) -> Result<Vec<HotCue>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT song_id, slot, position_ms, label, color_hex
         FROM cue_points
         WHERE song_id = ? AND cue_kind = 'hotcue' AND slot IS NOT NULL
         ORDER BY slot ASC",
    )
    .bind(song_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let slot = r.get::<i64, _>("slot");
            if !(1..=8).contains(&slot) {
                return None;
            }
            Some(HotCue {
                song_id: r.get("song_id"),
                slot: slot as u8,
                position_ms: r.get("position_ms"),
                label: r.get("label"),
                color_hex: r.get("color_hex"),
                quantized: false,
            })
        })
        .collect())
}

pub async fn get_hot_cue(
    pool: &SqlitePool,
    song_id: i64,
    slot: u8,
) -> Result<Option<HotCue>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT song_id, slot, position_ms, label, color_hex
         FROM cue_points
         WHERE song_id = ? AND cue_kind = 'hotcue' AND slot = ?",
    )
    .bind(song_id)
    .bind(slot as i64)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| HotCue {
        song_id: r.get("song_id"),
        slot: r.get::<i64, _>("slot") as u8,
        position_ms: r.get("position_ms"),
        label: r.get("label"),
        color_hex: r.get("color_hex"),
        quantized: false,
    }))
}

pub async fn upsert_hot_cue(pool: &SqlitePool, cue: &HotCue) -> Result<(), sqlx::Error> {
    let cue_name = format!("hotcue_{}", cue.slot);
    sqlx::query(
        r#"
        INSERT INTO cue_points (song_id, name, position_ms, cue_kind, slot, label, color_hex, updated_at)
        VALUES (?, ?, ?, 'hotcue', ?, ?, ?, strftime('%s','now'))
        ON CONFLICT(song_id, cue_kind, slot) DO UPDATE SET
            name = excluded.name,
            position_ms = excluded.position_ms,
            label = excluded.label,
            color_hex = excluded.color_hex,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(cue.song_id)
    .bind(cue_name)
    .bind(cue.position_ms)
    .bind(cue.slot as i64)
    .bind(if cue.label.is_empty() {
        format!("Cue {}", cue.slot)
    } else {
        cue.label.clone()
    })
    .bind(if cue.color_hex.is_empty() {
        "#f59e0b".to_string()
    } else {
        cue.color_hex.clone()
    })
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_hot_cue(pool: &SqlitePool, song_id: i64, slot: u8) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cue_points WHERE song_id = ? AND cue_kind = 'hotcue' AND slot = ?")
        .bind(song_id)
        .bind(slot as i64)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rename_hot_cue(
    pool: &SqlitePool,
    song_id: i64,
    slot: u8,
    label: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE cue_points SET label = ?, updated_at = strftime('%s','now')
         WHERE song_id = ? AND cue_kind = 'hotcue' AND slot = ?",
    )
    .bind(label)
    .bind(song_id)
    .bind(slot as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn recolor_hot_cue(
    pool: &SqlitePool,
    song_id: i64,
    slot: u8,
    color_hex: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE cue_points SET color_hex = ?, updated_at = strftime('%s','now')
         WHERE song_id = ? AND cue_kind = 'hotcue' AND slot = ?",
    )
    .bind(color_hex)
    .bind(song_id)
    .bind(slot as i64)
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
    pub pipeline_settings_json: Option<String>,
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
        pipeline_settings_json: r.try_get("pipeline_settings_json").ok(),
    }))
}

pub async fn upsert_channel_dsp(pool: &SqlitePool, row: &ChannelDspRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO channel_dsp_settings
            (channel, eq_low_gain_db, eq_low_freq_hz, eq_mid_gain_db, eq_mid_freq_hz, eq_mid_q,
             eq_high_gain_db, eq_high_freq_hz, agc_enabled, agc_gate_db, agc_max_gain_db,
             agc_attack_ms, agc_release_ms, agc_pre_emphasis, comp_enabled, comp_settings_json, pipeline_settings_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            comp_settings_json = excluded.comp_settings_json,
            pipeline_settings_json = excluded.pipeline_settings_json
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
    .bind(&row.pipeline_settings_json)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Runtime DJ mode ──────────────────────────────────────────────────────────

pub async fn get_runtime_dj_mode(pool: &SqlitePool) -> Result<String, sqlx::Error> {
    let row = sqlx::query("SELECT dj_mode FROM dj_runtime_config WHERE id = 1")
        .fetch_optional(pool)
        .await?;
    Ok(row
        .map(|r| r.get::<String, _>("dj_mode"))
        .unwrap_or_else(|| "manual".to_string()))
}

pub async fn save_runtime_dj_mode(pool: &SqlitePool, mode: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO dj_runtime_config (id, dj_mode, updated_at)
        VALUES (1, ?, strftime('%s','now'))
        ON CONFLICT(id) DO UPDATE SET
            dj_mode = excluded.dj_mode,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(mode)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_autodj_transition_config(
    pool: &SqlitePool,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query("SELECT config_json FROM autodj_transition_config WHERE id = 1")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>("config_json")))
}

pub async fn save_autodj_transition_config(
    pool: &SqlitePool,
    json: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO autodj_transition_config (id, config_json) VALUES (1, ?)
        ON CONFLICT(id) DO UPDATE SET config_json = excluded.config_json
        "#,
    )
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Waveform cache ───────────────────────────────────────────────────────────

pub async fn get_waveform_cache(
    pool: &SqlitePool,
    file_path: &str,
    mtime_ms: i64,
    resolution: i64,
) -> Result<Option<Vec<f32>>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT peaks_json FROM waveform_cache WHERE file_path = ? AND mtime_ms = ? AND resolution = ?",
    )
    .bind(file_path)
    .bind(mtime_ms)
    .bind(resolution)
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else {
        return Ok(None);
    };
    let json: String = r.get("peaks_json");
    let parsed = serde_json::from_str::<Vec<f32>>(&json).unwrap_or_default();
    Ok(Some(parsed))
}

pub async fn save_waveform_cache(
    pool: &SqlitePool,
    file_path: &str,
    mtime_ms: i64,
    resolution: i64,
    peaks: &[f32],
) -> Result<(), sqlx::Error> {
    let peaks_json = serde_json::to_string(peaks).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        r#"
        INSERT INTO waveform_cache (file_path, mtime_ms, resolution, peaks_json, updated_at)
        VALUES (?, ?, ?, ?, strftime('%s','now'))
        ON CONFLICT(file_path, mtime_ms, resolution) DO UPDATE SET
            peaks_json = excluded.peaks_json,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(file_path)
    .bind(mtime_ms)
    .bind(resolution)
    .bind(peaks_json)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Beat-grid cache ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatGridAnalysis {
    pub song_id: i64,
    pub file_path: String,
    pub mtime_ms: i64,
    pub bpm: f32,
    pub first_beat_ms: i64,
    pub confidence: f32,
    pub beat_times_ms: Vec<i64>,
    pub updated_at: Option<i64>,
}

pub async fn get_beatgrid_analysis(
    pool: &SqlitePool,
    song_id: i64,
    file_path: &str,
    mtime_ms: i64,
) -> Result<Option<BeatGridAnalysis>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT song_id, file_path, mtime_ms, bpm, first_beat_ms, confidence, beat_times_json, updated_at
         FROM beatgrid_analysis WHERE song_id = ? AND file_path = ? AND mtime_ms = ?",
    )
    .bind(song_id)
    .bind(file_path)
    .bind(mtime_ms)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| BeatGridAnalysis {
        song_id: r.get("song_id"),
        file_path: r.get("file_path"),
        mtime_ms: r.get("mtime_ms"),
        bpm: r.get::<f64, _>("bpm") as f32,
        first_beat_ms: r.get("first_beat_ms"),
        confidence: r.get::<f64, _>("confidence") as f32,
        beat_times_ms: serde_json::from_str::<Vec<i64>>(&r.get::<String, _>("beat_times_json"))
            .unwrap_or_default(),
        updated_at: r.get("updated_at"),
    }))
}

pub async fn get_latest_beatgrid_by_song_id(
    pool: &SqlitePool,
    song_id: i64,
) -> Result<Option<BeatGridAnalysis>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT song_id, file_path, mtime_ms, bpm, first_beat_ms, confidence, beat_times_json, updated_at
         FROM beatgrid_analysis WHERE song_id = ? LIMIT 1",
    )
    .bind(song_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| BeatGridAnalysis {
        song_id: r.get("song_id"),
        file_path: r.get("file_path"),
        mtime_ms: r.get("mtime_ms"),
        bpm: r.get::<f64, _>("bpm") as f32,
        first_beat_ms: r.get("first_beat_ms"),
        confidence: r.get::<f64, _>("confidence") as f32,
        beat_times_ms: serde_json::from_str::<Vec<i64>>(&r.get::<String, _>("beat_times_json"))
            .unwrap_or_default(),
        updated_at: r.get("updated_at"),
    }))
}

pub async fn save_beatgrid_analysis(
    pool: &SqlitePool,
    analysis: &BeatGridAnalysis,
) -> Result<(), sqlx::Error> {
    let beat_times_json =
        serde_json::to_string(&analysis.beat_times_ms).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        r#"
        INSERT INTO beatgrid_analysis
            (song_id, file_path, mtime_ms, bpm, first_beat_ms, confidence, beat_times_json, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, strftime('%s','now'))
        ON CONFLICT(song_id) DO UPDATE SET
            file_path = excluded.file_path,
            mtime_ms = excluded.mtime_ms,
            bpm = excluded.bpm,
            first_beat_ms = excluded.first_beat_ms,
            confidence = excluded.confidence,
            beat_times_json = excluded.beat_times_json,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(analysis.song_id)
    .bind(&analysis.file_path)
    .bind(analysis.mtime_ms)
    .bind(analysis.bpm as f64)
    .bind(analysis.first_beat_ms)
    .bind(analysis.confidence as f64)
    .bind(beat_times_json)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Stem analysis cache ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StemAnalysis {
    pub song_id: i64,
    pub source_file_path: String,
    pub source_mtime_ms: i64,
    pub vocals_file_path: String,
    pub instrumental_file_path: String,
    pub model_name: String,
    pub updated_at: Option<i64>,
}

pub async fn get_stem_analysis(
    pool: &SqlitePool,
    song_id: i64,
    source_file_path: &str,
    source_mtime_ms: i64,
) -> Result<Option<StemAnalysis>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT song_id, source_file_path, source_mtime_ms, vocals_file_path, instrumental_file_path, model_name, updated_at
         FROM stem_analysis WHERE song_id = ? AND source_file_path = ? AND source_mtime_ms = ?",
    )
    .bind(song_id)
    .bind(source_file_path)
    .bind(source_mtime_ms)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_stem_analysis_row))
}

pub async fn get_latest_stem_analysis_by_song_id(
    pool: &SqlitePool,
    song_id: i64,
) -> Result<Option<StemAnalysis>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT song_id, source_file_path, source_mtime_ms, vocals_file_path, instrumental_file_path, model_name, updated_at
         FROM stem_analysis WHERE song_id = ? LIMIT 1",
    )
    .bind(song_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_stem_analysis_row))
}

pub async fn save_stem_analysis(
    pool: &SqlitePool,
    analysis: &StemAnalysis,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO stem_analysis
            (song_id, source_file_path, source_mtime_ms, vocals_file_path, instrumental_file_path, model_name, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, strftime('%s','now'))
        ON CONFLICT(song_id) DO UPDATE SET
            source_file_path = excluded.source_file_path,
            source_mtime_ms = excluded.source_mtime_ms,
            vocals_file_path = excluded.vocals_file_path,
            instrumental_file_path = excluded.instrumental_file_path,
            model_name = excluded.model_name,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(analysis.song_id)
    .bind(&analysis.source_file_path)
    .bind(analysis.source_mtime_ms)
    .bind(&analysis.vocals_file_path)
    .bind(&analysis.instrumental_file_path)
    .bind(&analysis.model_name)
    .execute(pool)
    .await?;
    Ok(())
}

fn map_stem_analysis_row(r: sqlx::sqlite::SqliteRow) -> StemAnalysis {
    StemAnalysis {
        song_id: r.get("song_id"),
        source_file_path: r.get("source_file_path"),
        source_mtime_ms: r.get("source_mtime_ms"),
        vocals_file_path: r.get("vocals_file_path"),
        instrumental_file_path: r.get("instrumental_file_path"),
        model_name: r.get("model_name"),
        updated_at: r.get("updated_at"),
    }
}

// ── Cue monitor routing ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorRoutingConfig {
    pub master_device_id: Option<String>,
    pub cue_device_id: Option<String>,
    pub cue_mix_mode: String,
    pub cue_level: f32,
    pub master_level: f32,
}

impl Default for MonitorRoutingConfig {
    fn default() -> Self {
        Self {
            master_device_id: None,
            cue_device_id: None,
            cue_mix_mode: "split".to_string(),
            cue_level: 1.0,
            master_level: 1.0,
        }
    }
}

pub async fn get_monitor_routing_config(
    pool: &SqlitePool,
) -> Result<MonitorRoutingConfig, sqlx::Error> {
    let row = sqlx::query(
        "SELECT master_device_id, cue_device_id, cue_mix_mode, cue_level, master_level
         FROM monitor_routing_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(MonitorRoutingConfig {
            master_device_id: r.get("master_device_id"),
            cue_device_id: r.get("cue_device_id"),
            cue_mix_mode: r.get("cue_mix_mode"),
            cue_level: r.get::<f64, _>("cue_level") as f32,
            master_level: r.get::<f64, _>("master_level") as f32,
        }),
        None => Ok(MonitorRoutingConfig::default()),
    }
}

pub async fn save_monitor_routing_config(
    pool: &SqlitePool,
    config: &MonitorRoutingConfig,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO monitor_routing_config
            (id, master_device_id, cue_device_id, cue_mix_mode, cue_level, master_level)
        VALUES (1, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            master_device_id = excluded.master_device_id,
            cue_device_id = excluded.cue_device_id,
            cue_mix_mode = excluded.cue_mix_mode,
            cue_level = excluded.cue_level,
            master_level = excluded.master_level
        "#,
    )
    .bind(&config.master_device_id)
    .bind(&config.cue_device_id)
    .bind(&config.cue_mix_mode)
    .bind(config.cue_level as f64)
    .bind(config.master_level as f64)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Controller config ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerConfigRow {
    pub enabled: bool,
    pub auto_connect: bool,
    pub preferred_device_id: Option<String>,
    pub profile: String,
}

impl Default for ControllerConfigRow {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_connect: true,
            preferred_device_id: None,
            profile: "hercules_djcontrol_starlight".to_string(),
        }
    }
}

pub async fn get_controller_config(pool: &SqlitePool) -> Result<ControllerConfigRow, sqlx::Error> {
    let row = sqlx::query(
        "SELECT enabled, auto_connect, preferred_device_id, profile
         FROM controller_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(ControllerConfigRow {
            enabled: r.get::<i64, _>("enabled") != 0,
            auto_connect: r.get::<i64, _>("auto_connect") != 0,
            preferred_device_id: r.get("preferred_device_id"),
            profile: r.get("profile"),
        }),
        None => Ok(ControllerConfigRow::default()),
    }
}

pub async fn save_controller_config(
    pool: &SqlitePool,
    config: &ControllerConfigRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO controller_config
            (id, enabled, auto_connect, preferred_device_id, profile, updated_at)
        VALUES (1, ?, ?, ?, ?, strftime('%s','now'))
        ON CONFLICT(id) DO UPDATE SET
            enabled = excluded.enabled,
            auto_connect = excluded.auto_connect,
            preferred_device_id = excluded.preferred_device_id,
            profile = excluded.profile,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(config.enabled as i64)
    .bind(config.auto_connect as i64)
    .bind(&config.preferred_device_id)
    .bind(&config.profile)
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

pub async fn save_gateway_config(
    pool: &SqlitePool,
    config: &GatewayConfig,
) -> Result<(), sqlx::Error> {
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

pub async fn get_dj_permissions(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<DjPermissions, sqlx::Error> {
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

// ── SAM DB connection config ──────────────────────────────────────────────────

/// Stored SAM DB connection settings (password omitted from public-facing struct).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamDbConfig {
    pub host: String,
    pub port: i64,
    pub username: String,
    pub database_name: String,
    pub auto_connect: bool,
    /// Windows-style path prefix to replace (e.g. `C:\Music\`). Empty = no translation.
    pub path_prefix_from: String,
    /// Local path to substitute in (e.g. `/Volumes/Music/`). Empty = no translation.
    pub path_prefix_to: String,
}

impl Default for SamDbConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 3306,
            username: String::new(),
            database_name: "samdb".into(),
            auto_connect: false,
            path_prefix_from: String::new(),
            path_prefix_to: String::new(),
        }
    }
}

/// Internal row that includes the password — only used at startup for auto-connect.
pub struct SamDbConfigFull {
    pub config: SamDbConfig,
    pub password: String,
}

/// Load SAM DB config (without password).
pub async fn get_sam_db_config(pool: &SqlitePool) -> Result<SamDbConfig, sqlx::Error> {
    let row = sqlx::query(
        "SELECT host, port, username, database_name, auto_connect, \
         path_prefix_from, path_prefix_to FROM sam_db_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(SamDbConfig {
            host: r.get("host"),
            port: r.get("port"),
            username: r.get("username"),
            database_name: r.get("database_name"),
            auto_connect: r.get::<i64, _>("auto_connect") != 0,
            path_prefix_from: r.get("path_prefix_from"),
            path_prefix_to: r.get("path_prefix_to"),
        }),
        None => Ok(SamDbConfig::default()),
    }
}

/// Load the full config including password — only for internal startup use.
pub async fn load_sam_db_config_full(
    pool: &SqlitePool,
) -> Result<Option<SamDbConfigFull>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT host, port, username, password, database_name, auto_connect, \
         path_prefix_from, path_prefix_to FROM sam_db_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| SamDbConfigFull {
        password: r.get("password"),
        config: SamDbConfig {
            host: r.get("host"),
            port: r.get("port"),
            username: r.get("username"),
            database_name: r.get("database_name"),
            auto_connect: r.get::<i64, _>("auto_connect") != 0,
            path_prefix_from: r.get("path_prefix_from"),
            path_prefix_to: r.get("path_prefix_to"),
        },
    }))
}

/// Save SAM DB config including password (stored locally only).
pub async fn save_sam_db_config(
    pool: &SqlitePool,
    config: &SamDbConfig,
    password: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO sam_db_config
            (id, host, port, username, password, database_name,
             auto_connect, path_prefix_from, path_prefix_to)
        VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            host             = excluded.host,
            port             = excluded.port,
            username         = excluded.username,
            password         = excluded.password,
            database_name    = excluded.database_name,
            auto_connect     = excluded.auto_connect,
            path_prefix_from = excluded.path_prefix_from,
            path_prefix_to   = excluded.path_prefix_to
        "#,
    )
    .bind(&config.host)
    .bind(config.port)
    .bind(&config.username)
    .bind(password)
    .bind(&config.database_name)
    .bind(config.auto_connect as i64)
    .bind(&config.path_prefix_from)
    .bind(&config.path_prefix_to)
    .execute(pool)
    .await?;
    Ok(())
}
