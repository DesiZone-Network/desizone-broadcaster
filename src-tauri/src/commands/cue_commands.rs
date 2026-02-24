use tauri::State;

use crate::{db::local::CuePoint, state::AppState};

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
        &CuePoint { id: None, song_id, name, position_ms },
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
