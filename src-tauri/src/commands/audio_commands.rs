use std::path::PathBuf;
use tauri::State;

use crate::{
    audio::{crossfade::DeckId, engine::DeckStateEvent},
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
pub async fn seek_deck(
    deck: String,
    position_ms: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deck_id = parse_deck(&deck)?;
    state.engine.lock().unwrap().seek(deck_id, position_ms)
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
