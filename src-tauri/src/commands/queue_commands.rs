use tauri::State;

use crate::{db::sam::QueueEntry, state::AppState};

#[tauri::command]
pub async fn get_queue(state: State<'_, AppState>) -> Result<Vec<QueueEntry>, String> {
    let pool = state.sam_db.as_ref().ok_or("SAM DB not connected")?;
    crate::db::sam::get_queue(pool)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn add_to_queue(song_id: i32, state: State<'_, AppState>) -> Result<u64, String> {
    let pool = state.sam_db.as_ref().ok_or("SAM DB not connected")?;
    crate::db::sam::add_to_queue(pool, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn remove_from_queue(queue_id: i32, state: State<'_, AppState>) -> Result<(), String> {
    let pool = state.sam_db.as_ref().ok_or("SAM DB not connected")?;
    crate::db::sam::remove_from_queue(pool, queue_id)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn search_songs(
    query: String,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::sam::SamSong>, String> {
    let pool = state.sam_db.as_ref().ok_or("SAM DB not connected")?;
    crate::db::sam::search_songs(pool, &query, limit.unwrap_or(50))
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn get_history(
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::sam::HistoryEntry>, String> {
    let pool = state.sam_db.as_ref().ok_or("SAM DB not connected")?;
    crate::db::sam::get_history(pool, limit.unwrap_or(20))
        .await
        .map_err(|e| format!("DB error: {e}"))
}
