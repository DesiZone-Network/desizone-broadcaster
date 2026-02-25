use tauri::State;

use crate::{state::AppState, stream::icecast::IcecastConfig};

/// Start streaming to an Icecast server.
#[tauri::command]
pub async fn start_stream(
    host: String,
    port: u16,
    mount: String,
    password: String,
    bitrate_kbps: u32,
    stream_name: Option<String>,
    genre: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = state.stream_handle.lock().unwrap();
    if guard.is_some() {
        return Err("Stream already running".to_string());
    }

    let consumer = state
        .engine
        .lock()
        .unwrap()
        .encoder_consumer
        .take()
        .ok_or("Encoder consumer already taken")?;

    let config = IcecastConfig {
        host,
        port,
        mount,
        password,
        bitrate_kbps,
        sample_rate: 44100,
        stream_name: stream_name.unwrap_or_else(|| "DesiZone Radio".to_string()),
        genre: genre.unwrap_or_else(|| "Various".to_string()),
        is_shoutcast: false,
    };

    let handle = crate::stream::icecast::start_stream(config, consumer);
    *guard = Some(handle);
    Ok(())
}

/// Stop the active stream.
#[tauri::command]
pub async fn stop_stream(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.stream_handle.lock().unwrap();
    match guard.take() {
        Some(handle) => {
            handle.stop();
            Ok(())
        }
        None => Err("No stream running".to_string()),
    }
}

#[tauri::command]
pub async fn get_stream_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.stream_handle.lock().unwrap().is_some())
}
