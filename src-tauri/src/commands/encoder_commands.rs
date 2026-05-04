/// Phase 4 — Encoder & Stats Tauri commands
///
/// All commands operate on `AppState.encoder_manager` (EncoderManager).
use std::time::Duration;

use tauri::State;

use crate::{
    db::local,
    state::AppState,
    stats::icecast_stats::{self, ListenerSnapshot},
    stream::{broadcaster::EncoderRuntimeState, encoder_manager::EncoderConfig},
};

fn ensure_broadcast_loop(state: &AppState) {
    let mut started = state.broadcaster_loop_started.lock().unwrap();
    if *started {
        return;
    }

    let consumer = match state.engine.lock().unwrap().encoder_consumer.take() {
        Some(c) => c,
        None => {
            log::warn!("Encoder broadcast loop not started: master encoder consumer unavailable");
            return;
        }
    };

    let broadcaster = state.broadcaster.clone();
    tauri::async_runtime::spawn(async move {
        let mut master = consumer;
        let mut interval = tokio::time::interval(Duration::from_millis(5));
        loop {
            interval.tick().await;
            broadcaster.distribute(&mut master);
        }
    });

    *started = true;
    log::info!("Encoder broadcast loop started");
}

fn current_engine_sample_rate(state: &AppState) -> u32 {
    state.engine.lock().unwrap().get_output_sample_rate()
}

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
    log::info!("save_encoder: request for id={}", encoder.id);
    let id = state.encoder_manager.save_encoder(encoder);
    if let Some(pool) = &state.local_db {
        let cfg = state
            .encoder_manager
            .get_encoder(id)
            .ok_or_else(|| format!("Encoder {id} missing after save"))?;
        local::save_encoder_config(pool, &cfg).await?;
        log::info!("save_encoder: persisted encoder id={id}");
    }
    Ok(id)
}

#[tauri::command]
pub async fn delete_encoder(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.delete_encoder(id);
    if let Some(pool) = &state.local_db {
        local::delete_encoder_config(pool, id).await?;
        log::info!("delete_encoder: removed persisted encoder id={id}");
    }
    Ok(())
}

// ── Start / Stop ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_encoder(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    ensure_broadcast_loop(&state);
    let source_sr = current_engine_sample_rate(&state);
    state
        .encoder_manager
        .start_encoder_with_sample_rate(id, Some(source_sr), None);
    Ok(())
}

#[tauri::command]
pub async fn stop_encoder(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.stop_encoder(id);
    Ok(())
}

#[tauri::command]
pub async fn start_all_encoders(state: State<'_, AppState>) -> Result<(), String> {
    ensure_broadcast_loop(&state);
    let source_sr = current_engine_sample_rate(&state);
    state
        .encoder_manager
        .start_all_with_sample_rate(Some(source_sr));
    Ok(())
}

#[tauri::command]
pub async fn stop_all_encoders(state: State<'_, AppState>) -> Result<(), String> {
    state.encoder_manager.stop_all();
    Ok(())
}

// ── Connection test ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn test_encoder_connection(id: i64, state: State<'_, AppState>) -> Result<bool, String> {
    log::info!("test_encoder_connection: starting test for encoder_id={id}");
    match state.encoder_manager.test_connection(id).await {
        Ok(()) => {
            log::info!("test_encoder_connection: success for encoder_id={id}");
            Ok(true)
        }
        Err(e) => {
            log::warn!("test_encoder_connection: failed for encoder_id={id}: {e}");
            Err(e)
        }
    }
}

// ── Runtime state ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_encoder_runtime(
    state: State<'_, AppState>,
) -> Result<Vec<EncoderRuntimeState>, String> {
    Ok(state.encoder_manager.get_all_runtime())
}

// ── Recording ─────────────────────────────────────────────────────────────────

/// Recording is managed via the same start/stop_encoder commands (file encoders).
/// Convenience aliases for explicit UI calls.
#[tauri::command]
pub async fn start_recording(encoder_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    ensure_broadcast_loop(&state);
    let source_sr = current_engine_sample_rate(&state);
    state
        .encoder_manager
        .start_encoder_with_sample_rate(encoder_id, Some(source_sr), None);
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(encoder_id: i64, state: State<'_, AppState>) -> Result<(), String> {
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
