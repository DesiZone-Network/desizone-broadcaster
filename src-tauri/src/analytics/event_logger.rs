use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventCategory {
    Audio,
    Stream,
    Scheduler,
    Gateway,
    Scripting,
    Database,
    System,
}

impl EventCategory {
    pub fn as_str(&self) -> &str {
        match self {
            EventCategory::Audio => "audio",
            EventCategory::Stream => "stream",
            EventCategory::Scheduler => "scheduler",
            EventCategory::Gateway => "gateway",
            EventCategory::Scripting => "scripting",
            EventCategory::Database => "database",
            EventCategory::System => "system",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub id: i64,
    pub timestamp: i64,
    pub level: String,
    pub category: String,
    pub event: String,
    pub message: String,
    pub metadata_json: Option<String>,
    pub deck: Option<String>,
    pub song_id: Option<i64>,
    pub encoder_id: Option<i64>,
}

/// Log an event to the SQLite event_log table
pub async fn log_event(
    pool: &SqlitePool,
    level: LogLevel,
    category: EventCategory,
    event: &str,
    message: &str,
    metadata: Option<serde_json::Value>,
    deck: Option<&str>,
    song_id: Option<i64>,
    encoder_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let metadata_json = metadata.map(|m| serde_json::to_string(&m).unwrap_or_default());

    sqlx::query(
        r#"
        INSERT INTO event_log (
            timestamp, level, category, event, message, metadata_json, deck, song_id, encoder_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(now_ms)
    .bind(level.as_str())
    .bind(category.as_str())
    .bind(event)
    .bind(message)
    .bind(metadata_json)
    .bind(deck)
    .bind(song_id)
    .bind(encoder_id)
    .execute(pool)
    .await?;

    // Also log to console
    match level {
        LogLevel::Debug => log::debug!("[{}] {}: {}", category.as_str(), event, message),
        LogLevel::Info => log::info!("[{}] {}: {}", category.as_str(), event, message),
        LogLevel::Warn => log::warn!("[{}] {}: {}", category.as_str(), event, message),
        LogLevel::Error => log::error!("[{}] {}: {}", category.as_str(), event, message),
    }

    Ok(())
}

/// Get event log entries with filtering and pagination
pub async fn get_event_log(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
    level: Option<&str>,
    category: Option<&str>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    search: Option<&str>,
) -> Result<(Vec<EventLogEntry>, i64), sqlx::Error> {
    // For simplicity, just return all events for now
    // TODO: Implement proper filtering
    let rows = sqlx::query_as::<_, (i64, i64, String, String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT id, timestamp, level, category, event, message, metadata_json, deck, song_id, encoder_id FROM event_log ORDER BY timestamp DESC LIMIT ? OFFSET ?"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let entries: Vec<EventLogEntry> = rows
        .into_iter()
        .map(|(id, timestamp, level, category, event, message, metadata_json, deck, song_id, encoder_id)| {
            EventLogEntry {
                id,
                timestamp,
                level,
                category,
                event,
                message,
                metadata_json,
                deck,
                song_id,
                encoder_id,
            }
        })
        .collect();

    // Get total count
    let count_query = "SELECT COUNT(*) FROM event_log WHERE 1=1";
    let total: i64 = sqlx::query_scalar(count_query).fetch_one(pool).await?;

    Ok((entries, total))
}

/// Clear old event log entries
pub async fn clear_event_log(pool: &SqlitePool, older_than_days: i64) -> Result<u64, sqlx::Error> {
    let cutoff_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        - (older_than_days * 24 * 60 * 60 * 1000);

    let result = sqlx::query("DELETE FROM event_log WHERE timestamp < ?")
        .bind(cutoff_ms)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

