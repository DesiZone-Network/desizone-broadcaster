use tauri::State;

use crate::{
    controller::types::{ControllerConfig, ControllerDevice, ControllerStatus},
    db::local::{
        get_controller_config as db_get_controller_config,
        save_controller_config as db_save_controller_config, ControllerConfigRow,
    },
    state::AppState,
};

fn to_public_config(row: ControllerConfigRow) -> ControllerConfig {
    ControllerConfig {
        enabled: row.enabled,
        auto_connect: row.auto_connect,
        preferred_device_id: row.preferred_device_id,
        profile: row.profile,
    }
}

fn to_row(config: &ControllerConfig) -> ControllerConfigRow {
    ControllerConfigRow {
        enabled: config.enabled,
        auto_connect: config.auto_connect,
        preferred_device_id: config.preferred_device_id.clone(),
        profile: config.profile.clone(),
    }
}

#[tauri::command]
pub async fn list_controller_devices(
    state: State<'_, AppState>,
) -> Result<Vec<ControllerDevice>, String> {
    state.controller_service.list_devices()
}

#[tauri::command]
pub async fn get_controller_status(state: State<'_, AppState>) -> Result<ControllerStatus, String> {
    Ok(state.controller_service.get_status())
}

#[tauri::command]
pub async fn get_controller_config(state: State<'_, AppState>) -> Result<ControllerConfig, String> {
    if let Some(pool) = &state.local_db {
        let row = db_get_controller_config(pool)
            .await
            .map_err(|e| format!("DB error: {e}"))?;
        Ok(to_public_config(row))
    } else {
        Ok(state.controller_service.get_config())
    }
}

#[tauri::command]
pub async fn save_controller_config_cmd(
    config: ControllerConfig,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if let Some(pool) = &state.local_db {
        db_save_controller_config(pool, &to_row(&config))
            .await
            .map_err(|e| format!("DB error: {e}"))?;
    }

    state
        .controller_service
        .set_config(config.clone(), Some(&app));

    if config.enabled && config.auto_connect {
        let _ = state.controller_service.connect(None, &app);
    } else if !config.enabled {
        let _ = state.controller_service.disconnect(&app);
    }
    Ok(())
}

#[tauri::command]
pub async fn connect_controller(
    device_id: Option<String>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ControllerStatus, String> {
    state.controller_service.connect(device_id, &app)
}

#[tauri::command]
pub async fn disconnect_controller(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ControllerStatus, String> {
    state.controller_service.disconnect(&app)
}
