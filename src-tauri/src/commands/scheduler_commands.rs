/// Phase 3 — Automation & Scheduling commands
use tauri::State;
use crate::state::AppState;
use crate::scheduler::{
    rotation::{self, RotationRuleRow, Playlist},
    show_scheduler::{self, Show, ScheduledEvent},
    request_policy::{self, RequestPolicy, RequestLogEntry, RequestStatus},
    autodj::{self, DjMode, GapKillerConfig},
};

// ── DJ Mode ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_dj_mode() -> String {
    autodj::get_dj_mode().as_str().to_string()
}

#[tauri::command]
pub async fn set_dj_mode(mode: String) {
    autodj::set_dj_mode(DjMode::from_str(&mode));
}

// ── Rotation Rules ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_rotation_rules(
    state: State<'_, AppState>,
) -> Result<Vec<RotationRuleRow>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::get_rotation_rules(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_rotation_rule(
    state: State<'_, AppState>,
    rule: RotationRuleRow,
) -> Result<i64, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::upsert_rotation_rule(pool, &rule)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_rotation_rule(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::delete_rotation_rule(pool, id)
        .await
        .map_err(|e| e.to_string())
}

// ── Playlists ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::get_playlists(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_playlist(
    state: State<'_, AppState>,
    playlist: Playlist,
) -> Result<i64, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::upsert_playlist(pool, &playlist)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_active_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::set_active_playlist(pool, playlist_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_next_autodj_track(
    state: State<'_, AppState>,
) -> Result<Option<rotation::SongCandidate>, String> {
    let local_pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let sam_pool = state.sam_db.as_ref().ok_or("SAM DB not connected")?;
    rotation::select_next_track(local_pool, sam_pool, None)
        .await
        .map_err(|e| e.to_string())
}

// ── Show Scheduler ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_shows(state: State<'_, AppState>) -> Result<Vec<Show>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    show_scheduler::get_shows(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_show(state: State<'_, AppState>, show: Show) -> Result<i64, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    show_scheduler::upsert_show(pool, &show)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_show(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    show_scheduler::delete_show(pool, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_upcoming_events(
    state: State<'_, AppState>,
    hours: u32,
) -> Result<Vec<ScheduledEvent>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    show_scheduler::get_upcoming_events(pool, hours)
        .await
        .map_err(|e| e.to_string())
}

// ── GAP Killer ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_gap_killer_config(
    state: State<'_, AppState>,
) -> Result<GapKillerConfig, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let row: Option<String> = sqlx::query_scalar(
        "SELECT gap_killer_json FROM gap_killer_config WHERE id = 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn set_gap_killer_config(
    state: State<'_, AppState>,
    config: GapKillerConfig,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO gap_killer_config (id, gap_killer_json) VALUES (1, ?) \
         ON CONFLICT(id) DO UPDATE SET gap_killer_json = excluded.gap_killer_json"
    )
    .bind(&json)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Request Policy ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_request_policy(
    state: State<'_, AppState>,
) -> Result<RequestPolicy, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    request_policy::load_policy(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_request_policy(
    state: State<'_, AppState>,
    policy: RequestPolicy,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    request_policy::save_policy(pool, &policy)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pending_requests(
    state: State<'_, AppState>,
) -> Result<Vec<RequestLogEntry>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    request_policy::get_requests(pool, "pending")
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn accept_request_p3(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    request_policy::update_request_status(pool, id, RequestStatus::Accepted, None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reject_request_p3(
    state: State<'_, AppState>,
    id: i64,
    reason: Option<String>,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    request_policy::update_request_status(pool, id, RequestStatus::Rejected, reason.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_request_history(
    state: State<'_, AppState>,
    limit: i64,
    offset: i64,
) -> Result<Vec<RequestLogEntry>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    request_policy::get_request_history(pool, limit, offset)
        .await
        .map_err(|e| e.to_string())
}

use sqlx;
