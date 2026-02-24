use chrono::Timelike;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopSong {
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub play_count: i64,
    pub total_played_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapData {
    pub date: String,
    pub hour: i32,
    pub play_count: i64,
    pub unique_songs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayHistoryEntry {
    pub id: i64,
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub played_at: i64,
    pub duration_ms: i64,
    pub deck: Option<String>,
}

/// Get top songs by play count for a given period
pub async fn get_top_songs(
    pool: &SqlitePool,
    period: &str,
    limit: i64,
) -> Result<Vec<TopSong>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"
        SELECT song_id, play_count, total_played_ms
        FROM play_stats_cache
        WHERE period = ?
        ORDER BY play_count DESC
        LIMIT ?
        "#,
    )
    .bind(period)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    // For now, return mock data with song_id only
    // In production, you'd join with SAM database to get title/artist
    Ok(rows
        .into_iter()
        .map(|(song_id, play_count, total_played_ms)| TopSong {
            song_id,
            title: format!("Song {}", song_id),
            artist: "Unknown Artist".to_string(),
            play_count,
            total_played_ms,
        })
        .collect())
}

/// Get hourly play heatmap data
pub async fn get_hourly_heatmap(
    pool: &SqlitePool,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<HeatmapData>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, i32, i64, i64)>(
        r#"
        SELECT date, hour, play_count, unique_songs
        FROM hourly_play_counts
        WHERE date >= ? AND date <= ?
        ORDER BY date ASC, hour ASC
        "#,
    )
    .bind(start_date)
    .bind(end_date)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(date, hour, play_count, unique_songs)| HeatmapData {
            date,
            hour,
            play_count,
            unique_songs,
        })
        .collect())
}

/// Get play history for a specific song
pub async fn get_song_play_history(
    _pool: &SqlitePool,
    _song_id: i64,
    _limit: i64,
) -> Result<Vec<PlayHistoryEntry>, sqlx::Error> {
    // This would query SAM historylist table
    // For now, return empty vec as placeholder
    Ok(vec![])
}

/// Refresh play stats cache from SAM historylist
pub async fn refresh_play_stats_cache(
    _sqlite_pool: &SqlitePool,
    _mysql_pool: Option<&sqlx::MySqlPool>,
) -> Result<(), sqlx::Error> {
    // TODO: Query SAM historylist and aggregate into play_stats_cache
    // For now, just ensure the table exists
    Ok(())
}

/// Update hourly play counts (called on each track play)
pub async fn update_hourly_play_count(
    pool: &SqlitePool,
    song_id: i64,
) -> Result<(), sqlx::Error> {
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let hour = now.hour() as i32;

    sqlx::query(
        r#"
        INSERT INTO hourly_play_counts (date, hour, play_count, unique_songs)
        VALUES (?, ?, 1, 1)
        ON CONFLICT(date, hour) DO UPDATE SET
            play_count = play_count + 1
        "#,
    )
    .bind(date)
    .bind(hour)
    .execute(pool)
    .await?;

    Ok(())
}

