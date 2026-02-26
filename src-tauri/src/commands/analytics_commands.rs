use serde::{Deserialize, Serialize};
use tauri::State;

use crate::analytics::{
    event_logger::{self, EventLogEntry},
    health_monitor::{HealthMonitor, SystemHealthSnapshot},
    listener_stats::{self, ListenerPeak, ListenerSnapshot},
    play_stats::{self, HeatmapData, PlayHistoryEntry, TopSong},
    reports::{self, ReportData, ReportType},
};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogResponse {
    pub events: Vec<EventLogEntry>,
    pub total: i64,
}

// ── Play Stats ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_top_songs(
    period: String,
    limit: i64,
    state: State<'_, AppState>,
) -> Result<Vec<TopSong>, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    play_stats::get_top_songs(pool, &period, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_hourly_heatmap(
    start_date: String,
    end_date: String,
    state: State<'_, AppState>,
) -> Result<Vec<HeatmapData>, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    play_stats::get_hourly_heatmap(pool, &start_date, &end_date)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_song_play_history(
    song_id: i64,
    limit: i64,
    state: State<'_, AppState>,
) -> Result<Vec<PlayHistoryEntry>, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    play_stats::get_song_play_history(pool, song_id, limit)
        .await
        .map_err(|e| e.to_string())
}

// ── Listener Stats ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_listener_graph(
    encoder_id: i64,
    period: String,
    state: State<'_, AppState>,
) -> Result<Vec<ListenerSnapshot>, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    listener_stats::get_listener_graph(pool, encoder_id, &period)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_listener_peak(
    encoder_id: i64,
    period: String,
    state: State<'_, AppState>,
) -> Result<ListenerPeak, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    listener_stats::get_listener_peak(pool, encoder_id, &period)
        .await
        .map_err(|e| e.to_string())
}

// ── Event Log ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_event_log(
    limit: i64,
    offset: i64,
    level: Option<String>,
    category: Option<String>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    search: Option<String>,
    deck: Option<String>,
    state: State<'_, AppState>,
) -> Result<EventLogResponse, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    let (events, total) = event_logger::get_event_log(
        pool,
        limit,
        offset,
        level.as_deref(),
        category.as_deref(),
        start_time,
        end_time,
        search.as_deref(),
        deck.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(EventLogResponse { events, total })
}

#[tauri::command]
pub async fn clear_event_log(
    older_than_days: i64,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    event_logger::clear_event_log(pool, older_than_days)
        .await
        .map_err(|e| e.to_string())
}

// ── System Health ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_health_snapshot(
    state: State<'_, AppState>,
) -> Result<SystemHealthSnapshot, String> {
    Ok(state.health_monitor.get_current_snapshot().await)
}

#[tauri::command]
pub async fn get_health_history(
    period_minutes: i64,
    state: State<'_, AppState>,
) -> Result<Vec<SystemHealthSnapshot>, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    HealthMonitor::get_health_history(pool, period_minutes)
        .await
        .map_err(|e| e.to_string())
}

// ── Reports ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_report(
    report_type: ReportType,
    state: State<'_, AppState>,
) -> Result<ReportData, String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    let sam_pool = {
        let guard = state.sam_db.read().await;
        guard.clone()
    };

    reports::generate_report(pool, sam_pool.as_ref(), report_type)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_report_csv(report_data: ReportData) -> Result<String, String> {
    reports::export_report_csv(&report_data)
}

#[tauri::command]
pub async fn write_event_log(
    level: String,
    category: String,
    event: String,
    message: String,
    deck: Option<String>,
    song_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pool = state
        .local_db
        .as_ref()
        .ok_or("Local database not available")?;

    let log_level = match level.as_str() {
        "error" => event_logger::LogLevel::Error,
        "warn" => event_logger::LogLevel::Warn,
        "info" => event_logger::LogLevel::Info,
        _ => event_logger::LogLevel::Debug,
    };
    let log_category = match category.as_str() {
        "audio" => event_logger::EventCategory::Audio,
        "stream" => event_logger::EventCategory::Stream,
        "scheduler" => event_logger::EventCategory::Scheduler,
        "gateway" => event_logger::EventCategory::Gateway,
        "scripting" => event_logger::EventCategory::Scripting,
        "database" => event_logger::EventCategory::Database,
        _ => event_logger::EventCategory::System,
    };

    event_logger::log_event(
        pool,
        log_level,
        log_category,
        &event,
        &message,
        None,
        deck.as_deref(),
        song_id,
        None,
    )
    .await
    .map_err(|e| e.to_string())
}
