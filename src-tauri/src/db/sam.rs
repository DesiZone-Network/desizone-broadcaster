use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPool, Row};

/// Connect to a SAM Broadcaster MySQL database.
pub async fn connect(url: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPool::connect(url).await
}

// ── songlist ─────────────────────────────────────────────────────────────────

/// A row from SAM's `songlist` table (read-only; SAM owns this schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamSong {
    pub songid: i32,
    pub artist: String,
    pub title: String,
    pub filename: String,
    /// Duration in seconds
    pub duration: i32,
    pub intro: i32,
    pub outro: i32,
    pub bpm: Option<f64>,
    pub gain: Option<f64>,
}

pub async fn get_song(pool: &MySqlPool, song_id: i32) -> Result<Option<SamSong>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT songid, artist, title, filename, duration, intro, outro, bpm, gain FROM songlist WHERE songid = ?",
    )
    .bind(song_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| SamSong {
        songid: r.get("songid"),
        artist: r.get("artist"),
        title: r.get("title"),
        filename: r.get("filename"),
        duration: r.get("duration"),
        intro: r.get("intro"),
        outro: r.get("outro"),
        bpm: r.get("bpm"),
        gain: r.get("gain"),
    }))
}

pub async fn search_songs(
    pool: &MySqlPool,
    query: &str,
    limit: u32,
) -> Result<Vec<SamSong>, sqlx::Error> {
    let pattern = format!("%{query}%");
    let rows = sqlx::query(
        r#"
        SELECT songid, artist, title, filename, duration, intro, outro, bpm, gain
        FROM songlist
        WHERE artist LIKE ? OR title LIKE ?
        ORDER BY artist, title
        LIMIT ?
        "#,
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SamSong {
            songid: r.get("songid"),
            artist: r.get("artist"),
            title: r.get("title"),
            filename: r.get("filename"),
            duration: r.get("duration"),
            intro: r.get("intro"),
            outro: r.get("outro"),
            bpm: r.get("bpm"),
            gain: r.get("gain"),
        })
        .collect())
}

// ── queuelist ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub queueid: Option<i32>,
    pub songid: i32,
    pub played: i32,
    /// Unix timestamp for when this entry should play
    pub playtime: Option<i64>,
}

pub async fn get_queue(pool: &MySqlPool) -> Result<Vec<QueueEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT queueid, songid, played, playtime FROM queuelist WHERE played = 0 ORDER BY queueid",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| QueueEntry {
            queueid: r.get("queueid"),
            songid: r.get("songid"),
            played: r.get("played"),
            playtime: r.get("playtime"),
        })
        .collect())
}

pub async fn add_to_queue(pool: &MySqlPool, song_id: i32) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("INSERT INTO queuelist (songid, played) VALUES (?, 0)")
        .bind(song_id)
        .execute(pool)
        .await?;
    Ok(result.last_insert_id())
}

pub async fn mark_played(pool: &MySqlPool, queue_id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE queuelist SET played = 1 WHERE queueid = ?")
        .bind(queue_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_from_queue(pool: &MySqlPool, queue_id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM queuelist WHERE queueid = ?")
        .bind(queue_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── historylist ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub histid: Option<i32>,
    pub songid: i32,
    /// Unix timestamp
    pub played_at: i64,
}

pub async fn get_history(
    pool: &MySqlPool,
    limit: u32,
) -> Result<Vec<HistoryEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT histid, songid, played_at FROM historylist ORDER BY played_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| HistoryEntry {
            histid: r.get("histid"),
            songid: r.get("songid"),
            played_at: r.get("played_at"),
        })
        .collect())
}

pub async fn add_to_history(pool: &MySqlPool, song_id: i32, played_at: i64) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO historylist (songid, played_at) VALUES (?, ?)")
        .bind(song_id)
        .bind(played_at)
        .execute(pool)
        .await?;
    Ok(())
}
