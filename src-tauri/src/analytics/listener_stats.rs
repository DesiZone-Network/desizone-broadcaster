use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerSnapshot {
    pub timestamp: i64,
    pub listener_count: i32,
    pub peak_listeners: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerPeak {
    pub peak: i32,
    pub average: f32,
    pub timestamp: i64,
}

/// Get listener graph data for an encoder
pub async fn get_listener_graph(
    pool: &SqlitePool,
    encoder_id: i64,
    period: &str,
) -> Result<Vec<ListenerSnapshot>, sqlx::Error> {
    let minutes = match period {
        "1h" => 60,
        "24h" => 24 * 60,
        "7d" => 7 * 24 * 60,
        _ => 60,
    };

    let cutoff_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        - (minutes * 60 * 1000);

    let rows = sqlx::query_as::<_, (i64, i32, Option<i32>)>(
        r#"
        SELECT timestamp, listener_count, peak_listeners
        FROM listener_snapshots
        WHERE encoder_id = ? AND timestamp >= ?
        ORDER BY timestamp ASC
        "#,
    )
    .bind(encoder_id)
    .bind(cutoff_ms)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(timestamp, listener_count, peak_listeners)| ListenerSnapshot {
            timestamp,
            listener_count,
            peak_listeners,
        })
        .collect())
}

/// Get listener peak stats for a period
pub async fn get_listener_peak(
    pool: &SqlitePool,
    encoder_id: i64,
    period: &str,
) -> Result<ListenerPeak, sqlx::Error> {
    let minutes = match period {
        "1h" => 60,
        "24h" => 24 * 60,
        "7d" => 7 * 24 * 60,
        _ => 60,
    };

    let cutoff_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
        - (minutes * 60 * 1000);

    let row = sqlx::query_as::<_, (i32, f64, i64)>(
        r#"
        SELECT
            MAX(listener_count) as peak,
            AVG(listener_count) as average,
            MAX(timestamp) as timestamp
        FROM listener_snapshots
        WHERE encoder_id = ? AND timestamp >= ?
        "#,
    )
    .bind(encoder_id)
    .bind(cutoff_ms)
    .fetch_optional(pool)
    .await?;

    match row {
        Some((peak, average, timestamp)) => Ok(ListenerPeak {
            peak,
            average: average as f32,
            timestamp,
        }),
        None => Ok(ListenerPeak {
            peak: 0,
            average: 0.0,
            timestamp: 0,
        }),
    }
}

/// Record a listener snapshot (called from encoder polling task)
pub async fn record_listener_snapshot(
    pool: &SqlitePool,
    encoder_id: i64,
    listener_count: i32,
) -> Result<(), sqlx::Error> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    sqlx::query(
        r#"
        INSERT INTO listener_snapshots (encoder_id, timestamp, listener_count, peak_listeners)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(encoder_id)
    .bind(now_ms)
    .bind(listener_count)
    .bind(listener_count) // Use same value for peak initially
    .execute(pool)
    .await?;

    Ok(())
}

