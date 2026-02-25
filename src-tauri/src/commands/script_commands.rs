/// `commands/script_commands.rs` — Phase 5 Tauri commands for scripting
use tauri::State;

use crate::{
    scripting::engine::{Script, ScriptEngine, ScriptRunResult},
    state::AppState,
};

/// Return all scripts (enabled + disabled).
#[tauri::command]
pub async fn get_scripts(state: State<'_, AppState>) -> Result<Vec<Script>, String> {
    let mut scripts = state.script_engine.get_scripts();
    scripts.sort_by_key(|s| s.id);
    Ok(scripts)
}

/// Create or update a script. Returns the script id.
#[tauri::command]
pub async fn save_script(state: State<'_, AppState>, script: Script) -> Result<i64, String> {
    let is_new = script.id == 0;
    let id = state.script_engine.save_script(script);
    // Start event listener loop for new scripts
    if is_new {
        state.script_engine.start_event_loop(id);
    }
    Ok(id)
}

/// Delete a script by id.
#[tauri::command]
pub async fn delete_script(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.script_engine.delete_script(id);
    Ok(())
}

/// Run a script immediately (manual trigger).
#[tauri::command]
pub async fn run_script(state: State<'_, AppState>, id: i64) -> Result<ScriptRunResult, String> {
    Ok(state.script_engine.run_script(id).await)
}

/// Return the last N log entries for a script.
#[tauri::command]
pub async fn get_script_log(
    state: State<'_, AppState>,
    id: i64,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let entries = state.script_engine.get_log(id, limit.unwrap_or(50));
    let json = entries
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "level": e.level,
                "message": e.message,
                "timestamp": e.timestamp,
            })
        })
        .collect();
    Ok(json)
}
