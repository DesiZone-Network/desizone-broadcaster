use tauri::State;

use crate::{
    db::{
        local::get_sam_db_config,
        sam::{self, HistoryEntry, QueueEntry, SamSong, SongUpdateFields},
    },
    scheduler::rotation,
    state::AppState,
};

#[tauri::command]
pub async fn get_queue(state: State<'_, AppState>) -> Result<Vec<QueueEntry>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    sam::get_queue(pool)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn add_to_queue(song_id: i64, state: State<'_, AppState>) -> Result<i64, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    let queue_id = sam::add_to_queue(pool, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    if let Some(local) = &state.local_db {
        if let Err(err) = rotation::apply_weight_delta_on_request(local, pool, song_id).await {
            log::warn!(
                "Failed to apply on-request weight adjustment for song {}: {}",
                song_id,
                err
            );
        }
    }

    Ok(queue_id)
}

#[tauri::command]
pub async fn remove_from_queue(queue_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    sam::remove_from_queue(pool, queue_id)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

/// Mark a queue entry as completed: removes it from `queuelist` and writes a
/// full metadata snapshot to `historylist`.  Replaces the old `mark_played` command.
#[tauri::command]
pub async fn complete_queue_item(
    queue_id: i64,
    song_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;

    // Fetch the full song record so we can snapshot metadata into historylist
    let song = sam::get_song(pool, song_id)
        .await
        .map_err(|e| format!("DB error fetching song: {e}"))?
        .ok_or_else(|| format!("Song {song_id} not found in SAM DB"))?;

    sam::complete_track(pool, queue_id, &song)
        .await
        .map_err(|e| format!("DB error completing track: {e}"))
}

#[tauri::command]
pub async fn search_songs(
    query: String,
    search_artist: Option<bool>,
    search_title: Option<bool>,
    search_album: Option<bool>,
    search_filename: Option<bool>,
    song_type: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<SamSong>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;

    let mut songs = sam::search_songs(
        pool,
        &query,
        search_artist.unwrap_or(true),
        search_title.unwrap_or(true),
        search_album.unwrap_or(false),
        search_filename.unwrap_or(false),
        song_type.as_deref(),
        limit.unwrap_or(500),
        offset.unwrap_or(0),
    )
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    // Apply path prefix translation if configured
    if let Some(local) = &state.local_db {
        if let Ok(cfg) = get_sam_db_config(local).await {
            if !cfg.path_prefix_from.is_empty() {
                for song in &mut songs {
                    song.filename = sam::translate_path(
                        &song.filename,
                        &cfg.path_prefix_from,
                        &cfg.path_prefix_to,
                    );
                }
            }
        }
    }

    Ok(songs)
}

#[tauri::command]
pub async fn get_songs_by_weight_range(
    min_weight: f64,
    max_weight: f64,
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<SamSong>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;

    let mut songs = sam::get_songs_by_weight_range(
        pool,
        min_weight,
        max_weight,
        limit.unwrap_or(500),
        offset.unwrap_or(0),
    )
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    // Apply same path translation
    if let Some(local) = &state.local_db {
        if let Ok(cfg) = get_sam_db_config(local).await {
            if !cfg.path_prefix_from.is_empty() {
                for song in &mut songs {
                    song.filename = sam::translate_path(
                        &song.filename,
                        &cfg.path_prefix_from,
                        &cfg.path_prefix_to,
                    );
                }
            }
        }
    }

    Ok(songs)
}

#[tauri::command]
pub async fn get_song_types(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    sam::get_distinct_song_types(pool)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn get_history(
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryEntry>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    sam::get_history(pool, limit.unwrap_or(20))
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn get_songs_in_category(
    category_id: i64,
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<SamSong>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;

    let mut songs =
        sam::get_songs_in_category(pool, category_id, limit.unwrap_or(500), offset.unwrap_or(0))
            .await
            .map_err(|e| format!("DB error: {e}"))?;

    // Apply same path translation as search_songs
    if let Some(local) = &state.local_db {
        if let Ok(cfg) = get_sam_db_config(local).await {
            if !cfg.path_prefix_from.is_empty() {
                for song in &mut songs {
                    song.filename = sam::translate_path(
                        &song.filename,
                        &cfg.path_prefix_from,
                        &cfg.path_prefix_to,
                    );
                }
            }
        }
    }

    Ok(songs)
}

#[tauri::command]
pub async fn get_song(song_id: i64, state: State<'_, AppState>) -> Result<Option<SamSong>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    let song = sam::get_song(pool, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))?;
    let Some(mut song) = song else {
        return Ok(None);
    };
    // Apply path translation
    if let Some(local) = &state.local_db {
        if let Ok(cfg) = get_sam_db_config(local).await {
            if !cfg.path_prefix_from.is_empty() {
                song.filename =
                    sam::translate_path(&song.filename, &cfg.path_prefix_from, &cfg.path_prefix_to);
            }
        }
    }
    Ok(Some(song))
}

#[tauri::command]
pub async fn update_song(
    song_id: i64,
    fields: SongUpdateFields,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    sam::update_song(pool, song_id, fields)
        .await
        .map_err(|e| format!("DB error: {e}"))
}
