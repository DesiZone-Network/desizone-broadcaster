use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    db::{
        local::{get_sam_db_config, save_sam_db_config, SamDbConfig},
        sam::{connect, create_category, get_categories, SamCategory},
    },
    state::AppState,
};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamDbConnectArgs {
    pub host: String,
    pub port: i64,
    pub username: String,
    pub password: String,
    pub database: String,
    pub auto_connect: bool,
    pub path_prefix_from: Option<String>,
    pub path_prefix_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamDbStatus {
    pub connected: bool,
    pub host: Option<String>,
    pub database: Option<String>,
    pub error: Option<String>,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Test a SAM DB connection without saving or storing it.
#[tauri::command]
pub async fn test_sam_db_connection(args: SamDbConnectArgs) -> Result<SamDbStatus, String> {
    let url = build_mysql_url(
        &args.host,
        args.port,
        &args.username,
        &args.password,
        &args.database,
    );
    match connect(&url).await {
        Ok(pool) => {
            pool.close().await;
            Ok(SamDbStatus {
                connected: true,
                host: Some(args.host),
                database: Some(args.database),
                error: None,
            })
        }
        Err(e) => Ok(SamDbStatus {
            connected: false,
            host: Some(args.host),
            database: Some(args.database),
            error: Some(e.to_string()),
        }),
    }
}

/// Connect to a SAM Broadcaster MySQL database, save the config to SQLite,
/// and store the live pool in AppState.
#[tauri::command]
pub async fn connect_sam_db(
    args: SamDbConnectArgs,
    state: State<'_, AppState>,
) -> Result<SamDbStatus, String> {
    let url = build_mysql_url(
        &args.host,
        args.port,
        &args.username,
        &args.password,
        &args.database,
    );

    let pool = connect(&url)
        .await
        .map_err(|e| format!("SAM DB connect failed: {e}"))?;

    // Store pool in AppState
    *state.sam_db.write().await = Some(pool);

    // Persist config (including password) to local SQLite
    if let Some(local) = &state.local_db {
        let cfg = SamDbConfig {
            host: args.host.clone(),
            port: args.port,
            username: args.username.clone(),
            database_name: args.database.clone(),
            // Reliability-first behavior: once a connection succeeds, always
            // persist auto-connect enabled for the next app launch.
            auto_connect: true,
            path_prefix_from: args.path_prefix_from.clone().unwrap_or_default(),
            path_prefix_to: args.path_prefix_to.clone().unwrap_or_default(),
        };
        save_sam_db_config(local, &cfg, &args.password)
            .await
            .map_err(|e| format!("Failed to save SAM DB config: {e}"))?;
    }

    Ok(SamDbStatus {
        connected: true,
        host: Some(args.host),
        database: Some(args.database),
        error: None,
    })
}

/// Disconnect from SAM DB and drop the pool.
#[tauri::command]
pub async fn disconnect_sam_db(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.sam_db.write().await;
    if let Some(pool) = guard.take() {
        pool.close().await;
    }
    Ok(())
}

/// Return the saved SAM DB config (no password).
#[tauri::command]
pub async fn get_sam_db_config_cmd(state: State<'_, AppState>) -> Result<SamDbConfig, String> {
    let local = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    get_sam_db_config(local)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

/// Save connection config to SQLite without actually connecting.
/// Useful for pre-configuring auto_connect before next launch.
#[tauri::command]
pub async fn save_sam_db_config_cmd(
    config: SamDbConfig,
    password: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let local = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    save_sam_db_config(local, &config, &password)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

/// Return live connection status.
#[tauri::command]
pub async fn get_sam_db_status(state: State<'_, AppState>) -> Result<SamDbStatus, String> {
    let guard = state.sam_db.read().await;
    if guard.is_some() {
        // Load saved config to show host/database info (no password)
        let (host, database) = if let Some(local) = &state.local_db {
            match get_sam_db_config(local).await {
                Ok(cfg) => (Some(cfg.host), Some(cfg.database_name)),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };
        Ok(SamDbStatus {
            connected: true,
            host,
            database,
            error: None,
        })
    } else {
        Ok(SamDbStatus {
            connected: false,
            host: None,
            database: None,
            error: None,
        })
    }
}

/// Return SAM categories.  Empty Vec if catlist table doesn't exist.
#[tauri::command]
pub async fn get_sam_categories(state: State<'_, AppState>) -> Result<Vec<SamCategory>, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    get_categories(pool)
        .await
        .map_err(|e| format!("SAM DB error: {e}"))
}

#[tauri::command]
pub async fn create_sam_category(
    name: String,
    parent_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<SamCategory, String> {
    let guard = state.sam_db.read().await;
    let pool = guard.as_ref().ok_or("SAM DB not connected")?;
    create_category(pool, &name, parent_id).await
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_mysql_url(host: &str, port: i64, user: &str, password: &str, database: &str) -> String {
    // URL-encode the password to handle special characters
    let enc_password = urlencoding::encode(password);
    format!("mysql://{user}:{enc_password}@{host}:{port}/{database}")
}
