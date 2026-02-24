/// Phase 4 — Encoder & Stats Tauri commands
///
/// All commands operate on `AppState.encoder_manager` (EncoderManager).
use tauri::State;

use crate::{
    state::AppState,
    stats::icecast_stats::{self, ListenerSnapshot},
    stream::{
        broadcaster::EncoderRuntimeState,
        encoder_manager::EncoderConfig,
    },
};

// ── Encoder CRUD ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_encoders(state: State<'_, AppState>) -> Result<Vec<EncoderConfig>, String> {
    Ok(state.encoder_manager.get_encoders())
}

#[tauri::command]
pub async fn save_encoder(
    encoder: EncoderConfig,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    Ok(state.encoder_manager.save_encoder(encoder))
}

#[tauri::command]
pub async fn delete_encoder(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.delete_encoder(id);
    Ok(())
}

// ── Start / Stop ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_encoder(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.start_encoder(id, None);
    Ok(())
}

#[tauri::command]
pub async fn stop_encoder(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.stop_encoder(id);
    Ok(())
}

#[tauri::command]
pub async fn start_all_encoders(state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.start_all();
    Ok(())
}

#[tauri::command]
pub async fn stop_all_encoders(state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.stop_all();
    Ok(())
}

// ── Connection test ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn test_encoder_connection(
    id: i64,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    match state.encoder_manager.test_connection(id).await {
        Ok(()) => Ok(true),
        Err(e) => Err(e),
    }
}

// ── Runtime state ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_encoder_runtime(state: State<'_, AppState>) -> Result<Vec<EncoderRuntimeState>, String> {
    Ok(state.encoder_manager.get_all_runtime())
}

// ── Recording ─────────────────────────────────────────────────────────────────

/// Recording is managed via the same start/stop_encoder commands (file encoders).
/// Convenience aliases for explicit UI calls.
#[tauri::command]
pub async fn start_recording(
    encoder_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.encoder_manager.start_encoder(encoder_id, None);
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    encoder_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.encoder_manager.stop_encoder(encoder_id);
    Ok(())
}

// ── Stats ─────────────────────────────────────────────────────────────────────

/// period: '1h' | '6h' | '24h' | '7d'
#[tauri::command]
pub async fn get_listener_stats(
    encoder_id: i64,
    period: String,
    state: State<'_, AppState>,
) -> Result<Vec<ListenerSnapshot>, String> {
    let period_secs = match period.as_str() {
        "1h" => 3600,
        "6h" => 6 * 3600,
        "24h" => 24 * 3600,
        "7d" => 7 * 24 * 3600,
        other => return Err(format!("Unknown period: {other}. Use 1h, 6h, 24h, or 7d")),
    };

    if let Some(pool) = &state.local_db {
        icecast_stats::get_snapshots(pool, encoder_id, period_secs).await
    } else {
        Err("Local database not initialised".to_string())
    }
}

#[tauri::command]
pub async fn get_current_listeners(
    encoder_id: i64,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    Ok(state
        .encoder_manager
        .get_runtime(encoder_id)
        .and_then(|r| r.listeners)
        .unwrap_or(0))
}

// ── Metadata push  ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn push_track_metadata(
    artist: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.encoder_manager.push_metadata(&artist, &title).await;
    Ok(())
}
