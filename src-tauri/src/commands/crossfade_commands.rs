use tauri::State;

use crate::{
    audio::crossfade::{CrossfadeConfig, FadeCurve},
    state::AppState,
};

use super::audio_commands::parse_deck;

#[tauri::command]
pub async fn get_crossfade_config(
    state: State<'_, AppState>,
) -> Result<CrossfadeConfig, String> {
    Ok(state.engine.lock().unwrap().get_crossfade_config())
}

#[tauri::command]
pub async fn set_crossfade_config(
    config: CrossfadeConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Persist to SQLite
    if let Some(pool) = &state.local_db {
        let json = serde_json::to_string(&config)
            .map_err(|e| format!("Serialize error: {e}"))?;
        crate::db::local::save_crossfade_config(pool, &json)
            .await
            .map_err(|e| format!("DB error: {e}"))?;
    }
    state.engine.lock().unwrap().set_crossfade_config(config)
}

#[tauri::command]
pub async fn start_crossfade(
    outgoing: String,
    incoming: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let out_id = parse_deck(&outgoing)?;
    let in_id = parse_deck(&incoming)?;
    state.engine.lock().unwrap().start_crossfade(out_id, in_id)
}

/// Returns a preview of the crossfade curve pair for the frontend visualiser.
#[tauri::command]
pub async fn get_fade_curve_preview(
    curve: FadeCurve,
    steps: Option<usize>,
) -> Result<Vec<crate::audio::crossfade::CurvePoint>, String> {
    Ok(curve.preview(steps.unwrap_or(50)))
}
