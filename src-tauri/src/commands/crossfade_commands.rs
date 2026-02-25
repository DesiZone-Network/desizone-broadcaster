use tauri::State;

use crate::{
    audio::{
        crossfade::{CrossfadeConfig, CrossfadeMode, CrossfadeTriggerMode, FadeCurve},
        engine::ManualFadeDirection,
    },
    state::AppState,
};

use super::audio_commands::parse_deck;

#[tauri::command]
pub async fn get_crossfade_config(state: State<'_, AppState>) -> Result<CrossfadeConfig, String> {
    if let Some(pool) = &state.local_db {
        if let Ok(Some(json)) = crate::db::local::load_crossfade_config(pool).await {
            let cfg = parse_crossfade_config_json(&json);
            let _ = state
                .engine
                .lock()
                .unwrap()
                .set_crossfade_config(cfg.clone());
            return Ok(cfg);
        }
    }
    Ok(normalize_crossfade_config(
        state.engine.lock().unwrap().get_crossfade_config(),
    ))
}

#[tauri::command]
pub async fn set_crossfade_config(
    config: CrossfadeConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config = normalize_crossfade_config(config);
    // Persist to SQLite
    if let Some(pool) = &state.local_db {
        let json = serde_json::to_string(&config).map_err(|e| format!("Serialize error: {e}"))?;
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

#[tauri::command]
pub async fn set_manual_crossfade(position: f32, state: State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().unwrap().set_manual_crossfade(position)
}

#[tauri::command]
pub async fn trigger_manual_fade(
    direction: String,
    duration_ms: u32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dir = match direction.as_str() {
        "a_to_b" => ManualFadeDirection::AtoB,
        "b_to_a" => ManualFadeDirection::BtoA,
        _ => return Err(format!("Unknown fade direction: {direction}")),
    };
    state
        .engine
        .lock()
        .unwrap()
        .trigger_manual_fade(dir, duration_ms)
}

/// Returns a preview of the crossfade curve pair for the frontend visualiser.
#[tauri::command]
pub async fn get_fade_curve_preview(
    curve: FadeCurve,
    steps: Option<usize>,
) -> Result<Vec<crate::audio::crossfade::CurvePoint>, String> {
    Ok(curve.preview(steps.unwrap_or(50)))
}

pub(crate) fn parse_crossfade_config_json(json: &str) -> CrossfadeConfig {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return CrossfadeConfig::default(),
    };

    let mut cfg: CrossfadeConfig = serde_json::from_value(value.clone()).unwrap_or_default();

    if value.get("trigger_mode").is_none() {
        cfg.trigger_mode = match cfg.crossfade_mode {
            CrossfadeMode::Fixed => CrossfadeTriggerMode::FixedPointMs,
            CrossfadeMode::Manual => CrossfadeTriggerMode::Manual,
            _ => CrossfadeTriggerMode::AutoDetectDb,
        };
    }

    normalize_crossfade_config(cfg)
}

pub(crate) fn normalize_crossfade_config(mut cfg: CrossfadeConfig) -> CrossfadeConfig {
    if matches!(
        cfg.crossfade_mode,
        CrossfadeMode::AutoDetect | CrossfadeMode::Fixed | CrossfadeMode::Manual
    ) {
        cfg.crossfade_mode = CrossfadeMode::Overlap;
    }

    if cfg.fixed_crossfade_point_ms.is_none() {
        cfg.fixed_crossfade_point_ms = Some(cfg.fixed_crossfade_ms.max(500));
    }

    cfg.fade_out_level_pct = cfg.fade_out_level_pct.clamp(0, 100);
    cfg.fade_in_level_pct = cfg.fade_in_level_pct.clamp(0, 100);
    cfg.min_fade_time_ms = cfg.min_fade_time_ms.max(100);
    cfg.max_fade_time_ms = cfg.max_fade_time_ms.max(cfg.min_fade_time_ms);
    cfg
}
