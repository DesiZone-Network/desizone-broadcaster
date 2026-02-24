/// Playlist Rotation Engine
///
/// Selects the next track for AutoDJ based on active rotation rules.
/// Rules are evaluated against the recent play history to avoid repetition.
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::mysql::MySqlPool;
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
        // Deactivate others if this one is active
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

/// Select the next track for AutoDJ from the SAM `songlist` table,
/// applying all enabled rotation rules from our local DB.
///
/// Returns None if no eligible tracks are found.
pub async fn select_next_track(
    local_pool: &SqlitePool,
    sam_pool: &MySqlPool,
    active_category: Option<&str>,
) -> Result<Option<SongCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    // 1. Load enabled rules
    let rules = get_rotation_rules(local_pool).await?;
    let enabled_rules: Vec<RotationRuleRow> = rules.into_iter().filter(|r| r.enabled).collect();

    // 2. Load recent history (last 60 songs) from SAM historylist
    let history: Vec<(i64, String, String, i64)> = sqlx::query(
        "SELECT songid, artist, album, played_at_unix FROM historylist ORDER BY played_at_unix DESC LIMIT 60"
    )
    .fetch_all(sam_pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (
        r.get::<i64, _>(0),
        r.get::<String, _>(1),
        r.get::<Option<String>, _>(2).unwrap_or_default(),
        r.get::<i64, _>(3),
    ))
    .collect();

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // 3. Fetch candidates from SAM songlist
    let category_filter = active_category.unwrap_or("%");
    let candidates_raw = sqlx::query(
        "SELECT songid, title, artist, album, category, duration, filename \
         FROM songlist WHERE enabled = 1 AND (? = '%' OR category LIKE ?) \
         ORDER BY RANDOM() LIMIT 200"
    )
    .bind(category_filter)
    .bind(format!("%{category_filter}%"))
    .fetch_all(sam_pool)
    .await
    .unwrap_or_default();

    let mut candidates: Vec<SongCandidate> = candidates_raw
        .into_iter()
        .map(|r| SongCandidate {
            song_id: r.get::<i64, _>(0),
            title: r.get::<String, _>(1),
            artist: r.get::<String, _>(2),
            album: r.get::<Option<String>, _>(3),
            category: r.get::<Option<String>, _>(4),
            duration: r.get::<i64, _>(5),
            file_path: r.get::<String, _>(6),
            score: 1.0,
        })
        .collect();

    // 4. Apply rotation rules to filter / penalise candidates
    for rule_row in &enabled_rules {
        let rule: Result<RotationRule, _> = serde_json::from_str(&rule_row.config_json);
        let Ok(rule) = rule else { continue };

        candidates.retain_mut(|c| {
            match &rule {
                RotationRule::ArtistSeparation { min_songs } => {
                    let recent_artists: Vec<&str> = history.iter()
                        .take(*min_songs as usize)
                        .map(|(_, a, _, _)| a.as_str())
                        .collect();
                    !recent_artists.contains(&c.artist.as_str())
                }
                RotationRule::ArtistSeparationTime { min_minutes } => {
                    let cutoff = now_unix - (*min_minutes as i64 * 60);
                    !history.iter().any(|(_, a, _, t)| {
                        a == &c.artist && *t > cutoff
                    })
                }
                RotationRule::SongSeparation { min_songs } => {
                    !history.iter().take(*min_songs as usize).any(|(id, _, _, _)| *id == c.song_id)
                }
                RotationRule::SongSeparationTime { min_minutes } => {
                    let cutoff = now_unix - (*min_minutes as i64 * 60);
                    !history.iter().any(|(id, _, _, t)| *id == c.song_id && *t > cutoff)
                }
                RotationRule::AlbumSeparation { min_songs } => {
                    let album = c.album.as_deref().unwrap_or("");
                    if album.is_empty() { return true; }
                    !history.iter().take(*min_songs as usize).any(|(_, _, alb, _)| alb == album)
                }
                RotationRule::MaxPlaysPerHour { song_id, max, window_hours } => {
                    if c.song_id != *song_id { return true; }
                    let cutoff = now_unix - (*window_hours as i64 * 3600);
                    let plays = history.iter()
                        .filter(|(id, _, _, t)| *id == c.song_id && *t > cutoff)
                        .count() as u32;
                    plays < *max
                }
                _ => true,
            }
        });
    }

    // 5. Score remaining candidates (prefer less recently played)
    for c in &mut candidates {
        let last_played = history.iter()
            .find(|(id, _, _, _)| *id == c.song_id)
            .map(|(_, _, _, t)| *t)
            .unwrap_or(0);
        let age_hours = (now_unix - last_played) as f64 / 3600.0;
        c.score = (age_hours / 24.0).min(2.0); // cap at 2x bonus for 24h+ unplayed
    }

    // 6. Sort by score descending, pick best
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(candidates.into_iter().next())
}
