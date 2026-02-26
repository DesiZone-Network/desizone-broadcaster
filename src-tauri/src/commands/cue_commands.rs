use tauri::State;

use crate::{
    db::local::{CueKind, CuePoint, CueQuantize, HotCue, MonitorRoutingConfig},
    state::AppState,
};

const HOT_CUE_MIN_SLOT: u8 = 1;
const HOT_CUE_MAX_SLOT: u8 = 8;
const BEATGRID_CONFIDENCE_MIN: f32 = 0.55;

fn validate_slot(slot: u8) -> Result<(), String> {
    if (HOT_CUE_MIN_SLOT..=HOT_CUE_MAX_SLOT).contains(&slot) {
        Ok(())
    } else {
        Err(format!(
            "Hot cue slot must be between {} and {}",
            HOT_CUE_MIN_SLOT, HOT_CUE_MAX_SLOT
        ))
    }
}

async fn maybe_quantize_position(
    state: &AppState,
    song_id: i64,
    position_ms: i64,
    mode: CueQuantize,
) -> Result<(i64, bool), String> {
    if matches!(mode, CueQuantize::Off) {
        return Ok((position_ms.max(0), false));
    }
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let Some(grid) = crate::db::local::get_latest_beatgrid_by_song_id(pool, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))?
    else {
        return Ok((position_ms.max(0), false));
    };
    if grid.confidence < BEATGRID_CONFIDENCE_MIN || grid.beat_times_ms.is_empty() {
        return Ok((position_ms.max(0), false));
    }
    let snapped = crate::audio::analyzer::beatgrid::quantize_position_ms(
        position_ms,
        &grid.beat_times_ms,
        mode,
    );
    Ok((snapped.max(0), true))
}

#[tauri::command]
pub async fn get_cue_points(
    song_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<CuePoint>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::get_cue_points(pool, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn set_cue_point(
    song_id: i64,
    name: String,
    position_ms: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::upsert_cue_point(
        pool,
        &CuePoint {
            id: None,
            song_id,
            name,
            position_ms,
            cue_kind: CueKind::Memory,
            slot: None,
            label: "".to_string(),
            color_hex: "#f59e0b".to_string(),
            updated_at: None,
        },
    )
    .await
    .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn delete_cue_point(
    song_id: i64,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::delete_cue_point(pool, song_id, &name)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

/// Jump a deck to a named cue point (seeks the deck to the stored position).
#[tauri::command]
pub async fn jump_to_cue(
    deck: String,
    song_id: i64,
    cue_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let cues = crate::db::local::get_cue_points(pool, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    let cue = cues
        .into_iter()
        .find(|c| c.name == cue_name)
        .ok_or(format!("Cue '{cue_name}' not found for song {song_id}"))?;

    let deck_id = super::audio_commands::parse_deck(&deck)?;
    state
        .engine
        .lock()
        .unwrap()
        .seek(deck_id, cue.position_ms as u64)
}

#[tauri::command]
pub async fn get_hot_cues(song_id: i64, state: State<'_, AppState>) -> Result<Vec<HotCue>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::get_hot_cues(pool, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn set_hot_cue(
    song_id: i64,
    slot: u8,
    position_ms: i64,
    label: Option<String>,
    color_hex: Option<String>,
    quantize_mode: Option<CueQuantize>,
    state: State<'_, AppState>,
) -> Result<HotCue, String> {
    validate_slot(slot)?;
    let (position_ms, quantized) = maybe_quantize_position(
        &state,
        song_id,
        position_ms,
        quantize_mode.unwrap_or(CueQuantize::Off),
    )
    .await?;

    let cue = HotCue {
        song_id,
        slot,
        position_ms,
        label: label.unwrap_or_else(|| format!("Cue {slot}")),
        color_hex: color_hex.unwrap_or_else(|| "#f59e0b".to_string()),
        quantized,
    };

    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::upsert_hot_cue(pool, &cue)
        .await
        .map_err(|e| format!("DB error: {e}"))?;
    Ok(cue)
}

#[tauri::command]
pub async fn clear_hot_cue(
    song_id: i64,
    slot: u8,
    state: State<'_, AppState>,
) -> Result<(), String> {
    validate_slot(slot)?;
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::clear_hot_cue(pool, song_id, slot)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn trigger_hot_cue(
    deck: String,
    song_id: i64,
    slot: u8,
    quantize_mode: Option<CueQuantize>,
    state: State<'_, AppState>,
) -> Result<HotCue, String> {
    validate_slot(slot)?;
    let deck_id = super::audio_commands::parse_deck(&deck)?;
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let mut cue = crate::db::local::get_hot_cue(pool, song_id, slot)
        .await
        .map_err(|e| format!("DB error: {e}"))?
        .ok_or_else(|| format!("Hot cue {slot} not found for song {song_id}"))?;

    let (snapped, quantized) = maybe_quantize_position(
        &state,
        song_id,
        cue.position_ms,
        quantize_mode.unwrap_or(CueQuantize::Off),
    )
    .await?;
    cue.position_ms = snapped;
    cue.quantized = quantized;

    state
        .engine
        .lock()
        .unwrap()
        .seek(deck_id, cue.position_ms as u64)?;
    Ok(cue)
}

#[tauri::command]
pub async fn rename_hot_cue(
    song_id: i64,
    slot: u8,
    label: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    validate_slot(slot)?;
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::rename_hot_cue(pool, song_id, slot, &label)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn recolor_hot_cue(
    song_id: i64,
    slot: u8,
    color_hex: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    validate_slot(slot)?;
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::recolor_hot_cue(pool, song_id, slot, &color_hex)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn get_monitor_routing_config(
    state: State<'_, AppState>,
) -> Result<MonitorRoutingConfig, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::get_monitor_routing_config(pool)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn set_monitor_routing_config(
    config: MonitorRoutingConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::save_monitor_routing_config(pool, &config)
        .await
        .map_err(|e| format!("DB error: {e}"))?;
    state
        .engine
        .lock()
        .unwrap()
        .set_monitor_routing_config(config);
    Ok(())
}

#[tauri::command]
pub async fn set_deck_cue_preview_enabled(
    deck: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = super::audio_commands::parse_deck(&deck)?;
    state
        .engine
        .lock()
        .unwrap()
        .set_deck_cue_preview_enabled(deck_id, enabled)
}
