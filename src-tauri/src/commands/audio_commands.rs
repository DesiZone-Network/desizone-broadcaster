use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

use crate::{
    audio::{
        crossfade::DeckId,
        device_manager::{AudioOutputDevice, AudioOutputRoutingConfig, AudioOutputStatus},
        engine::DeckStateEvent,
    },
    db::local::MonitorRoutingConfig,
    state::AppState,
};

pub(crate) fn parse_deck(deck: &str) -> Result<DeckId, String> {
    match deck {
        "deck_a" => Ok(DeckId::DeckA),
        "deck_b" => Ok(DeckId::DeckB),
        "sound_fx" => Ok(DeckId::SoundFx),
        "aux_1" => Ok(DeckId::Aux1),
        "aux_2" => Ok(DeckId::Aux2),
        "voice_fx" => Ok(DeckId::VoiceFx),
        _ => Err(format!("Unknown deck: {deck}")),
    }
}

#[tauri::command]
pub async fn load_track(
    deck: String,
    file_path: String,
    song_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    let path = PathBuf::from(&file_path);

    // Validate before handing off to the RT ring buffer so the frontend
    // receives an immediate, descriptive error instead of silent failure.
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }
    if !path.is_file() {
        return Err(format!("Path is not a file: {file_path}"));
    }

    state
        .engine
        .lock()
        .unwrap()
        .load_track(deck_id, path, song_id)
}

#[tauri::command]
pub async fn play_deck(deck: String, state: State<'_, AppState>) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state.engine.lock().unwrap().play(deck_id)
}

#[tauri::command]
pub async fn pause_deck(deck: String, state: State<'_, AppState>) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state.engine.lock().unwrap().pause(deck_id)
}

#[tauri::command]
pub async fn stop_deck(deck: String, state: State<'_, AppState>) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    let mut engine = state.engine.lock().unwrap();
    let _ = engine.pause(deck_id);
    engine.seek(deck_id, 0)
}

#[tauri::command]
pub async fn next_deck(deck: String, state: State<'_, AppState>) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state.engine.lock().unwrap().stop_with_completion(deck_id)
}

#[tauri::command]
pub async fn seek_deck(
    deck: String,
    position_ms: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state.engine.lock().unwrap().seek(deck_id, position_ms)
}

#[tauri::command]
pub async fn jog_deck(
    deck: String,
    delta_steps: i8,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    if delta_steps == 0 {
        return Ok(());
    }

    let deck_state = { state.engine.lock().unwrap().get_deck_state(deck_id) };
    let Some(deck_state) = deck_state else {
        return Ok(());
    };
    if deck_state.duration_ms == 0 {
        return Ok(());
    }

    let clamped_steps = delta_steps.clamp(-12, 12) as i64;
    let step_ms: i64 = if deck_state.state == "playing" || deck_state.state == "crossfading" {
        25
    } else {
        150
    };
    let position = deck_state.position_ms as i64;
    let duration = deck_state.duration_ms as i64;
    let target = (position + (clamped_steps * step_ms)).clamp(0, duration) as u64;

    state.engine.lock().unwrap().seek(deck_id, target)
}

#[tauri::command]
pub async fn set_channel_gain(
    deck: String,
    gain: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state.engine.lock().unwrap().set_channel_gain(deck_id, gain)
}

#[tauri::command]
pub async fn set_deck_bass(
    deck: String,
    bass_db: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state.engine.lock().unwrap().set_deck_bass(deck_id, bass_db)
}

#[tauri::command]
pub async fn set_deck_filter(
    deck: String,
    amount: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state
        .engine
        .lock()
        .unwrap()
        .set_deck_filter(deck_id, amount)
}

#[tauri::command]
pub async fn set_master_level(level: f32, state: State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().unwrap().set_master_level(level)
}

#[tauri::command]
pub async fn get_master_level(state: State<'_, AppState>) -> Result<f32, String> {
    Ok(state.engine.lock().unwrap().get_master_level())
}

#[tauri::command]
pub async fn set_local_monitor_muted(
    muted: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.engine.lock().unwrap().set_local_monitor_muted(muted)
}

#[tauri::command]
pub async fn get_local_monitor_muted(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.engine.lock().unwrap().get_local_monitor_muted())
}

#[tauri::command]
pub async fn set_headphone_mix(value: f32, state: State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().unwrap().set_headphone_mix(value)
}

