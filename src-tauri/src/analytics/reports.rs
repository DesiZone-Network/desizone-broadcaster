use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, Row, SqlitePool};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ReportType {
    DailyBroadcast {
        date: String,
    },
    SongPlayHistory {
        song_id: i64,
        days: i32,
    },
    ListenerTrend {
        period_days: i32,
    },
    RequestLog {
        start_date: String,
        end_date: String,
    },
    StreamUptime {
        period_days: i32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    pub report_type: String,
    pub generated_at: i64,
    pub title: String,
    pub summary: ReportSummary,
    pub sections: Vec<ReportSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_plays: Option<i64>,
    pub total_listeners: Option<i64>,
    pub top_song: Option<String>,
    pub peak_hour: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSection {
    pub title: String,
    pub data: serde_json::Value,
}

/// Generate report data based on type
pub async fn generate_report(
    pool: &SqlitePool,
    sam_pool: Option<&MySqlPool>,
    report_type: ReportType,
) -> Result<ReportData, sqlx::Error> {
    let now_ms = Utc::now().timestamp_millis();

    match report_type {
        ReportType::DailyBroadcast { date } => {
            generate_daily_broadcast_report(pool, sam_pool, now_ms, &date).await
        }
        ReportType::SongPlayHistory { song_id, days } => {
            generate_song_play_history_report(pool, sam_pool, now_ms, song_id, days).await
        }
        ReportType::ListenerTrend { period_days } => {
            generate_listener_trend_report(pool, now_ms, period_days).await
        }
        ReportType::RequestLog {
            start_date,
            end_date,
        } => generate_request_log_report(pool, now_ms, &start_date, &end_date).await,
        ReportType::StreamUptime { period_days } => {
            generate_stream_uptime_report(pool, now_ms, period_days).await
        }
    }
}

async fn generate_daily_broadcast_report(
    pool: &SqlitePool,
    sam_pool: Option<&MySqlPool>,
    now_ms: i64,
    date: &str,
) -> Result<ReportData, sqlx::Error> {
    let total_plays: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(play_count), 0) FROM hourly_play_counts WHERE date = ?",
    )
    .bind(date)
    .fetch_one(pool)
    .await?;

    let peak_row = sqlx::query("SELECT hour, play_count FROM hourly_play_counts WHERE date = ? ORDER BY play_count DESC, hour ASC LIMIT 1")
        .bind(date)
        .fetch_optional(pool)
        .await?;

    let peak_hour = peak_row.map(|row| {
        let hour: i32 = row.get("hour");
        format!("{hour:02}:00")
    });

    let top_song_row = sqlx::query("SELECT song_id, play_count FROM play_stats_cache WHERE period = ? ORDER BY play_count DESC, total_played_ms DESC LIMIT 1")
        .bind(format!("day:{date}"))
        .fetch_optional(pool)
        .await?;

    let top_song = if let Some(row) = top_song_row {
        let song_id: i64 = row.get("song_id");
        let play_count: i64 = row.get("play_count");
        Some(resolve_song_label(sam_pool, song_id, play_count).await)
    } else {
        None
    };

    let hourly_rows = sqlx::query_as::<_, (i32, i64, i64)>(
        "SELECT hour, play_count, unique_songs FROM hourly_play_counts WHERE date = ? ORDER BY hour ASC",
    )
    .bind(date)
    .fetch_all(pool)
    .await?;

    Ok(ReportData {
        report_type: "daily_broadcast".to_string(),
        generated_at: now_ms,
        title: format!("Daily Broadcast Report - {date}"),
        summary: ReportSummary {
            total_plays: Some(total_plays),
            total_listeners: None,
            top_song,
            peak_hour,
        },
        sections: vec![ReportSection {
            title: "Hourly Breakdown".to_string(),
            data: serde_json::json!({
                "date": date,
                "hours": hourly_rows
                    .iter()
                    .map(|(hour, play_count, unique_songs)| serde_json::json!({
                        "hour": hour,
                        "play_count": play_count,
                        "unique_songs": unique_songs,
                    }))
                    .collect::<Vec<_>>()
            }),
        }],
    })
}

async fn generate_song_play_history_report(
    pool: &SqlitePool,
    sam_pool: Option<&MySqlPool>,
    now_ms: i64,
    song_id: i64,
    days: i32,
) -> Result<ReportData, sqlx::Error> {
    let period = format!("{}d", days.max(1));
    let cache_row = sqlx::query("SELECT play_count, total_played_ms, last_played_at, skip_count FROM play_stats_cache WHERE song_id = ? AND period = ?")
        .bind(song_id)
        .bind(&period)
        .fetch_optional(pool)
        .await?;

    let (play_count, total_played_ms, last_played_at, skip_count) = if let Some(row) = cache_row {
        (
            row.get::<i64, _>("play_count"),
            row.get::<i64, _>("total_played_ms"),
            row.get::<Option<i64>, _>("last_played_at"),
            row.get::<i64, _>("skip_count"),
        )
    } else {
        (0, 0, None, 0)
    };

    Ok(ReportData {
        report_type: "song_play_history".to_string(),
        generated_at: now_ms,
        title: format!("Song Play History - Song #{song_id}"),
        summary: ReportSummary {
            total_plays: Some(play_count),
            total_listeners: None,
            top_song: Some(resolve_song_label(sam_pool, song_id, play_count).await),
            peak_hour: None,
        },
        sections: vec![ReportSection {
            title: "Song Window Stats".to_string(),
            data: serde_json::json!({
                "song_id": song_id,
                "days": days,
                "period_key": period,
                "total_played_ms": total_played_ms,
                "last_played_at": last_played_at,
                "skip_count": skip_count,
            }),
        }],
    })
}

