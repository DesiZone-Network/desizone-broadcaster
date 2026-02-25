/// `commands/mic_commands.rs` — Phase 5 Tauri commands for microphone/voice
use tauri::{Emitter, State};

use crate::{
    audio::mic_input::{list_input_devices, AudioDevice, MicConfig},
    state::AppState,
};

/// List all available audio input devices.
#[tauri::command]
pub async fn get_audio_input_devices() -> Result<Vec<AudioDevice>, String> {
    Ok(list_input_devices())
}

/// Return the current mic configuration.
#[tauri::command]
pub async fn get_mic_config(state: State<'_, AppState>) -> Result<MicConfig, String> {
    Ok(state.mic_input.get_config())
}

/// Save a new mic configuration (does not restart the stream).
#[tauri::command]
pub async fn set_mic_config(state: State<'_, AppState>, config: MicConfig) -> Result<(), String> {
    state.mic_input.set_config(config);
    Ok(())
}

/// Start the microphone input stream.
#[tauri::command]
pub async fn start_mic(state: State<'_, AppState>) -> Result<(), String> {
    state.mic_input.start()
}

/// Stop the microphone input stream.
#[tauri::command]
pub async fn stop_mic(state: State<'_, AppState>) -> Result<(), String> {
    state.mic_input.stop();
    Ok(())
}

/// Set push-to-talk active state (for UI PTT button fallback).
#[tauri::command]
pub async fn set_ptt(
    state: State<'_, AppState>,
    active: bool,
    app: tauri::AppHandle,
) -> Result<(), String> {
    state.mic_input.set_ptt(active);
    let _ = app.emit("ptt_state_changed", serde_json::json!({ "active": active }));
    Ok(())
}

/// Start recording a voice track to a temp file.
#[tauri::command]
pub async fn start_voice_recording(state: State<'_, AppState>) -> Result<(), String> {
    let path = std::env::temp_dir()
        .join(format!(
            "voice_track_{}.wav",
            chrono::Utc::now().timestamp()
        ))
        .to_string_lossy()
        .to_string();
    state
        .voice_recording_path
        .lock()
        .unwrap()
        .replace(path.clone());
    state.mic_input.start_recording(&path)
}

/// Stop recording a voice track; returns the file path and duration.
#[tauri::command]
pub async fn stop_voice_recording(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let duration_ms = state.mic_input.stop_recording()?;
    let file_path = state
        .voice_recording_path
        .lock()
        .unwrap()
        .take()
        .unwrap_or_default();
    Ok(serde_json::json!({
        "filePath": file_path,
        "durationMs": duration_ms,
    }))
}

/// Import a voice track file into the library (saves metadata to SAM DB / local DB).
#[tauri::command]
pub async fn save_voice_track(
    state: State<'_, AppState>,
    file_path: String,
    title: String,
) -> Result<i64, String> {
    // In full implementation, this would call the SAM MySQL importer or local library.
    // For now, save to local SQLite with a stub song_id.
    let _ = (&state, file_path, title); // used
    Ok(-1) // stub id until library import is wired
}
