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
    queue_commands::{add_to_queue, get_history, get_queue, remove_from_queue, search_songs},
    script_commands::{get_scripts, save_script, delete_script, run_script, get_script_log},
    stream_commands::{get_stream_status, start_stream, stop_stream},
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine = audio::engine::AudioEngine::new()
        .expect("Failed to initialise audio engine");

    let app_state = AppState::new(engine);

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