async fn generate_listener_trend_report(
    pool: &SqlitePool,
    now_ms: i64,
    period_days: i32,
) -> Result<ReportData, sqlx::Error> {
    let cutoff_s = now_ms / 1000 - (period_days.max(1) as i64 * 24 * 60 * 60);

    let summary_row = sqlx::query(
        "SELECT COALESCE(MAX(current_listeners), 0) AS peak, COALESCE(AVG(current_listeners), 0.0) AS avg_count FROM listener_snapshots WHERE snapshot_at >= ?",
    )
    .bind(cutoff_s)
    .fetch_one(pool)
    .await?;

    let peak: i64 = summary_row.get("peak");
    let average: f64 = summary_row.get("avg_count");

    let trend_rows = sqlx::query_as::<_, (i64, i64)>(
        "SELECT snapshot_at, current_listeners FROM listener_snapshots WHERE snapshot_at >= ? ORDER BY snapshot_at ASC LIMIT 500",
    )
    .bind(cutoff_s)
    .fetch_all(pool)
    .await?;

    Ok(ReportData {
        report_type: "listener_trend".to_string(),
        generated_at: now_ms,
        title: format!("Listener Trend - Last {} Days", period_days.max(1)),
        summary: ReportSummary {
            total_plays: None,
            total_listeners: Some(peak),
            top_song: None,
            peak_hour: Some(format!("avg:{average:.2}")),
        },
        sections: vec![ReportSection {
            title: "Trend".to_string(),
            data: serde_json::json!({
                "period_days": period_days,
                "average_listeners": average,
                "samples": trend_rows
                    .iter()
                    .map(|(timestamp, listener_count)| serde_json::json!({
                        "timestamp": timestamp,
                        "listener_count": listener_count,
                    }))
                    .collect::<Vec<_>>()
            }),
        }],
    })
}

async fn generate_request_log_report(
    pool: &SqlitePool,
    now_ms: i64,
    start_date: &str,
    end_date: &str,
) -> Result<ReportData, sqlx::Error> {
    let (start_s, end_s) = parse_date_window_seconds(start_date, end_date);

    let total_requests: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM request_log WHERE requested_at >= ? AND requested_at <= ?",
    )
    .bind(start_s)
    .bind(end_s)
    .fetch_one(pool)
    .await?;

    let fulfilled: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM request_log WHERE requested_at >= ? AND requested_at <= ? AND status = 'played'",
    )
    .bind(start_s)
    .bind(end_s)
    .fetch_one(pool)
    .await?;

    let top_request_row = sqlx::query(
        "SELECT song_title, artist, COUNT(*) as cnt FROM request_log WHERE requested_at >= ? AND requested_at <= ? GROUP BY song_id, song_title, artist ORDER BY cnt DESC LIMIT 1",
    )
    .bind(start_s)
    .bind(end_s)
    .fetch_optional(pool)
    .await?;

    let top_song = top_request_row.map(|row| {
        let title: Option<String> = row.get("song_title");
        let artist: Option<String> = row.get("artist");
        let count: i64 = row.get("cnt");
        format!(
            "{} - {} ({} requests)",
            artist.unwrap_or_else(|| "Unknown Artist".to_string()),
            title.unwrap_or_else(|| "Unknown Title".to_string()),
            count
        )
    });

    Ok(ReportData {
        report_type: "request_log".to_string(),
        generated_at: now_ms,
        title: format!("Request Log - {start_date} to {end_date}"),
        summary: ReportSummary {
            total_plays: Some(total_requests),
            total_listeners: Some(fulfilled),
            top_song,
            peak_hour: None,
        },
        sections: vec![ReportSection {
            title: "Request Summary".to_string(),
            data: serde_json::json!({
                "start_date": start_date,
                "end_date": end_date,
                "total_requests": total_requests,
                "fulfilled_requests": fulfilled,
            }),
        }],
    })
}

