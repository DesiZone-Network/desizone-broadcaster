use serde::{Deserialize, Serialize};
use tauri::State;

use crate::gateway::client::{GatewayClient, GatewayMessage, GatewayStatus};
use crate::gateway::remote_dj::{DjPermissions, RemoteSession};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotStatus {
    pub enabled: bool,
    pub mode: String, // "rotation" | "queue" | "scheduled"
    pub current_rule: Option<String>,
}

/// Connect to the DBE gateway
#[tauri::command]
pub async fn connect_gateway(
    url: String,
    token: String,
    state: State<'_, AppState>,
) -> Result<GatewayStatus, String> {
    let mut client = GatewayClient::new(url.clone(), token);

    // Create message handler
    client
        .connect(move |msg| {
            // Handle incoming messages from gateway
            match msg {
                GatewayMessage::RemoteCommand {
                    session_id,
                    command,
                } => {
                    log::info!(
                        "Remote command from session {}: {:?}",
                        session_id,
                        command
                    );
                    // Commands will be handled via Tauri events
                }
                GatewayMessage::RemoteDjJoined {
                    session_id,
                    user_id: _,
                    display_name,
                } => {
                    log::info!("Remote DJ joined: {} ({})", display_name, session_id);
                }
                GatewayMessage::RemoteDjLeft { session_id } => {
                    log::info!("Remote DJ left: {}", session_id);
                }
                _ => {}
            }
        })
        .await?;

    let status = client.get_status().await;

    // Store client in state
    *state.gateway_client.lock().unwrap() = Some(client);

    Ok(status)
}

/// Disconnect from gateway
#[tauri::command]
pub async fn disconnect_gateway(state: State<'_, AppState>) -> Result<(), String> {
    let mut client = {
        let mut client_guard = state.gateway_client.lock().unwrap();
        client_guard.take()
    };

    if let Some(ref mut c) = client {
        c.disconnect().await;
    }
    Ok(())
}

/// Get gateway connection status
#[tauri::command]
pub async fn get_gateway_status(state: State<'_, AppState>) -> Result<GatewayStatus, String> {
    let client = {
        let client_guard = state.gateway_client.lock().unwrap();
        client_guard.as_ref().cloned()
    };

    if let Some(c) = client {
        Ok(c.get_status().await)
    } else {
        Ok(GatewayStatus {
            connected: false,
            url: String::new(),
            reconnecting: false,
            last_error: Some("Not connected".to_string()),
        })
    }
}

/// Set AutoPilot mode
#[tauri::command]
pub async fn set_autopilot(
    enabled: bool,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut autopilot = state.autopilot_status.lock().unwrap();
    autopilot.enabled = enabled;
    autopilot.mode = mode;

    // Push to gateway if connected
    if let Some(_client) = state.gateway_client.lock().unwrap().as_ref() {
        // TODO: send autopilot status update to gateway
        log::info!("AutoPilot status updated: enabled={}", enabled);
    }

    Ok(())
}

/// Get AutoPilot status
#[tauri::command]
pub fn get_autopilot_status(state: State<'_, AppState>) -> Result<AutoPilotStatus, String> {
    let autopilot = state.autopilot_status.lock().unwrap();
    Ok(autopilot.clone())
}

/// Get active remote DJ sessions
#[tauri::command]
pub fn get_remote_sessions(state: State<'_, AppState>) -> Result<Vec<RemoteSession>, String> {
    let sessions = state.remote_sessions.lock().unwrap();
    Ok(sessions.values().cloned().collect())
}

/// Kick a remote DJ session
#[tauri::command]
pub async fn kick_remote_dj(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut sessions = state.remote_sessions.lock().unwrap();
    sessions.remove(&session_id);

    // TODO: Send kick message to gateway
    log::info!("Kicked remote DJ session: {}", session_id);

    Ok(())
}

/// Set remote DJ permissions for a user
#[tauri::command]
pub fn set_remote_dj_permissions(
    session_id: String,
    permissions: DjPermissions,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut perms = state.remote_dj_permissions.lock().unwrap();
    perms.insert(session_id.clone(), permissions);

    log::info!("Updated permissions for session: {}", session_id);

    Ok(())
}

/// Get remote DJ permissions for a session
#[tauri::command]
pub fn get_remote_dj_permissions(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<DjPermissions, String> {
    let perms = state.remote_dj_permissions.lock().unwrap();
    Ok(perms
        .get(&session_id)
        .cloned()
        .unwrap_or_else(DjPermissions::default))
}

/// Start live talk mode (mic to air)
#[tauri::command]
pub fn start_live_talk(channel: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut live_talk = state.live_talk_active.lock().unwrap();
    *live_talk = Some(channel.clone());

    log::info!("Live talk started on channel: {}", channel);

    // Push to gateway if connected
    if let Some(_client) = state.gateway_client.lock().unwrap().as_ref() {
        // TODO: notify gateway of live talk start
    }

    Ok(())
}

/// Stop live talk mode
#[tauri::command]
pub fn stop_live_talk(state: State<'_, AppState>) -> Result<(), String> {
    let mut live_talk = state.live_talk_active.lock().unwrap();
    *live_talk = None;

    log::info!("Live talk stopped");

    // Push to gateway if connected
    if let Some(_client) = state.gateway_client.lock().unwrap().as_ref() {
        // TODO: notify gateway of live talk stop
    }

    Ok(())
}

/// Set mix-minus (audio without mic return for remote callers)
#[tauri::command]
pub fn set_mix_minus(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    let mut mix_minus = state.mix_minus_enabled.lock().unwrap();
    *mix_minus = enabled;

    log::info!("Mix-minus: {}", if enabled { "enabled" } else { "disabled" });

    Ok(())
}






