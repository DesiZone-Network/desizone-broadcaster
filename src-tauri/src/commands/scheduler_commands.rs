use crate::scheduler::{
    autodj::{
        self, AutoTransitionConfig, AutoTransitionMode, AutodjTransitionEngine, DjMode,
        GapKillerConfig, MixxxPlannerConfig, TransitionDecisionDebug,
    },
    request_policy::{self, RequestLogEntry, RequestPolicy, RequestStatus},
    rotation::{self, ClockwheelConfig, Playlist, RotationRuleRow},
    show_scheduler::{self, ScheduledEvent, Show},
};
use crate::state::AppState;
/// Phase 3 — Automation & Scheduling commands
use tauri::State;
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnqueuedClockwheelTrack {
    pub queue_id: i64,
    pub song: rotation::SongCandidate,
}

// ── DJ Mode ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_dj_mode(state: State<'_, AppState>) -> Result<String, String> {
    if let Some(pool) = &state.local_db {
        if let Ok(saved) = crate::db::local::get_runtime_dj_mode(pool).await {
            let mode = DjMode::from_str(&saved);
            autodj::set_dj_mode(mode);
            return Ok(mode.as_str().to_string());
        }
    }
    Ok(autodj::get_dj_mode().as_str().to_string())
}

#[tauri::command]
pub async fn set_dj_mode(mode: String, state: State<'_, AppState>) -> Result<(), String> {
    let mode_enum = DjMode::from_str(&mode);
    autodj::set_dj_mode(mode_enum);
    if let Some(pool) = &state.local_db {
        crate::db::local::save_runtime_dj_mode(pool, mode_enum.as_str())
            .await
            .map_err(|e| format!("Failed to persist DJ mode: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_autodj_transition_config(
    state: State<'_, AppState>,
) -> Result<AutoTransitionConfig, String> {
    if let Some(pool) = &state.local_db {
        if let Ok(Some(json)) = crate::db::local::load_autodj_transition_config(pool).await {
            let cfg = parse_autodj_transition_config_json(&json);
            autodj::set_auto_transition_config(cfg.clone());
            return Ok(cfg);
        }
    }
    Ok(autodj::get_auto_transition_config())
}

#[tauri::command]
pub async fn set_autodj_transition_config(
    config: AutoTransitionConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    autodj::set_auto_transition_config(config.clone());
    if let Some(pool) = &state.local_db {
        let json = serde_json::to_string(&config).map_err(|e| format!("Serialize error: {e}"))?;
        crate::db::local::save_autodj_transition_config(pool, &json)
            .await
            .map_err(|e| format!("DB error: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn recalculate_autodj_plan_now() -> Result<(), String> {
    autodj::request_replan();
    Ok(())
}

#[tauri::command]
pub async fn get_last_transition_decision() -> Result<TransitionDecisionDebug, String> {
    Ok(autodj::get_last_transition_decision())
}

#[derive(Debug, serde::Deserialize)]
struct LegacyAutoTransitionConfig {
    enabled: Option<bool>,
    mode: Option<AutoTransitionMode>,
    transition_time_sec: Option<i32>,
    min_track_duration_ms: Option<u32>,
}

pub(crate) fn parse_autodj_transition_config_json(json: &str) -> AutoTransitionConfig {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return AutoTransitionConfig::default(),
    };

    if value.get("engine").is_some() {
        return serde_json::from_value(value).unwrap_or_default();
    }

    // Migration: old planner-only shape -> advanced Mixxx planner engine.
    let legacy: LegacyAutoTransitionConfig =
        serde_json::from_value(value).unwrap_or(LegacyAutoTransitionConfig {
            enabled: None,
            mode: None,
            transition_time_sec: None,
            min_track_duration_ms: None,
        });

    AutoTransitionConfig {
        engine: AutodjTransitionEngine::MixxxPlanner,
        mixxx_planner_config: MixxxPlannerConfig {
            enabled: legacy.enabled.unwrap_or(true),
            mode: legacy.mode.unwrap_or(AutoTransitionMode::FullIntroOutro),
            transition_time_sec: legacy.transition_time_sec.unwrap_or(10),
            min_track_duration_ms: legacy.min_track_duration_ms.unwrap_or(200),
        },
    }
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
pub async fn delete_rotation_rule(state: State<'_, AppState>, id: i64) -> Result<(), String> {
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
pub async fn save_playlist(state: State<'_, AppState>, playlist: Playlist) -> Result<i64, String> {
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
    let sam_guard = state.sam_db.read().await;
    let sam_pool = sam_guard.as_ref().ok_or("SAM DB not connected")?;
    rotation::select_next_track(local_pool, sam_pool, None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_clockwheel_config(state: State<'_, AppState>) -> Result<ClockwheelConfig, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::get_clockwheel_config(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_clockwheel_config(
    state: State<'_, AppState>,
    config: ClockwheelConfig,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    rotation::save_clockwheel_config(pool, &config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_song_directories(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<String>, String> {
    let sam_guard = state.sam_db.read().await;
    let sam_pool = sam_guard.as_ref().ok_or("SAM DB not connected")?;
    rotation::get_song_directories(sam_pool, limit.unwrap_or(3000))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enqueue_next_clockwheel_track(
    state: State<'_, AppState>,
    slot_id: Option<String>,
) -> Result<Option<EnqueuedClockwheelTrack>, String> {
    let local_pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let sam_guard = state.sam_db.read().await;
    let sam_pool = sam_guard.as_ref().ok_or("SAM DB not connected")?;

    let candidate = if let Some(slot_id) = slot_id.as_deref() {
        rotation::select_next_track_for_slot(local_pool, sam_pool, slot_id)
            .await
            .map_err(|e| e.to_string())?
    } else {
        rotation::select_next_track(local_pool, sam_pool, None)
            .await
            .map_err(|e| e.to_string())?
    };

    let Some(song) = candidate else {
        return Ok(None);
    };

    let queue_id = crate::db::sam::add_to_queue(sam_pool, song.song_id)
        .await
        .map_err(|e| format!("Queue insert failed: {e}"))?;

    Ok(Some(EnqueuedClockwheelTrack { queue_id, song }))
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
pub async fn get_gap_killer_config(state: State<'_, AppState>) -> Result<GapKillerConfig, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let row: Option<String> =
        sqlx::query_scalar("SELECT gap_killer_json FROM gap_killer_config WHERE id = 1")
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
         ON CONFLICT(id) DO UPDATE SET gap_killer_json = excluded.gap_killer_json",
    )
    .bind(&json)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Request Policy ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_request_policy(state: State<'_, AppState>) -> Result<RequestPolicy, String> {
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
pub async fn accept_request_p3(state: State<'_, AppState>, id: i64) -> Result<(), String> {
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
