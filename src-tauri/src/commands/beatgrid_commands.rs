use std::path::Path;

use tauri::State;

use crate::{db::local::BeatGridAnalysis, state::AppState};

fn file_mtime_ms(path: &Path) -> i64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn analyze_beatgrid(
    song_id: i64,
    file_path: String,
    force_reanalyze: Option<bool>,
    state: State<'_, AppState>,
) -> Result<BeatGridAnalysis, String> {
    let local = state
        .local_db
        .as_ref()
        .ok_or("Local DB not initialised")?
        .clone();
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }
    if !path.is_file() {
        return Err(format!("Path is not a file: {file_path}"));
    }

    let mtime_ms = file_mtime_ms(path);
    if !force_reanalyze.unwrap_or(false) {
        if let Ok(Some(cached)) =
            crate::db::local::get_beatgrid_analysis(&local, song_id, &file_path, mtime_ms).await
        {
            return Ok(cached);
        }
    }

    let analyze_path = path.to_path_buf();
    let computed = tauri::async_runtime::spawn_blocking(move || {
        crate::audio::analyzer::beatgrid::analyze_file(&analyze_path)
    })
    .await
    .map_err(|e| format!("Beat-grid worker join failed: {e}"))??;

    let analysis = BeatGridAnalysis {
        song_id,
        file_path: file_path.clone(),
        mtime_ms,
        bpm: computed.bpm,
        first_beat_ms: computed.first_beat_ms,
        confidence: computed.confidence,
        beat_times_ms: computed.beat_times_ms,
        updated_at: None,
    };
    crate::db::local::save_beatgrid_analysis(&local, &analysis)
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    crate::db::local::get_beatgrid_analysis(&local, song_id, &file_path, mtime_ms)
        .await
        .map_err(|e| format!("DB error: {e}"))?
        .ok_or("Failed to read saved beat-grid".to_string())
}

#[tauri::command]
pub async fn get_beatgrid(
    song_id: i64,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<Option<BeatGridAnalysis>, String> {
    let local = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let path = Path::new(&file_path);
    if !path.exists() || !path.is_file() {
        return Ok(None);
    }
    let mtime_ms = file_mtime_ms(path);
    crate::db::local::get_beatgrid_analysis(local, song_id, &file_path, mtime_ms)
        .await
        .map_err(|e| format!("DB error: {e}"))
}
