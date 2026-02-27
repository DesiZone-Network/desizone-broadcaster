use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

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
    deck: Option<&str>,
) -> Result<(Vec<EventLogEntry>, i64), sqlx::Error> {
    let mut query_builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, timestamp, level, category, event, message, metadata_json, deck, song_id, encoder_id FROM event_log WHERE 1=1",
    );

    append_filters(
        &mut query_builder,
        level,
        category,
        start_time,
        end_time,
        search,
        deck,
    );

    query_builder.push(" ORDER BY timestamp DESC LIMIT ");
    query_builder.push_bind(limit.max(1));
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset.max(0));

    let rows = query_builder
        .build_query_as::<(
            i64,
            i64,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
        )>()
        .fetch_all(pool)
        .await?;

    let entries: Vec<EventLogEntry> = rows
        .into_iter()
        .map(
            |(
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
            )| {
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
            },
        )
        .collect();

    let mut count_query_builder =
        QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM event_log WHERE 1=1");
    append_filters(
        &mut count_query_builder,
        level,
        category,
        start_time,
        end_time,
        search,
        deck,
    );
    let total: i64 = count_query_builder
        .build_query_scalar()
        .fetch_one(pool)
        .await?;

    Ok((entries, total))
}

fn append_filters(
    query_builder: &mut QueryBuilder<'_, Sqlite>,
    level: Option<&str>,
    category: Option<&str>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    search: Option<&str>,
    deck: Option<&str>,
) {
    if let Some(level) = level.filter(|value| !value.trim().is_empty()) {
        query_builder.push(" AND level = ");
        query_builder.push_bind(level.trim().to_string());
    }

    if let Some(category) = category.filter(|value| !value.trim().is_empty()) {
        query_builder.push(" AND category = ");
        query_builder.push_bind(category.trim().to_string());
    }

    if let Some(start_time) = start_time {
        query_builder.push(" AND timestamp >= ");
        query_builder.push_bind(start_time);
    }

    if let Some(end_time) = end_time {
        query_builder.push(" AND timestamp <= ");
        query_builder.push_bind(end_time);
    }

    if let Some(deck) = deck.filter(|value| !value.trim().is_empty()) {
        query_builder.push(" AND deck = ");
        query_builder.push_bind(deck.trim().to_string());
    }

    if let Some(search) = search.filter(|value| !value.trim().is_empty()) {
        let pattern = format!("%{}%", search.trim().to_lowercase());
        query_builder.push(" AND (LOWER(event) LIKE ");
        query_builder.push_bind(pattern.clone());
        query_builder.push(" OR LOWER(message) LIKE ");
        query_builder.push_bind(pattern.clone());
        query_builder.push(" OR LOWER(COALESCE(metadata_json, '')) LIKE ");
        query_builder.push_bind(pattern);
        query_builder.push(")");
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE event_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                level TEXT NOT NULL,
                category TEXT NOT NULL,
                event TEXT NOT NULL,
                message TEXT NOT NULL,
                metadata_json TEXT,
                deck TEXT,
                song_id INTEGER,
                encoder_id INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create event_log table");

        pool
    }

    #[tokio::test]
    async fn get_event_log_applies_filters_and_count() {
        let pool = setup_pool().await;

        sqlx::query("INSERT INTO event_log (timestamp, level, category, event, message, metadata_json, deck) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(1_700_000_000_000_i64)
            .bind("info")
            .bind("stream")
            .bind("encoder_connected")
            .bind("Connected")
            .bind("{\"source\":\"icecast\"}")
            .bind("deck_a")
            .execute(&pool)
            .await
            .expect("insert row 1");

        sqlx::query("INSERT INTO event_log (timestamp, level, category, event, message, metadata_json, deck) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(1_700_000_100_000_i64)
            .bind("error")
            .bind("audio")
            .bind("buffer_underrun")
            .bind("Underrun detected")
            .bind("{\"severity\":\"high\"}")
            .bind("deck_b")
            .execute(&pool)
            .await
            .expect("insert row 2");

        let (rows, total) = get_event_log(
            &pool,
            20,
            0,
            Some("info"),
            Some("stream"),
            Some(1_699_999_999_000),
            Some(1_700_000_050_000),
            Some("icecast"),
            Some("deck_a"),
        )
        .await
        .expect("filtered event log");

        assert_eq!(total, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event, "encoder_connected");
        assert_eq!(rows[0].deck.as_deref(), Some("deck_a"));
    }
}
