/// Playlist Rotation Engine
///
/// Selects the next track for AutoDJ based on active rotation rules.
/// Rules are evaluated against the recent play history to avoid repetition.
use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::{Datelike, NaiveDateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

// ── Rule types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RotationRule {
    /// Don't play same artist within N songs
    ArtistSeparation { min_songs: u32 },
    /// Don't play same artist within N minutes
    ArtistSeparationTime { min_minutes: u32 },
    /// Don't repeat same song within N songs
    SongSeparation { min_songs: u32 },
    /// Don't repeat same song within N minutes
    SongSeparationTime { min_minutes: u32 },
    /// Don't repeat same album within N songs
    AlbumSeparation { min_songs: u32 },
    /// Category rotation: cycle through categories in order
    CategoryRotation { sequence: Vec<String> },
    /// Maximum plays per song per N hours
    MaxPlaysPerHour {
        song_id: i64,
        max: u32,
        window_hours: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationRuleRow {
    pub id: Option<i64>,
    pub name: String,
    pub rule_type: String,
    pub config_json: String,
    pub enabled: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub config_json: String, // { categories: [], rules: [], shuffle: bool }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistSong {
    pub playlist_id: i64,
    pub song_id: i64,
    pub position: Option<i32>,
    pub weight: f64,
}

// ── SAM-style clockwheel config ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClockwheelSlotKind {
    Category,
    Directory,
    Request,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClockwheelSelectionMethod {
    Weighted,
    Priority,
    Random,
    MostRecentlyPlayedSong,
    LeastRecentlyPlayedSong,
    MostRecentlyPlayedArtist,
    LeastRecentlyPlayedArtist,
    Lemming,
    PlaylistOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockwheelSlot {
    pub id: String,
    pub kind: ClockwheelSlotKind,
    pub target: String,
    pub selection_method: ClockwheelSelectionMethod,
    pub enforce_rules: bool,
    pub start_hour: Option<u8>,
    pub end_hour: Option<u8>,
    pub active_days: Vec<u8>, // 0=Mon..6=Sun
}

impl Default for ClockwheelSlot {
    fn default() -> Self {
        Self {
            id: "slot-1".to_string(),
            kind: ClockwheelSlotKind::Category,
            target: String::new(),
            selection_method: ClockwheelSelectionMethod::Weighted,
            enforce_rules: true,
            start_hour: None,
            end_hour: None,
            active_days: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockwheelRules {
    pub no_same_album_minutes: u32,
    pub no_same_artist_minutes: u32,
    pub no_same_title_minutes: u32,
    pub no_same_track_minutes: u32,
    pub keep_songs_in_queue: u32,
    pub use_ghost_queue: bool,
    pub cache_queue_count: bool,
    pub enforce_playlist_rotation_rules: bool,
}

impl Default for ClockwheelRules {
    fn default() -> Self {
        Self {
            no_same_album_minutes: 15,
            no_same_artist_minutes: 8,
            no_same_title_minutes: 15,
            no_same_track_minutes: 180,
            keep_songs_in_queue: 1,
            use_ghost_queue: false,
            cache_queue_count: true,
            enforce_playlist_rotation_rules: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockwheelConfig {
    pub rules: ClockwheelRules,
    pub on_play_reduce_weight_by: f64,
    pub on_request_increase_weight_by: f64,
    pub verbose_logging: bool,
    pub slots: Vec<ClockwheelSlot>,
}

impl Default for ClockwheelConfig {
    fn default() -> Self {
        Self {
            rules: ClockwheelRules::default(),
            on_play_reduce_weight_by: 0.0,
            on_request_increase_weight_by: 0.0,
            verbose_logging: false,
            slots: vec![ClockwheelSlot::default()],
        }
    }
}

impl ClockwheelConfig {
    fn normalized(mut self) -> Self {
        self.on_play_reduce_weight_by = self.on_play_reduce_weight_by.max(0.0);
        self.on_request_increase_weight_by = self.on_request_increase_weight_by.max(0.0);

        if self.slots.is_empty() {
            self.slots.push(ClockwheelSlot::default());
        }

        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.id.trim().is_empty() {
                slot.id = format!("slot-{}", i + 1);
            }
            slot.start_hour = slot.start_hour.map(|h| h.min(23));
            slot.end_hour = slot.end_hour.map(|h| h.min(23));
            slot.active_days.retain(|d| *d <= 6);
            slot.active_days.sort_unstable();
            slot.active_days.dedup();
        }

        self
    }
}

// ── DB helpers ────────────────────────────────────────────────────────────────

pub async fn get_rotation_rules(pool: &SqlitePool) -> Result<Vec<RotationRuleRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, rule_type, config_json, enabled, priority FROM rotation_rules ORDER BY priority DESC, id ASC"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| RotationRuleRow {
            id: r.get("id"),
            name: r.get("name"),
            rule_type: r.get("rule_type"),
            config_json: r.get("config_json"),
            enabled: r.get::<i64, _>("enabled") != 0,
            priority: r.get("priority"),
        })
        .collect())
}

pub async fn upsert_rotation_rule(
    pool: &SqlitePool,
    rule: &RotationRuleRow,
) -> Result<i64, sqlx::Error> {
    let result = if let Some(id) = rule.id {
        sqlx::query(
            "UPDATE rotation_rules SET name=?, rule_type=?, config_json=?, enabled=?, priority=? WHERE id=?"
        )
        .bind(&rule.name)
        .bind(&rule.rule_type)
        .bind(&rule.config_json)
        .bind(rule.enabled as i64)
        .bind(rule.priority)
        .bind(id)
        .execute(pool)
        .await?;
        id
    } else {
        let r = sqlx::query(
            "INSERT INTO rotation_rules (name, rule_type, config_json, enabled, priority) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&rule.name)
        .bind(&rule.rule_type)
        .bind(&rule.config_json)
        .bind(rule.enabled as i64)
        .bind(rule.priority)
        .execute(pool)
        .await?;
        r.last_insert_rowid()
    };
    Ok(result)
}

pub async fn delete_rotation_rule(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM rotation_rules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_playlists(pool: &SqlitePool) -> Result<Vec<Playlist>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, description, is_active, config_json FROM rotation_playlists ORDER BY name"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Playlist {
            id: r.get("id"),
            name: r.get("name"),
            description: r.get("description"),
            is_active: r.get::<i64, _>("is_active") != 0,
            config_json: r.get("config_json"),
        })
        .collect())
}

pub async fn upsert_playlist(pool: &SqlitePool, playlist: &Playlist) -> Result<i64, sqlx::Error> {
    let result = if let Some(id) = playlist.id {
        sqlx::query(
            "UPDATE rotation_playlists SET name=?, description=?, is_active=?, config_json=? WHERE id=?"
        )
        .bind(&playlist.name)
        .bind(&playlist.description)
        .bind(playlist.is_active as i64)
        .bind(&playlist.config_json)
        .bind(id)
        .execute(pool)
        .await?;
        id
    } else {
        if playlist.is_active {
            sqlx::query("UPDATE rotation_playlists SET is_active = 0")
                .execute(pool)
                .await?;
        }
        let r = sqlx::query(
            "INSERT INTO rotation_playlists (name, description, is_active, config_json) VALUES (?, ?, ?, ?)"
        )
        .bind(&playlist.name)
        .bind(&playlist.description)
        .bind(playlist.is_active as i64)
        .bind(&playlist.config_json)
        .execute(pool)
        .await?;
        r.last_insert_rowid()
    };
    Ok(result)
}

pub async fn set_active_playlist(pool: &SqlitePool, playlist_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE rotation_playlists SET is_active = 0")
        .execute(pool)
        .await?;
    sqlx::query("UPDATE rotation_playlists SET is_active = 1 WHERE id = ?")
        .bind(playlist_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_clockwheel_config(pool: &SqlitePool) -> Result<ClockwheelConfig, sqlx::Error> {
    let row: Option<String> =
        sqlx::query_scalar("SELECT config_json FROM autodj_clockwheel_config WHERE id = 1")
            .fetch_optional(pool)
            .await?;

    let cfg = row
        .and_then(|j| serde_json::from_str::<ClockwheelConfig>(&j).ok())
        .unwrap_or_default()
        .normalized();

    Ok(cfg)
}

pub async fn save_clockwheel_config(
    pool: &SqlitePool,
    config: &ClockwheelConfig,
) -> Result<(), sqlx::Error> {
    let normalized = config.clone().normalized();
    let json = serde_json::to_string(&normalized).unwrap_or_else(|_| "{}".to_string());

    sqlx::query(
        r#"
        INSERT INTO autodj_clockwheel_config (id, config_json)
        VALUES (1, ?)
        ON CONFLICT(id) DO UPDATE SET config_json = excluded.config_json
        "#,
    )
    .bind(json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_song_directories(
    sam_pool: &MySqlPool,
    limit: u32,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT filename FROM songlist WHERE filename IS NOT NULL AND filename <> '' LIMIT ?",
    )
    .bind(limit)
    .fetch_all(sam_pool)
    .await?;

    let mut dirs = BTreeSet::new();
    for row in rows {
        let filename: String = row.try_get("filename").unwrap_or_default();
        let normalized = filename.replace('\\', "/");
        if let Some(idx) = normalized.rfind('/') {
            let dir = normalized[..idx].trim();
            if !dir.is_empty() {
                dirs.insert(dir.to_string());
            }
        }
    }

    Ok(dirs.into_iter().collect())
}

pub async fn apply_weight_delta_on_play(
    local_pool: &SqlitePool,
    sam_pool: &MySqlPool,
    song_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = get_clockwheel_config(local_pool).await.unwrap_or_default();
    let delta = -cfg.on_play_reduce_weight_by.abs();
    if delta.abs() < f64::EPSILON {
        return Ok(());
    }
    update_song_weight_by_delta(sam_pool, song_id, delta).await?;
    Ok(())
}

pub async fn apply_weight_delta_on_request(
    local_pool: &SqlitePool,
    sam_pool: &MySqlPool,
    song_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = get_clockwheel_config(local_pool).await.unwrap_or_default();
    let delta = cfg.on_request_increase_weight_by.max(0.0);
    if delta.abs() < f64::EPSILON {
        return Ok(());
    }
    update_song_weight_by_delta(sam_pool, song_id, delta).await?;
    Ok(())
}

async fn update_song_weight_by_delta(
    sam_pool: &MySqlPool,
    song_id: i64,
    delta: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE songlist SET weight = GREATEST(0, COALESCE(weight, 0) + ?) WHERE ID = ?")
        .bind(delta)
        .bind(song_id)
        .execute(sam_pool)
        .await?;
    Ok(())
}

// ── Next track selection ──────────────────────────────────────────────────────

/// Represents a candidate song from the SAM MySQL songlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongCandidate {
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub category: Option<String>,
    pub duration: i64,
    pub file_path: String,
    pub score: f64,
}

#[derive(Debug, Clone)]
struct CandidateInternal {
    song_id: i64,
    title: String,
    artist: String,
    album: String,
    category: Option<String>,
    duration: i64,
    file_path: String,
    weight: f64,
    count_played: i64,
    song_last_played_unix: i64,
}

#[derive(Debug, Clone)]
struct HistoryRow {
    song_id: i64,
    artist: String,
    title: String,
    album: String,
    played_unix: i64,
}

/// Select the next track for AutoDJ from the SAM `songlist` table,
/// applying enabled rules from both legacy rules and SAM-style clockwheel config.
pub async fn select_next_track(
    local_pool: &SqlitePool,
    sam_pool: &MySqlPool,
    active_category: Option<&str>,
) -> Result<Option<SongCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    select_next_track_with_exclusions(local_pool, sam_pool, active_category, None).await
}

pub async fn select_next_track_with_exclusions(
    local_pool: &SqlitePool,
    sam_pool: &MySqlPool,
    active_category: Option<&str>,
    excluded_song_ids: Option<&HashSet<i64>>,
) -> Result<Option<SongCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    let rules = get_rotation_rules(local_pool).await?;
    let enabled_rules: Vec<RotationRuleRow> = rules.into_iter().filter(|r| r.enabled).collect();

    let mut clockwheel = get_clockwheel_config(local_pool)
        .await
        .unwrap_or_default()
        .normalized();
    if let Some(category) = active_category {
        clockwheel.slots = vec![ClockwheelSlot {
            id: "active-category".to_string(),
            kind: ClockwheelSlotKind::Category,
            target: category.to_string(),
            selection_method: ClockwheelSelectionMethod::Weighted,
            enforce_rules: true,
            start_hour: None,
            end_hour: None,
            active_days: vec![],
        }];
    }

    let history = load_history(sam_pool).await;
    let now = Utc::now();

    let mut slots = clockwheel.slots.clone();
    if slots.is_empty() {
        slots.push(ClockwheelSlot::default());
    }

    let start_cursor = load_clockwheel_cursor(local_pool).await.unwrap_or(0) % slots.len();

    for offset in 0..slots.len() {
        let idx = (start_cursor + offset) % slots.len();
        let slot = &slots[idx];
        if !slot_is_active(slot, &now) {
            continue;
        }

        let mut candidates = fetch_candidates_for_slot(sam_pool, slot, 300).await?;
        if candidates.is_empty() {
            continue;
        }
        if let Some(excluded) = excluded_song_ids {
            candidates.retain(|c| !excluded.contains(&c.song_id));
            if candidates.is_empty() {
                continue;
            }
        }

        if slot.enforce_rules && clockwheel.rules.enforce_playlist_rotation_rules {
            apply_clockwheel_rules(
                &mut candidates,
                &history,
                &clockwheel.rules,
                now.timestamp(),
            );
        }

        if candidates.is_empty() {
            continue;
        }

        apply_legacy_rotation_rules(&mut candidates, &history, &enabled_rules, now.timestamp());

        if candidates.is_empty() {
            continue;
        }

        if let Some(chosen) =
            choose_candidate(candidates, slot.selection_method, &history, now.timestamp())
        {
            let _ = save_clockwheel_cursor(local_pool, (idx + 1) % slots.len()).await;
            return Ok(Some(SongCandidate {
                song_id: chosen.song_id,
                title: chosen.title,
                artist: chosen.artist,
                album: Some(chosen.album),
                category: chosen.category,
                duration: chosen.duration,
                file_path: chosen.file_path,
                score: chosen.weight,
            }));
        }
    }

    // If all slots are currently inactive due time windows, fallback to a generic
    // weighted pick so AutoDJ doesn't stall.
    let fallback_slot = ClockwheelSlot::default();
    let mut fallback = fetch_candidates_for_slot(sam_pool, &fallback_slot, 300).await?;
    if fallback.is_empty() {
        return Ok(None);
    }
    if let Some(excluded) = excluded_song_ids {
        fallback.retain(|c| !excluded.contains(&c.song_id));
        if fallback.is_empty() {
            return Ok(None);
        }
    }
    if clockwheel.rules.enforce_playlist_rotation_rules {
        apply_clockwheel_rules(&mut fallback, &history, &clockwheel.rules, now.timestamp());
    }
    apply_legacy_rotation_rules(&mut fallback, &history, &enabled_rules, now.timestamp());

    Ok(choose_candidate(
        fallback,
        ClockwheelSelectionMethod::Weighted,
        &history,
        now.timestamp(),
    )
    .map(|chosen| SongCandidate {
        song_id: chosen.song_id,
        title: chosen.title,
        artist: chosen.artist,
        album: Some(chosen.album),
        category: chosen.category,
        duration: chosen.duration,
        file_path: chosen.file_path,
        score: chosen.weight,
    }))
}

pub async fn select_next_track_for_slot(
    local_pool: &SqlitePool,
    sam_pool: &MySqlPool,
    slot_id: &str,
) -> Result<Option<SongCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    let clockwheel = get_clockwheel_config(local_pool)
        .await
        .unwrap_or_default()
        .normalized();
    let Some(slot) = clockwheel.slots.iter().find(|s| s.id == slot_id).cloned() else {
        return Ok(None);
    };

    let history = load_history(sam_pool).await;
    let now = Utc::now();
    if !slot_is_active(&slot, &now) {
        return Ok(None);
    }

    let mut candidates = fetch_candidates_for_slot(sam_pool, &slot, 300).await?;
    if candidates.is_empty() {
        return Ok(None);
    }

    if slot.enforce_rules && clockwheel.rules.enforce_playlist_rotation_rules {
        apply_clockwheel_rules(
            &mut candidates,
            &history,
            &clockwheel.rules,
            now.timestamp(),
        );
    }
    if candidates.is_empty() {
        return Ok(None);
    }

    let rules = get_rotation_rules(local_pool).await?;
    let enabled_rules: Vec<RotationRuleRow> = rules.into_iter().filter(|r| r.enabled).collect();
    apply_legacy_rotation_rules(&mut candidates, &history, &enabled_rules, now.timestamp());
    if candidates.is_empty() {
        return Ok(None);
    }

    Ok(
        choose_candidate(candidates, slot.selection_method, &history, now.timestamp()).map(
            |chosen| SongCandidate {
                song_id: chosen.song_id,
                title: chosen.title,
                artist: chosen.artist,
                album: Some(chosen.album),
                category: chosen.category,
                duration: chosen.duration,
                file_path: chosen.file_path,
                score: chosen.weight,
            },
        ),
    )
}

fn slot_is_active(slot: &ClockwheelSlot, now: &chrono::DateTime<Utc>) -> bool {
    if !slot.active_days.is_empty() {
        let day = now.weekday().num_days_from_monday() as u8;
        if !slot.active_days.contains(&day) {
            return false;
        }
    }

    match (slot.start_hour, slot.end_hour) {
        (Some(start), Some(end)) => {
            let h = now.hour() as u8;
            if start == end {
                true
            } else if start < end {
                h >= start && h < end
            } else {
                h >= start || h < end
            }
        }
        _ => true,
    }
}

async fn load_clockwheel_cursor(pool: &SqlitePool) -> Result<usize, sqlx::Error> {
    let row: Option<i64> =
        sqlx::query_scalar("SELECT next_index FROM autodj_clockwheel_state WHERE id = 1")
            .fetch_optional(pool)
            .await?;
    Ok(row.unwrap_or(0).max(0) as usize)
}

async fn save_clockwheel_cursor(pool: &SqlitePool, next_index: usize) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO autodj_clockwheel_state (id, next_index, updated_at)
        VALUES (1, ?, strftime('%s','now'))
        ON CONFLICT(id) DO UPDATE SET
          next_index = excluded.next_index,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(next_index as i64)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_candidates_for_slot(
    sam_pool: &MySqlPool,
    slot: &ClockwheelSlot,
    limit: u32,
) -> Result<Vec<CandidateInternal>, sqlx::Error> {
    let rows = match slot.kind {
        ClockwheelSlotKind::Category => {
            let target = slot.target.trim();
            if target.is_empty() {
                sqlx::query(
                    r#"SELECT ID as song_id,
                              title,
                              artist,
                              album,
                              category,
                              duration,
                              filename,
                              weight,
                              count_played,
                              UNIX_TIMESTAMP(date_played) as song_last_played_unix
                       FROM songlist
                       LIMIT ?"#,
                )
                .bind(limit)
                .fetch_all(sam_pool)
                .await?
            } else {
                // Primary path: resolve SAM categories and read songs through `categorylist`.
                let categories = crate::db::sam::get_categories(sam_pool).await?;
                let target_lc = target.to_lowercase();
                let target_norm = normalize_label(target);

                let mut matched: Vec<(i64, String)> = categories
                    .iter()
                    .filter(|c| c.catname.eq_ignore_ascii_case(target))
                    .map(|c| (c.id, c.catname.clone()))
                    .collect();

                if matched.is_empty() {
                    matched = categories
                        .iter()
                        .filter(|c| normalize_label(&c.catname) == target_norm)
                        .map(|c| (c.id, c.catname.clone()))
                        .collect();
                }

                if matched.is_empty() {
                    matched = categories
                        .iter()
                        .filter(|c| {
                            let cat_lc = c.catname.to_lowercase();
                            cat_lc.contains(&target_lc) || target_lc.contains(&cat_lc)
                        })
                        .map(|c| (c.id, c.catname.clone()))
                        .collect();
                }

                let mut out: Vec<CandidateInternal> = Vec::new();
                let mut seen_song_ids = HashSet::new();
                for (cat_id, cat_name) in &matched {
                    let songs =
                        crate::db::sam::get_songs_in_category(sam_pool, *cat_id, limit * 2, 0)
                            .await
                            .unwrap_or_default();
                    for song in songs {
                        if !seen_song_ids.insert(song.id) {
                            continue;
                        }
                        out.push(CandidateInternal {
                            song_id: song.id,
                            title: song.title,
                            artist: song.artist,
                            album: song.album,
                            category: Some(cat_name.clone()),
                            duration: song.duration as i64,
                            file_path: song.filename,
                            weight: song.weight,
                            count_played: song.count_played as i64,
                            song_last_played_unix: parse_sam_datetime_unix(
                                song.date_played.as_deref(),
                            ),
                        });
                        if out.len() >= limit as usize {
                            break;
                        }
                    }
                    if out.len() >= limit as usize {
                        break;
                    }
                }

                // Fallback for non-standard SAM schemas that expose `songlist.category`.
                if out.is_empty() {
                    sqlx::query(
                        r#"SELECT ID as song_id,
                                  title,
                                  artist,
                                  album,
                                  category,
                                  duration,
                                  filename,
                                  weight,
                                  count_played,
                                  UNIX_TIMESTAMP(date_played) as song_last_played_unix
                           FROM songlist
                           WHERE category LIKE ?
                           LIMIT ?"#,
                    )
                    .bind(format!("%{}%", target))
                    .bind(limit)
                    .fetch_all(sam_pool)
                    .await
                    .unwrap_or_default()
                } else {
                    return Ok(out);
                }
            }
        }
        ClockwheelSlotKind::Directory => {
            let normalized_base = slot.target.trim().replace('\\', "/");
            let normalized_base = normalized_base.trim_end_matches('/').to_string();
            let normalized_pattern = format!("{}/%", normalized_base);
            let windows_pattern = normalized_pattern.replace('/', "\\\\");
            sqlx::query(
                r#"SELECT ID as song_id,
                          title,
                          artist,
                          album,
                          category,
                          duration,
                          filename,
                          weight,
                          count_played,
                          UNIX_TIMESTAMP(date_played) as song_last_played_unix
                   FROM songlist
                   WHERE (filename LIKE ? OR REPLACE(filename, '\\', '/') LIKE ?)
                   LIMIT ?"#,
            )
            .bind(windows_pattern)
            .bind(normalized_pattern)
            .bind(limit)
            .fetch_all(sam_pool)
            .await?
        }
        ClockwheelSlotKind::Request => {
            // Queue/request handling already happens before rotation selection in
            // runtime flow. Keep request slots as broad pool fallback.
            sqlx::query(
                r#"SELECT ID as song_id,
                          title,
                          artist,
                          album,
                          category,
                          duration,
                          filename,
                          weight,
                          count_played,
                          UNIX_TIMESTAMP(date_played) as song_last_played_unix
                   FROM songlist
                   LIMIT ?"#,
            )
            .bind(limit)
            .fetch_all(sam_pool)
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(|r| CandidateInternal {
            song_id: r.get::<i64, _>("song_id"),
            title: r.try_get::<String, _>("title").unwrap_or_default(),
            artist: r.try_get::<String, _>("artist").unwrap_or_default(),
            album: r.try_get::<String, _>("album").unwrap_or_default(),
            category: r.try_get::<Option<String>, _>("category").ok().flatten(),
            duration: r
                .try_get::<i64, _>("duration")
                .or_else(|_| r.try_get::<i32, _>("duration").map(|v| v as i64))
                .unwrap_or(0),
            file_path: r.try_get::<String, _>("filename").unwrap_or_default(),
            weight: r.try_get::<f64, _>("weight").unwrap_or(1.0),
            count_played: r
                .try_get::<i64, _>("count_played")
                .or_else(|_| r.try_get::<i32, _>("count_played").map(|v| v as i64))
                .unwrap_or(0),
            song_last_played_unix: r
                .try_get::<Option<i64>, _>("song_last_played_unix")
                .ok()
                .flatten()
                .unwrap_or(0),
        })
        .collect())
}

async fn load_history(sam_pool: &MySqlPool) -> Vec<HistoryRow> {
    let rows = sqlx::query(
        r#"SELECT songID,
                  artist,
                  title,
                  album,
                  UNIX_TIMESTAMP(date_played) as played_unix
           FROM historylist
           ORDER BY date_played DESC
           LIMIT 600"#,
    )
    .fetch_all(sam_pool)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| HistoryRow {
            song_id: r
                .try_get::<i64, _>("songID")
                .or_else(|_| r.try_get::<i32, _>("songID").map(|v| v as i64))
                .unwrap_or(0),
            artist: r.try_get::<String, _>("artist").unwrap_or_default(),
            title: r.try_get::<String, _>("title").unwrap_or_default(),
            album: r.try_get::<String, _>("album").unwrap_or_default(),
            played_unix: r
                .try_get::<Option<i64>, _>("played_unix")
                .ok()
                .flatten()
                .unwrap_or(0),
        })
        .collect()
}

fn apply_clockwheel_rules(
    candidates: &mut Vec<CandidateInternal>,
    history: &[HistoryRow],
    rules: &ClockwheelRules,
    now_unix: i64,
) {
    candidates.retain(|c| {
        if rules.no_same_track_minutes > 0 {
            let cutoff = now_unix - (rules.no_same_track_minutes as i64 * 60);
            if history
                .iter()
                .any(|h| h.song_id == c.song_id && h.played_unix >= cutoff)
            {
                return false;
            }
        }

        if rules.no_same_artist_minutes > 0 && !c.artist.trim().is_empty() {
            let cutoff = now_unix - (rules.no_same_artist_minutes as i64 * 60);
            if history.iter().any(|h| {
                !h.artist.is_empty()
                    && h.artist.eq_ignore_ascii_case(&c.artist)
                    && h.played_unix >= cutoff
            }) {
                return false;
            }
        }

        if rules.no_same_album_minutes > 0 && !c.album.trim().is_empty() {
            let cutoff = now_unix - (rules.no_same_album_minutes as i64 * 60);
            if history.iter().any(|h| {
                !h.album.is_empty()
                    && h.album.eq_ignore_ascii_case(&c.album)
                    && h.played_unix >= cutoff
            }) {
                return false;
            }
        }

        if rules.no_same_title_minutes > 0 && !c.title.trim().is_empty() {
            let cutoff = now_unix - (rules.no_same_title_minutes as i64 * 60);
            if history.iter().any(|h| {
                !h.title.is_empty()
                    && h.title.eq_ignore_ascii_case(&c.title)
                    && h.played_unix >= cutoff
            }) {
                return false;
            }
        }

        true
    });
}

fn apply_legacy_rotation_rules(
    candidates: &mut Vec<CandidateInternal>,
    history: &[HistoryRow],
    enabled_rules: &[RotationRuleRow],
    now_unix: i64,
) {
    for rule_row in enabled_rules {
        let rule: Result<RotationRule, _> = serde_json::from_str(&rule_row.config_json);
        let Ok(rule) = rule else { continue };

        candidates.retain(|c| match &rule {
            RotationRule::ArtistSeparation { min_songs } => {
                let recent_artists: Vec<&str> = history
                    .iter()
                    .take(*min_songs as usize)
                    .map(|h| h.artist.as_str())
                    .collect();
                !recent_artists
                    .iter()
                    .any(|a| !a.is_empty() && a.eq_ignore_ascii_case(&c.artist))
            }
            RotationRule::ArtistSeparationTime { min_minutes } => {
                let cutoff = now_unix - (*min_minutes as i64 * 60);
                !history
                    .iter()
                    .any(|h| h.artist.eq_ignore_ascii_case(&c.artist) && h.played_unix > cutoff)
            }
            RotationRule::SongSeparation { min_songs } => !history
                .iter()
                .take(*min_songs as usize)
                .any(|h| h.song_id == c.song_id),
            RotationRule::SongSeparationTime { min_minutes } => {
                let cutoff = now_unix - (*min_minutes as i64 * 60);
                !history
                    .iter()
                    .any(|h| h.song_id == c.song_id && h.played_unix > cutoff)
            }
            RotationRule::AlbumSeparation { min_songs } => {
                if c.album.is_empty() {
                    return true;
                }
                !history
                    .iter()
                    .take(*min_songs as usize)
                    .any(|h| !h.album.is_empty() && h.album.eq_ignore_ascii_case(&c.album))
            }
            RotationRule::MaxPlaysPerHour {
                song_id,
                max,
                window_hours,
            } => {
                if c.song_id != *song_id {
                    return true;
                }
                let cutoff = now_unix - (*window_hours as i64 * 3600);
                let plays = history
                    .iter()
                    .filter(|h| h.song_id == c.song_id && h.played_unix > cutoff)
                    .count() as u32;
                plays < *max
            }
            _ => true,
        });
    }
}

fn choose_candidate(
    mut candidates: Vec<CandidateInternal>,
    method: ClockwheelSelectionMethod,
    history: &[HistoryRow],
    now_unix: i64,
) -> Option<CandidateInternal> {
    if candidates.is_empty() {
        return None;
    }

    let mut song_last: HashMap<i64, i64> = HashMap::new();
    let mut artist_last: HashMap<String, i64> = HashMap::new();
    for h in history {
        song_last.entry(h.song_id).or_insert(h.played_unix);
        if !h.artist.trim().is_empty() {
            artist_last
                .entry(h.artist.to_lowercase())
                .or_insert(h.played_unix);
        }
    }

    let seed = pseudo_random_u64();

    let pick = match method {
        ClockwheelSelectionMethod::Weighted => {
            let total: f64 = candidates
                .iter()
                .map(|c| c.weight.max(0.01))
                .sum::<f64>()
                .max(0.01);
            let mut target = (seed as f64 / u64::MAX as f64) * total;
            let mut chosen = 0usize;
            for (i, c) in candidates.iter().enumerate() {
                let w = c.weight.max(0.01);
                if target <= w {
                    chosen = i;
                    break;
                }
                target -= w;
                chosen = i;
            }
            chosen
        }
        ClockwheelSelectionMethod::Priority => candidates
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.weight
                    .partial_cmp(&b.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.count_played.cmp(&a.count_played))
            })
            .map(|(i, _)| i)
            .unwrap_or(0),
        ClockwheelSelectionMethod::Random => (seed as usize) % candidates.len(),
        ClockwheelSelectionMethod::MostRecentlyPlayedSong => candidates
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| {
                song_last
                    .get(&c.song_id)
                    .copied()
                    .unwrap_or(c.song_last_played_unix)
            })
            .map(|(i, _)| i)
            .unwrap_or(0),
        ClockwheelSelectionMethod::LeastRecentlyPlayedSong => candidates
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| {
                let ts = song_last
                    .get(&c.song_id)
                    .copied()
                    .unwrap_or(c.song_last_played_unix);
                if ts <= 0 {
                    i64::MIN
                } else {
                    ts
                }
            })
            .map(|(i, _)| i)
            .unwrap_or(0),
        ClockwheelSelectionMethod::MostRecentlyPlayedArtist => candidates
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| {
                artist_last
                    .get(&c.artist.to_lowercase())
                    .copied()
                    .unwrap_or(0)
            })
            .map(|(i, _)| i)
            .unwrap_or(0),
        ClockwheelSelectionMethod::LeastRecentlyPlayedArtist => candidates
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| {
                let ts = artist_last
                    .get(&c.artist.to_lowercase())
                    .copied()
                    .unwrap_or(0);
                if ts <= 0 {
                    i64::MIN
                } else {
                    ts
                }
            })
            .map(|(i, _)| i)
            .unwrap_or(0),
        ClockwheelSelectionMethod::Lemming => {
            candidates.sort_by(|a, b| {
                let age_a = now_unix - song_last.get(&a.song_id).copied().unwrap_or(0);
                let age_b = now_unix - song_last.get(&b.song_id).copied().unwrap_or(0);
                age_b.cmp(&age_a).then_with(|| {
                    b.weight
                        .partial_cmp(&a.weight)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
            0
        }
        ClockwheelSelectionMethod::PlaylistOrder => candidates
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| (c.count_played, c.song_id))
            .map(|(i, _)| i)
            .unwrap_or(0),
    };

    Some(candidates.swap_remove(pick))
}

fn normalize_label(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn parse_sam_datetime_unix(value: Option<&str>) -> i64 {
    let Some(raw) = value.map(str::trim).filter(|s| !s.is_empty()) else {
        return 0;
    };
    NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(0)
}

fn pseudo_random_u64() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    (nanos as u64) ^ ((nanos >> 64) as u64)
}