#[tauri::command]
pub async fn set_headphone_level(value: f32, state: State<'_, AppState>) -> Result<(), String> {
    state.engine.lock().unwrap().set_headphone_level(value)
}

#[tauri::command]
pub async fn get_headphone_mix(state: State<'_, AppState>) -> Result<f32, String> {
    Ok(state.engine.lock().unwrap().get_headphone_mix())
}

#[tauri::command]
pub async fn get_headphone_level(state: State<'_, AppState>) -> Result<f32, String> {
    Ok(state.engine.lock().unwrap().get_headphone_level())
}

#[tauri::command]
pub async fn set_deck_cue_enabled(
    deck: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state
        .engine
        .lock()
        .unwrap()
        .set_deck_cue_preview_enabled(deck_id, enabled)
}

#[tauri::command]
pub async fn list_audio_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    crate::audio::engine::AudioEngine::list_audio_output_devices()
}

#[tauri::command]
pub async fn get_audio_output_status(
    state: State<'_, AppState>,
) -> Result<AudioOutputStatus, String> {
    Ok(state.engine.lock().unwrap().get_audio_output_status())
}

#[tauri::command]
pub async fn apply_audio_output_routing(
    config: AudioOutputRoutingConfig,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AudioOutputStatus, String> {
    let auto_fallback = config.auto_fallback;
    let (cue_level, master_level) = {
        let engine = state.engine.lock().unwrap();
        (engine.get_headphone_level(), engine.get_master_level())
    };

    let status = state
        .engine
        .lock()
        .unwrap()
        .apply_audio_output_routing(config)?;

    if let Some(pool) = &state.local_db {
        let monitor_cfg = MonitorRoutingConfig {
            master_device_id: status.master_device_id.clone(),
            cue_device_id: status.cue_device_id.clone(),
            cue_mix_mode: match status.active_mode {
                crate::audio::device_manager::AudioOutputMode::SingleDeviceFourChannel => {
                    "single_device_four_channel".to_string()
                }
                crate::audio::device_manager::AudioOutputMode::DualDeviceSplit => {
                    "dual_device_split".to_string()
                }
                crate::audio::device_manager::AudioOutputMode::SingleDeviceStereo => {
                    "single_device_stereo".to_string()
                }
            },
            cue_level,
            master_level,
            auto_fallback,
        };
        let pool = pool.clone();
        tauri::async_runtime::spawn(async move {
            let _ = crate::db::local::save_monitor_routing_config(&pool, &monitor_cfg).await;
        });
    }

    let _ = app.emit("audio_output_status_changed", status.clone());
    if let Some(msg) = status.last_error.clone() {
        let _ = app.emit(
            "audio_output_error",
            serde_json::json!({
                "message": msg
            }),
        );
    }
    Ok(status)
}

#[tauri::command]
pub async fn set_deck_pitch(
    deck: String,
    pitch_pct: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state
        .engine
        .lock()
        .unwrap()
        .set_deck_pitch(deck_id, pitch_pct)
}

#[tauri::command]
pub async fn set_deck_tempo(
    deck: String,
    tempo_pct: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state
        .engine
        .lock()
        .unwrap()
        .set_deck_tempo(deck_id, tempo_pct)
}

#[tauri::command]
pub async fn set_deck_loop(
    deck: String,
    start_ms: u64,
    end_ms: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state
        .engine
        .lock()
        .unwrap()
        .set_deck_loop(deck_id, start_ms, end_ms)
}

#[tauri::command]
pub async fn clear_deck_loop(deck: String, state: State<'_, AppState>) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    let mut engine = state.engine.lock().unwrap();
    let current_pos = engine.get_deck_state(deck_id).map(|s| s.position_ms);
    engine.clear_deck_loop(deck_id)?;
    if let Some(position_ms) = current_pos {
        let _ = engine.seek(deck_id, position_ms);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_deck_state(
    deck: String,
    state: State<'_, AppState>,
) -> Result<Option<DeckStateEvent>, String> {
    let deck_id = parse_deck(&deck)?;
    Ok(state.engine.lock().unwrap().get_deck_state(deck_id))
}

#[tauri::command]
pub async fn get_vu_readings(
    state: State<'_, AppState>,
) -> Result<Vec<crate::audio::engine::VuEvent>, String> {
    Ok(state.engine.lock().unwrap().get_vu_readings())
}