async fn generate_stream_uptime_report(
    pool: &SqlitePool,
    now_ms: i64,
    period_days: i32,
) -> Result<ReportData, sqlx::Error> {
    let cutoff_ms = now_ms - (period_days.max(1) as i64 * 24 * 60 * 60 * 1000);

    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM system_health_snapshots WHERE timestamp >= ?")
            .bind(cutoff_ms)
            .fetch_one(pool)
            .await?;

    let connected: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM system_health_snapshots WHERE timestamp >= ? AND stream_connected = 1",
    )
    .bind(cutoff_ms)
    .fetch_one(pool)
    .await?;

    let pct = if total == 0 {
        0.0
    } else {
        (connected as f64 / total as f64) * 100.0
    };

    Ok(ReportData {
        report_type: "stream_uptime".to_string(),
        generated_at: now_ms,
        title: format!("Stream Uptime - Last {} Days", period_days.max(1)),
        summary: ReportSummary {
            total_plays: Some(total),
            total_listeners: Some(connected),
            top_song: None,
            peak_hour: Some(format!("{pct:.2}%")),
        },
        sections: vec![ReportSection {
            title: "Uptime".to_string(),
            data: serde_json::json!({
                "period_days": period_days,
                "sample_count": total,
                "connected_samples": connected,
                "uptime_pct": pct,
            }),
        }],
    })
}

async fn resolve_song_label(sam_pool: Option<&MySqlPool>, song_id: i64, play_count: i64) -> String {
    if let Some(pool) = sam_pool {
        if let Ok(Some(song)) = crate::db::sam::get_song(pool, song_id).await {
            let artist = song.artist.trim();
            let title = song.title.trim();
            if !artist.is_empty() || !title.is_empty() {
                return format!(
                    "{} - {} ({} plays)",
                    if artist.is_empty() {
                        "Unknown Artist"
                    } else {
                        artist
                    },
                    if title.is_empty() {
                        "Unknown Title"
                    } else {
                        title
                    },
                    play_count,
                );
            }
        }
    }

    format!("Song #{} ({} plays)", song_id, play_count)
}

/// Export report data to CSV format
pub fn export_report_csv(report_data: &ReportData) -> Result<String, String> {
    let sanitized_type = report_data
        .report_type
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();

    let file_name = format!(
        "desizone_report_{}_{}.csv",
        sanitized_type, report_data.generated_at
    );
    let path: PathBuf = std::env::temp_dir().join(file_name);

    let mut csv_content = String::from("field,value\n");
    csv_content.push_str(&format!(
        "report_type,{}\n",
        csv_escape(&report_data.report_type)
    ));
    csv_content.push_str(&format!("generated_at,{}\n", report_data.generated_at));
    csv_content.push_str(&format!("title,{}\n", csv_escape(&report_data.title)));
    csv_content.push_str(&format!(
        "summary_total_plays,{}\n",
        report_data
            .summary
            .total_plays
            .map(|v| v.to_string())
            .unwrap_or_default()
    ));
    csv_content.push_str(&format!(
        "summary_total_listeners,{}\n",
        report_data
            .summary
            .total_listeners
            .map(|v| v.to_string())
            .unwrap_or_default()
    ));
    csv_content.push_str(&format!(
        "summary_top_song,{}\n",
        csv_escape(report_data.summary.top_song.as_deref().unwrap_or(""))
    ));
    csv_content.push_str(&format!(
        "summary_peak_hour,{}\n",
        csv_escape(report_data.summary.peak_hour.as_deref().unwrap_or(""))
    ));

    for (index, section) in report_data.sections.iter().enumerate() {
        csv_content.push_str(&format!(
            "section_{}_title,{}\n",
            index,
            csv_escape(&section.title)
        ));
        csv_content.push_str(&format!(
            "section_{}_data,{}\n",
            index,
            csv_escape(&section.data.to_string())
        ));
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(csv_content.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

fn parse_date_window_seconds(start_date: &str, end_date: &str) -> (i64, i64) {
    let fallback = Utc::now().date_naive();
    let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d").unwrap_or(fallback);
    let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d").unwrap_or(start);

    let start_s = start
        .and_hms_opt(0, 0, 0)
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(0);
    let end_s = end
        .and_hms_opt(23, 59, 59)
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(i64::MAX);

    if end_s < start_s {
        (end_s, start_s)
    } else {
        (start_s, end_s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_csv_file() {
        let report = ReportData {
            report_type: "daily_broadcast".to_string(),
            generated_at: Utc::now().timestamp_millis(),
            title: "Daily Broadcast Report".to_string(),
            summary: ReportSummary {
                total_plays: Some(10),
                total_listeners: Some(20),
                top_song: Some("Song 1".to_string()),
                peak_hour: Some("10:00".to_string()),
            },
            sections: vec![],
        };
        let path = export_report_csv(&report).expect("csv export should succeed");
        let content = std::fs::read_to_string(&path).expect("csv file should be readable");

        assert!(content.contains("field,value"));
        assert!(content.contains("report_type"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parses_date_window() {
        let (start_s, end_s) = parse_date_window_seconds("2026-02-01", "2026-02-02");
        assert!(end_s > start_s);
    }
}
