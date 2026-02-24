use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthSnapshot {
    pub timestamp: i64,
    pub cpu_pct: f32,
    pub memory_mb: f32,
    pub ring_buffer_fill_deck_a: f32,
    pub ring_buffer_fill_deck_b: f32,
    pub decoder_latency_ms: f32,
    pub stream_connected: bool,
    pub mysql_connected: bool,
    pub active_encoders: i32,
}

impl Default for SystemHealthSnapshot {
    fn default() -> Self {
        Self {
            timestamp: 0,
            cpu_pct: 0.0,
            memory_mb: 0.0,
            ring_buffer_fill_deck_a: 1.0,
            ring_buffer_fill_deck_b: 1.0,
            decoder_latency_ms: 0.0,
            stream_connected: false,
            mysql_connected: false,
            active_encoders: 0,
        }
    }
}

pub struct HealthMonitor {
    current: Arc<Mutex<SystemHealthSnapshot>>,
    pool: Option<SqlitePool>,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            current: Arc::new(Mutex::new(SystemHealthSnapshot::default())),
            pool: None,
        }
    }

    pub fn with_pool(mut self, pool: SqlitePool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Start background monitoring task
    pub fn start_monitoring(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));

            loop {
                ticker.tick().await;

                // Collect metrics
                let snapshot = self.collect_snapshot().await;

                // Update current snapshot
                {
                    let mut current = self.current.lock().await;
                    *current = snapshot.clone();
                }

                // Save to database if available
                if let Some(pool) = &self.pool {
                    let _ = self.save_snapshot(pool, &snapshot).await;
                }
            }
        });
    }

    async fn collect_snapshot(&self) -> SystemHealthSnapshot {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // TODO: Collect real metrics from audio engine
        // For now, return mock data
        SystemHealthSnapshot {
            timestamp: now_ms,
            cpu_pct: 5.0,
            memory_mb: 250.0,
            ring_buffer_fill_deck_a: 0.85,
            ring_buffer_fill_deck_b: 0.90,
            decoder_latency_ms: 2.5,
            stream_connected: true,
            mysql_connected: true,
            active_encoders: 2,
        }
    }

    async fn save_snapshot(
        &self,
        pool: &SqlitePool,
        snapshot: &SystemHealthSnapshot,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO system_health_snapshots (
                timestamp, cpu_pct, memory_mb,
                ring_buffer_fill_deck_a, ring_buffer_fill_deck_b,
                decoder_latency_ms, stream_connected, mysql_connected, active_encoders
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(snapshot.timestamp)
        .bind(snapshot.cpu_pct)
        .bind(snapshot.memory_mb)
        .bind(snapshot.ring_buffer_fill_deck_a)
        .bind(snapshot.ring_buffer_fill_deck_b)
        .bind(snapshot.decoder_latency_ms)
        .bind(snapshot.stream_connected as i64)
        .bind(snapshot.mysql_connected as i64)
        .bind(snapshot.active_encoders)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_current_snapshot(&self) -> SystemHealthSnapshot {
        self.current.lock().await.clone()
    }

    pub async fn get_health_history(
        pool: &SqlitePool,
        period_minutes: i64,
    ) -> Result<Vec<SystemHealthSnapshot>, sqlx::Error> {
        let cutoff_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            - (period_minutes * 60 * 1000);

        let rows = sqlx::query_as::<_, (i64, f32, f32, f32, f32, f32, i64, i64, i32)>(
            r#"
            SELECT timestamp, cpu_pct, memory_mb,
                   ring_buffer_fill_deck_a, ring_buffer_fill_deck_b,
                   decoder_latency_ms, stream_connected, mysql_connected, active_encoders
            FROM system_health_snapshots
            WHERE timestamp >= ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(cutoff_ms)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    timestamp,
                    cpu_pct,
                    memory_mb,
                    ring_buffer_fill_deck_a,
                    ring_buffer_fill_deck_b,
                    decoder_latency_ms,
                    stream_connected,
                    mysql_connected,
                    active_encoders,
                )| {
                    SystemHealthSnapshot {
                        timestamp,
                        cpu_pct,
                        memory_mb,
                        ring_buffer_fill_deck_a,
                        ring_buffer_fill_deck_b,
                        decoder_latency_ms,
                        stream_connected: stream_connected != 0,
                        mysql_connected: mysql_connected != 0,
                        active_encoders,
                    }
                },
            )
            .collect())
    }
}

