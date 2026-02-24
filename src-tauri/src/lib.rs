pub mod analytics;
pub mod audio;
pub mod commands;
pub mod db;
pub mod gateway;
pub mod scheduler;
pub mod scripting;
pub mod state;
pub mod stats;
pub mod stream;

use commands::{
    analytics_commands::{
        clear_event_log, export_report_csv, generate_report, get_event_log,
        get_health_history, get_health_snapshot, get_hourly_heatmap, get_listener_graph,
        get_listener_peak, get_song_play_history, get_top_songs,
    },
    audio_commands::{
        get_deck_state, get_vu_readings, load_track, pause_deck, play_deck, seek_deck,
        set_channel_gain,
    },
    crossfade_commands::{
        get_crossfade_config, get_fade_curve_preview, set_crossfade_config, start_crossfade,
    },
    cue_commands::{delete_cue_point, get_cue_points, jump_to_cue, set_cue_point},
    dsp_commands::{get_channel_dsp, set_channel_agc, set_channel_eq, set_pipeline_settings},
    encoder_commands::{
        delete_encoder, get_current_listeners, get_encoder_runtime, get_encoders,
        get_listener_stats, push_track_metadata, save_encoder, start_all_encoders,
        start_encoder, start_recording, stop_all_encoders, stop_encoder, stop_recording,
        test_encoder_connection,
    },
    gateway_commands::{
        connect_gateway, disconnect_gateway, get_autopilot_status, get_gateway_status,
        get_remote_dj_permissions, get_remote_sessions, kick_remote_dj,
        set_autopilot, set_mix_minus, set_remote_dj_permissions, start_live_talk, stop_live_talk,
    },
    mic_commands::{
        get_audio_input_devices, get_mic_config, set_mic_config,
        start_mic, stop_mic, set_ptt,
        start_voice_recording, stop_voice_recording, save_voice_track,
    },
    queue_commands::{
        add_to_queue, complete_queue_item, get_history, get_queue, remove_from_queue,
        search_songs,
    },
    sam_db_commands::{
        connect_sam_db, disconnect_sam_db, get_sam_categories, get_sam_db_config_cmd,
        get_sam_db_status, save_sam_db_config_cmd, test_sam_db_connection,
    },
    script_commands::{get_scripts, save_script, delete_script, run_script, get_script_log},
    stream_commands::{get_stream_status, start_stream, stop_stream},
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine = audio::engine::AudioEngine::new()
        .expect("Failed to initialise audio engine");

    // ── Database initialisation ──────────────────────────────────────────────
    //
    // We need both DBs ready before commands start serving requests.
    // Use a short-lived single-thread Tokio runtime for the async init work;
    // Tauri will create its own multi-thread runtime afterward.

    let app_data_dir = compute_app_data_dir();
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
    let db_path = format!("{app_data_dir}/app.db");

    let (local_pool, sam_pool_opt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build init Tokio runtime")
        .block_on(async {
            // 1. SQLite (always required)
            let local = db::local::init_db(&db_path)
                .await
                .expect("Failed to open local SQLite database");

            // 2. SAM MySQL — attempt auto-connect if configured
            let sam_opt = match db::local::load_sam_db_config_full(&local).await {
                Ok(Some(cfg)) if cfg.config.auto_connect => {
                    let enc_pw = urlencoding::encode(&cfg.password);
                    let url = format!(
                        "mysql://{}:{}@{}:{}/{}",
                        cfg.config.username,
                        enc_pw,
                        cfg.config.host,
                        cfg.config.port,
                        cfg.config.database_name,
                    );
                    match db::sam::connect(&url).await {
                        Ok(pool) => {
                            eprintln!(
                                "[startup] SAM DB auto-connected → {}:{}",
                                cfg.config.host, cfg.config.database_name
                            );
                            Some(pool)
                        }
                        Err(e) => {
                            // Non-fatal — app works without SAM DB
                            eprintln!("[startup] SAM DB auto-connect failed (continuing): {e}");
                            None
                        }
                    }
                }
                _ => None,
            };

            (local, sam_opt)
        });

    // ── AppState assembly ────────────────────────────────────────────────────
    let mut app_state = AppState::new(engine).with_local_db(local_pool);
    if let Some(pool) = sam_pool_opt {
        app_state = app_state.with_sam_db(pool);
    }

    // ── Tauri app ────────────────────────────────────────────────────────────
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Phase 1 — Deck control
            load_track,
            play_deck,
            pause_deck,
            seek_deck,
            set_channel_gain,
            get_deck_state,
            get_vu_readings,
            // Phase 1 — Crossfade
            get_crossfade_config,
            set_crossfade_config,
            start_crossfade,
            get_fade_curve_preview,
            // Phase 1 — DSP
            get_channel_dsp,
            set_channel_eq,
            set_channel_agc,
            set_pipeline_settings,
            // Phase 1 — Cue points
            get_cue_points,
            set_cue_point,
            delete_cue_point,
            jump_to_cue,
            // Phase 1 — Queue / SAM
            get_queue,
            add_to_queue,
            remove_from_queue,
            complete_queue_item,
            search_songs,
            get_history,
            // Phase 1 — Single legacy stream
            start_stream,
            stop_stream,
            get_stream_status,
            // Phase 4 — Multi-encoder
            get_encoders,
            save_encoder,
            delete_encoder,
            start_encoder,
            stop_encoder,
            start_all_encoders,
            stop_all_encoders,
            test_encoder_connection,
            get_encoder_runtime,
            // Phase 4 — Recording
            start_recording,
            stop_recording,
            // Phase 4 — Stats
            get_listener_stats,
            get_current_listeners,
            // Phase 4 — Metadata
            push_track_metadata,
            // Phase 5 — Scripts
            get_scripts,
            save_script,
            delete_script,
            run_script,
            get_script_log,
            // Phase 5 — Microphone / Voice FX
            get_audio_input_devices,
            get_mic_config,
            set_mic_config,
            start_mic,
            stop_mic,
            set_ptt,
            // Phase 5 — Voice Track Recording
            start_voice_recording,
            stop_voice_recording,
            save_voice_track,
            // Phase 6 — Gateway
            connect_gateway,
            disconnect_gateway,
            get_gateway_status,
            set_autopilot,
            get_autopilot_status,
            get_remote_sessions,
            kick_remote_dj,
            set_remote_dj_permissions,
            get_remote_dj_permissions,
            start_live_talk,
            stop_live_talk,
            set_mix_minus,
            // Phase 6 — SAM DB connection management
            test_sam_db_connection,
            connect_sam_db,
            disconnect_sam_db,
            get_sam_db_config_cmd,
            save_sam_db_config_cmd,
            get_sam_db_status,
            get_sam_categories,
            // Phase 7 — Analytics
            get_top_songs,
            get_hourly_heatmap,
            get_song_play_history,
            get_listener_graph,
            get_listener_peak,
            get_event_log,
            clear_event_log,
            get_health_snapshot,
            get_health_history,
            generate_report,
            export_report_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Return the platform-specific application data directory.
/// Mirrors what Tauri resolves for `PathResolver::app_data_dir()`.
fn compute_app_data_dir() -> String {
    const IDENTIFIER: &str = "com.minhaj.desizonebroadcaster";

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/Library/Application Support/{IDENTIFIER}")
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        format!("{appdata}\\{IDENTIFIER}")
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.config/{IDENTIFIER}")
    }
}
