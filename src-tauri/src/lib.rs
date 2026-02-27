pub mod analytics;
pub mod audio;
pub mod commands;
pub mod controller;
pub mod db;
pub mod gateway;
pub mod scheduler;
pub mod scripting;
pub mod state;
pub mod stats;
pub mod stream;

use commands::{
    analytics_commands::{
        clear_event_log, export_report_csv, generate_report, get_event_log, get_health_history,
        get_health_snapshot, get_hourly_heatmap, get_listener_graph, get_listener_peak,
        get_song_play_history, get_top_songs, write_event_log,
    },
    audio_commands::{
        clear_deck_loop, get_deck_state, get_master_level, get_vu_readings, jog_deck, load_track,
        next_deck, pause_deck, play_deck, seek_deck, set_channel_gain, set_deck_bass,
        set_deck_filter, set_deck_loop, set_deck_pitch, set_deck_tempo, set_master_level,
        stop_deck,
    },
    beatgrid_commands::{analyze_beatgrid, get_beatgrid},
    controller_commands::{
        connect_controller, disconnect_controller, get_controller_config,
        get_controller_status, list_controller_devices, save_controller_config_cmd,
    },
    crossfade_commands::{
        get_crossfade_config, get_fade_curve_preview, set_crossfade_config, set_manual_crossfade,
        start_crossfade, trigger_manual_fade,
    },
    cue_commands::{
        clear_hot_cue, delete_cue_point, get_cue_points, get_hot_cues, get_monitor_routing_config,
        jump_to_cue, recolor_hot_cue, rename_hot_cue, set_cue_point, set_deck_cue_preview_enabled,
        set_hot_cue, set_monitor_routing_config, trigger_hot_cue,
    },
    dsp_commands::{
        get_channel_dsp, set_channel_agc, set_channel_eq, set_channel_stem_filter,
        set_pipeline_settings,
    },
    encoder_commands::{
        delete_encoder, get_current_listeners, get_encoder_runtime, get_encoders,
        get_listener_stats, push_track_metadata, save_encoder, start_all_encoders, start_encoder,
        start_recording, stop_all_encoders, stop_encoder, stop_recording, test_encoder_connection,
    },
    gateway_commands::{
        connect_gateway, disconnect_gateway, get_autopilot_status, get_gateway_status,
        get_remote_dj_permissions, get_remote_sessions, kick_remote_dj, set_autopilot,
        set_mix_minus, set_remote_dj_permissions, start_live_talk, stop_live_talk,
    },
    mic_commands::{
        get_audio_input_devices, get_mic_config, save_voice_track, set_mic_config, set_ptt,
        start_mic, start_voice_recording, stop_mic, stop_voice_recording,
    },
    queue_commands::{
        add_to_queue, complete_queue_item, get_history, get_queue, get_song, get_song_types,
        get_songs_by_weight_range, get_songs_in_category, remove_from_queue, reorder_queue,
        search_songs, update_song,
    },
    sam_db_commands::{
        connect_sam_db, create_sam_category, disconnect_sam_db, get_sam_categories,
        get_sam_db_config_cmd, get_sam_db_status, save_sam_db_config_cmd, test_sam_db_connection,
    },
    scheduler_commands::{
        accept_request_p3, delete_rotation_rule, delete_show, enqueue_next_clockwheel_track,
        get_autodj_transition_config, get_clockwheel_config, get_dj_mode, get_gap_killer_config,
        get_last_transition_decision, get_next_autodj_track, get_pending_requests, get_playlists,
        get_request_history, get_request_policy, get_rotation_rules, get_shows,
        get_song_directories, get_upcoming_events, recalculate_autodj_plan_now, reject_request_p3,
        save_clockwheel_config, save_playlist, save_rotation_rule, save_show, set_active_playlist,
        set_autodj_transition_config, set_dj_mode, set_gap_killer_config, set_request_policy,
    },
    script_commands::{delete_script, get_script_log, get_scripts, run_script, save_script},
    stem_commands::{
        analyze_stems, get_latest_stem_analysis, get_stem_analysis, get_stems_runtime_status,
        install_stems_runtime, set_deck_stem_source,
    },
    stream_commands::{get_stream_status, start_stream, stop_stream},
    waveform_commands::get_waveform_data,
};
use state::AppState;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine = audio::engine::AudioEngine::new().expect("Failed to initialise audio engine");

    // ── Database initialisation ──────────────────────────────────────────────
    //
    // We need both DBs ready before commands start serving requests.
    // Use a short-lived single-thread Tokio runtime for the async init work;
    // Tauri will create its own multi-thread runtime afterward.

    let app_data_dir = compute_app_data_dir();
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
    let db_path = format!("{app_data_dir}/app.db");

    let (
        local_pool,
        sam_pool_opt,
        startup_crossfade_cfg,
        startup_autodj_cfg,
        startup_monitor_cfg,
        startup_controller_cfg,
    ) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build init Tokio runtime")
        .block_on(async {
                // 1. SQLite (always required)
                let local = db::local::init_db(&db_path)
                    .await
                    .expect("Failed to open local SQLite database");

                // Load persisted DJ mode into runtime state at startup.
                if let Ok(saved_mode) = db::local::get_runtime_dj_mode(&local).await {
                    let mode = crate::scheduler::autodj::DjMode::from_str(&saved_mode);
                    crate::scheduler::autodj::set_dj_mode(mode);
                }
                let mut startup_autodj_cfg: Option<crate::scheduler::autodj::AutoTransitionConfig> =
                    None;
                if let Ok(Some(json)) = db::local::load_autodj_transition_config(&local).await {
                    let cfg =
                        crate::commands::scheduler_commands::parse_autodj_transition_config_json(
                            &json,
                        );
                    crate::scheduler::autodj::set_auto_transition_config(cfg.clone());
                    startup_autodj_cfg = Some(cfg);
                }

                let mut startup_crossfade_cfg: Option<crate::audio::crossfade::CrossfadeConfig> =
                    None;
                if let Ok(Some(json)) = db::local::load_crossfade_config(&local).await {
                    let cfg =
                        crate::commands::crossfade_commands::parse_crossfade_config_json(&json);
                    startup_crossfade_cfg = Some(cfg);
                }
                let startup_monitor_cfg = db::local::get_monitor_routing_config(&local).await.ok();
                let startup_controller_cfg =
                    db::local::get_controller_config(&local).await.ok().map(|cfg| {
                        crate::controller::types::ControllerConfig {
                            enabled: cfg.enabled,
                            auto_connect: cfg.auto_connect,
                            preferred_device_id: cfg.preferred_device_id,
                            profile: cfg.profile,
                        }
                    });

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

                (
                    local,
                    sam_opt,
                    startup_crossfade_cfg,
                    startup_autodj_cfg,
                    startup_monitor_cfg,
                    startup_controller_cfg,
                )
            });

    // ── AppState assembly ────────────────────────────────────────────────────
    let mut app_state = AppState::new(engine).with_local_db(local_pool);
    if let Some(cfg) = startup_crossfade_cfg {
        let _ = app_state.engine.lock().unwrap().set_crossfade_config(cfg);
    }
    if let Some(cfg) = startup_autodj_cfg {
        crate::scheduler::autodj::set_auto_transition_config(cfg);
    }
    if let Some(cfg) = startup_monitor_cfg {
        app_state
            .engine
            .lock()
            .unwrap()
            .set_monitor_routing_config(cfg);
    }
    if let Some(cfg) = startup_controller_cfg {
        app_state.controller_service.set_config(cfg, None);
    }
    if let Some(pool) = sam_pool_opt {
        app_state = app_state.with_sam_db(pool);
    }

    // ── Tauri app ────────────────────────────────────────────────────────────
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .setup(|app| {
            // Force main window visible/focused in dev. This avoids cases where
            // the process is running but the webview starts hidden/off-screen.
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.show();
                let _ = main_window.unminimize();
                let _ = main_window.center();
                let _ = main_window.set_focus();
            }

            {
                let state = app.state::<AppState>();
                state
                    .controller_service
                    .start_background(app.handle().clone());
                let cfg = state.controller_service.get_config();
                if cfg.enabled && cfg.auto_connect {
                    let _ = state.controller_service.connect(None, &app.handle().clone());
                } else {
                    let _ = app
                        .handle()
                        .emit("controller_status_changed", state.controller_service.get_status());
                }
            }

            // ── Background polling loop ──────────────────────────────────────
            // Emits `deck_state_changed` (every 80 ms) and `vu_meter` events
            // to the frontend, since the audio engine is poll-based (no push).
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use crate::audio::crossfade::DeckId;
                use std::time::Duration;
                use tauri::{Emitter, Manager};

                let state = app_handle.state::<AppState>();
                let mut interval = tokio::time::interval(Duration::from_millis(80));
                let mut last_manual_crossfade_pos: Option<f32> = None;
                let mut last_master_level: Option<f32> = None;

                loop {
                    interval.tick().await;

                    // Collect data while holding the engine lock briefly,
                    // then release it before emitting (avoid holding across await).
                    let (deck_events, vu_events, crossfade_event, manual_crossfade_pos, master_level) = {
                        let engine = state.engine.lock().unwrap();
                        let deck_events: Vec<_> = [
                            DeckId::DeckA,
                            DeckId::DeckB,
                            DeckId::SoundFx,
                            DeckId::Aux1,
                            DeckId::Aux2,
                            DeckId::VoiceFx,
                        ]
                        .into_iter()
                        .filter_map(|id| engine.get_deck_state(id))
                        .collect();
                        let vu_events = engine.get_vu_readings();
                        let crossfade_event = engine.get_crossfade_progress_event();
                        let manual_crossfade_pos = engine.get_manual_crossfade_pos();
                        let master_level = engine.get_master_level();
                        (
                            deck_events,
                            vu_events,
                            crossfade_event,
                            manual_crossfade_pos,
                            master_level,
                        )
                    };

                    for ev in &deck_events {
                        let _ = app_handle.emit("deck_state_changed", ev);
                    }
                    for ev in &vu_events {
                        let _ = app_handle.emit("vu_meter", ev);
                    }
                    if let Some(ev) = &crossfade_event {
                        let _ = app_handle.emit("crossfade_progress", ev);
                    }
                    let should_emit_manual = last_manual_crossfade_pos
                        .map(|prev| (prev - manual_crossfade_pos).abs() > 0.001)
                        .unwrap_or(true);
                    if should_emit_manual {
                        last_manual_crossfade_pos = Some(manual_crossfade_pos);
                        let _ = app_handle.emit(
                            "manual_crossfade_changed",
                            serde_json::json!({ "position": manual_crossfade_pos }),
                        );
                    }
                    let should_emit_master = last_master_level
                        .map(|prev| (prev - master_level).abs() > 0.001)
                        .unwrap_or(true);
                    if should_emit_master {
                        last_master_level = Some(master_level);
                        let _ = app_handle.emit(
                            "master_volume_changed",
                            serde_json::json!({ "level": master_level }),
                        );
                    }
                }
            });

            // ── AutoDJ runtime loop ────────────────────────────────────────
            // Keeps queue/rotation playback moving for assisted/autodj modes.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use crate::audio::crossfade::{CrossfadeTriggerMode, DeckId};
                use crate::scheduler::autodj::{
                    self, AutodjTransitionEngine, DjMode, TransitionDecisionDebug,
                };
                use crate::scheduler::transition_planner::{
                    calculate_transition_plan, DeckSnapshot, TransitionPlan,
                };
                use std::collections::{HashMap, HashSet};
                use std::time::{Duration, Instant};
                use tauri::Manager;

                let state = app_handle.state::<AppState>();
                let mut interval = tokio::time::interval(Duration::from_millis(100));
                let mut marker_cache: HashMap<
                    i64,
                    crate::scheduler::transition_planner::TransitionMarkers,
                > = HashMap::new();
                let mut pending_gap: Option<PendingGapTransition> = None;
                let mut pending_sam_start: Option<PendingSamTransition> = None;
                let mut sam_below_threshold_since: HashMap<DeckId, std::time::Instant> =
                    HashMap::new();
                let mut claimed_queue_ids: HashSet<i64> = HashSet::new();
                let mut last_queue_topup_at = Instant::now()
                    .checked_sub(Duration::from_secs(5))
                    .unwrap_or_else(Instant::now);
                const SAM_HOLD_MS: u32 = 120;
                const SAM_PREROLL_MIN_MS: u64 = 150;
                const SAM_PREROLL_TIMEOUT_MS: u64 = 800;
                const SAM_RELEASE_HYST_DB: f32 = 0.5;
                const SAM_RECUE_NEAR_END_MS: u64 = 1000;

                loop {
                    interval.tick().await;

                    if crate::scheduler::autodj::take_replan_requested() {
                        marker_cache.clear();
                        pending_gap = None;
                        pending_sam_start = None;
                        sam_below_threshold_since.clear();
                    }

                    // Handle completed tracks (EOF) for queue/history bookkeeping
                    // in all modes (including manual).
                    let completed = { state.engine.lock().unwrap().take_track_completions() };
                    if !completed.is_empty() {
                        let completed_queue_ids =
                            process_track_completions(&state, completed).await;
                        for queue_id in completed_queue_ids {
                            claimed_queue_ids.remove(&queue_id);
                        }
                    }

                    let mode = crate::scheduler::autodj::get_dj_mode();
                    if mode == DjMode::Manual {
                        continue;
                    }

                    if mode == DjMode::AutoDj
                        && last_queue_topup_at.elapsed() >= Duration::from_secs(1)
                    {
                        top_up_rotation_queue(&state, &claimed_queue_ids).await;
                        last_queue_topup_at = Instant::now();
                    }

                    let (a, b, crossfade_active): (
                        Option<crate::audio::engine::DeckStateEvent>,
                        Option<crate::audio::engine::DeckStateEvent>,
                        bool,
                    ) = {
                        let engine = state.engine.lock().unwrap();
                        (
                            engine.get_deck_state(DeckId::DeckA),
                            engine.get_deck_state(DeckId::DeckB),
                            engine.get_crossfade_progress_event().is_some(),
                        )
                    };

                    let is_playing = |s: &str| matches!(s, "playing" | "crossfading");
                    let is_ready = |s: &str| matches!(s, "ready" | "paused");
                    let is_idleish = |s: &str| matches!(s, "idle" | "stopped");

                    let a_state = a
                        .as_ref()
                        .map(|d: &crate::audio::engine::DeckStateEvent| d.state.as_str())
                        .unwrap_or("idle");
                    let b_state = b
                        .as_ref()
                        .map(|d: &crate::audio::engine::DeckStateEvent| d.state.as_str())
                        .unwrap_or("idle");

                    let a_playing = is_playing(a_state);
                    let b_playing = is_playing(b_state);
                    let no_playing = !a_playing && !b_playing;

                    if let Some(gap) = pending_gap.clone() {
                        if std::time::Instant::now() >= gap.start_at {
                            let side = if gap.incoming == DeckId::DeckB {
                                1.0
                            } else {
                                -1.0
                            };
                            let mut engine = state.engine.lock().unwrap();
                            let _ = engine.set_manual_crossfade(side);
                            let _ = engine.play(gap.incoming);
                            pending_gap = None;
                        }
                        continue;
                    }

                    if no_playing {
                        pending_sam_start = None;
                        sam_below_threshold_since.clear();
                        if mode == DjMode::AutoDj {
                            if is_ready(a_state) {
                                let mut engine = state.engine.lock().unwrap();
                                let _ = engine.set_manual_crossfade(-1.0);
                                let _ = engine.play(DeckId::DeckA);
                                continue;
                            }
                            if is_ready(b_state) {
                                let mut engine = state.engine.lock().unwrap();
                                let _ = engine.set_manual_crossfade(1.0);
                                let _ = engine.play(DeckId::DeckB);
                                continue;
                            }
                            if let Some(next) =
                                pick_next_track(&state, mode, &claimed_queue_ids).await
                            {
                                let queue_to_claim = next.queue_id;
                                let loaded = {
                                    let mut engine = state.engine.lock().unwrap();
                                    engine
                                        .load_track_with_source(
                                            DeckId::DeckA,
                                            std::path::PathBuf::from(&next.file_path),
                                            Some(next.song_id),
                                            next.queue_id,
                                            next.from_rotation,
                                            next.declared_duration_ms,
                                        )
                                        .is_ok()
                                };
                                if loaded {
                                    if let Some(qid) = next.queue_id {
                                        claimed_queue_ids.insert(qid);
                                        claim_queue_item(&state, qid).await;
                                    }
                                    let mut engine = state.engine.lock().unwrap();
                                    let _ = engine.set_manual_crossfade(-1.0);
                                    let _ = engine.play(DeckId::DeckA);
                                } else if let Some(qid) = queue_to_claim {
                                    claimed_queue_ids.remove(&qid);
                                }
                            }
                        }
                        continue;
                    }

                    if crossfade_active {
                        pending_sam_start = None;
                        continue;
                    }

                    if let Some(pending) = pending_sam_start.clone() {
                        let from_ev = event_for_deck(&a, &b, pending.from);
                        let to_ev = event_for_deck(&a, &b, pending.to);
                        let from_valid = from_ev
                            .map(|ev| is_playing(ev.state.as_str()))
                            .unwrap_or(false);
                        let to_valid = to_ev.map(|ev| is_ready(ev.state.as_str())).unwrap_or(false);
                        if !from_valid || !to_valid {
                            autodj::set_last_transition_decision(TransitionDecisionDebug {
                                engine: "sam_classic".to_string(),
                                from_deck: Some(pending.from.to_string()),
                                to_deck: Some(pending.to.to_string()),
                                trigger_mode: Some(pending.trigger_mode.clone()),
                                reason: "pending_cancelled_state_changed".to_string(),
                                outgoing_rms_db: from_ev.map(|ev| ev.rms_db_pre_fader),
                                threshold_db: None,
                                outgoing_remaining_ms: from_ev
                                    .map(|ev| ev.duration_ms.saturating_sub(ev.position_ms)),
                                fixed_point_ms: None,
                                hold_ms: Some(SAM_HOLD_MS),
                                skip_cause: None,
                            });
                            pending_sam_start = None;
                            continue;
                        }

                        let incoming_near_end = to_ev
                            .map(|ev| {
                                ev.duration_ms > 0
                                    && ev.position_ms
                                        >= ev.duration_ms.saturating_sub(SAM_RECUE_NEAR_END_MS)
                            })
                            .unwrap_or(false);
                        if incoming_near_end {
                            let mut engine = state.engine.lock().unwrap();
                            let _ = engine.seek(pending.to, 0);
                        }
                        let incoming_buffer_ms = to_ev.map(|ev| ev.decoder_buffer_ms).unwrap_or(0);
                        if incoming_buffer_ms >= SAM_PREROLL_MIN_MS {
                            let mut engine = state.engine.lock().unwrap();
                            let _ = start_sam_transition(
                                &mut engine,
                                pending.from,
                                pending.to,
                                pending.fade_ms,
                            );
                            autodj::set_last_transition_decision(TransitionDecisionDebug {
                                engine: "sam_classic".to_string(),
                                from_deck: Some(pending.from.to_string()),
                                to_deck: Some(pending.to.to_string()),
                                trigger_mode: Some(pending.trigger_mode.clone()),
                                reason: "started_after_preroll".to_string(),
                                outgoing_rms_db: from_ev.map(|ev| ev.rms_db_pre_fader),
                                threshold_db: None,
                                outgoing_remaining_ms: from_ev
                                    .map(|ev| ev.duration_ms.saturating_sub(ev.position_ms)),
                                fixed_point_ms: None,
                                hold_ms: Some(SAM_HOLD_MS),
                                skip_cause: pending
                                    .short_track_fallback
                                    .then_some("short_track".to_string()),
                            });
                            pending_sam_start = None;
                        } else if pending.requested_at.elapsed()
                            >= Duration::from_millis(SAM_PREROLL_TIMEOUT_MS)
                        {
                            let timeout_fade_ms = pending.fade_ms.min(250).max(120);
                            let mut engine = state.engine.lock().unwrap();
                            let _ = start_sam_transition(
                                &mut engine,
                                pending.from,
                                pending.to,
                                timeout_fade_ms,
                            );
                            autodj::set_last_transition_decision(TransitionDecisionDebug {
                                engine: "sam_classic".to_string(),
                                from_deck: Some(pending.from.to_string()),
                                to_deck: Some(pending.to.to_string()),
                                trigger_mode: Some(pending.trigger_mode.clone()),
                                reason: "preroll_timeout_fallback".to_string(),
                                outgoing_rms_db: from_ev.map(|ev| ev.rms_db_pre_fader),
                                threshold_db: None,
                                outgoing_remaining_ms: from_ev
                                    .map(|ev| ev.duration_ms.saturating_sub(ev.position_ms)),
                                fixed_point_ms: None,
                                hold_ms: Some(SAM_HOLD_MS),
                                skip_cause: Some("incoming_preroll_timeout".to_string()),
                            });
                            pending_sam_start = None;
                        } else {
                            autodj::set_last_transition_decision(TransitionDecisionDebug {
                                engine: "sam_classic".to_string(),
                                from_deck: Some(pending.from.to_string()),
                                to_deck: Some(pending.to.to_string()),
                                trigger_mode: Some(pending.trigger_mode),
                                reason: "waiting_incoming_preroll".to_string(),
                                outgoing_rms_db: from_ev.map(|ev| ev.rms_db_pre_fader),
                                threshold_db: None,
                                outgoing_remaining_ms: from_ev
                                    .map(|ev| ev.duration_ms.saturating_sub(ev.position_ms)),
                                fixed_point_ms: None,
                                hold_ms: Some(SAM_HOLD_MS),
                                skip_cause: None,
                            });
                        }
                        continue;
                    }

                    // Preload next track on the idle deck before crossfade window.
                    // Explicit preload window request: preload next deck when
                    // active deck has 25 seconds or less remaining.
                    let preload_ms = 25_000_u64;
                    if a_playing && is_idleish(b_state) {
                        let rem = a
                            .as_ref()
                            .map(|d| d.duration_ms.saturating_sub(d.position_ms))
                            .unwrap_or(0);
                        if rem > 0 && rem <= preload_ms {
                            if let Some(next) =
                                pick_next_track(&state, mode, &claimed_queue_ids).await
                            {
                                let queue_to_claim = next.queue_id;
                                let loaded = state
                                    .engine
                                    .lock()
                                    .unwrap()
                                    .load_track_with_source(
                                        DeckId::DeckB,
                                        std::path::PathBuf::from(&next.file_path),
                                        Some(next.song_id),
                                        next.queue_id,
                                        next.from_rotation,
                                        next.declared_duration_ms,
                                    )
                                    .is_ok();
                                if loaded {
                                    if let Some(qid) = next.queue_id {
                                        claimed_queue_ids.insert(qid);
                                        claim_queue_item(&state, qid).await;
                                    }
                                } else if let Some(qid) = queue_to_claim {
                                    claimed_queue_ids.remove(&qid);
                                }
                            }
                        }
                    } else if b_playing && is_idleish(a_state) {
                        let rem = b
                            .as_ref()
                            .map(|d| d.duration_ms.saturating_sub(d.position_ms))
                            .unwrap_or(0);
                        if rem > 0 && rem <= preload_ms {
                            if let Some(next) =
                                pick_next_track(&state, mode, &claimed_queue_ids).await
                            {
                                let queue_to_claim = next.queue_id;
                                let loaded = state
                                    .engine
                                    .lock()
                                    .unwrap()
                                    .load_track_with_source(
                                        DeckId::DeckA,
                                        std::path::PathBuf::from(&next.file_path),
                                        Some(next.song_id),
                                        next.queue_id,
                                        next.from_rotation,
                                        next.declared_duration_ms,
                                    )
                                    .is_ok();
                                if loaded {
                                    if let Some(qid) = next.queue_id {
                                        claimed_queue_ids.insert(qid);
                                        claim_queue_item(&state, qid).await;
                                    }
                                } else if let Some(qid) = queue_to_claim {
                                    claimed_queue_ids.remove(&qid);
                                }
                            }
                        }
                    }

                    if mode != DjMode::AutoDj {
                        continue;
                    }

                    let autodj_cfg = autodj::get_auto_transition_config();
                    match autodj_cfg.engine {
                        AutodjTransitionEngine::SamClassic => {
                            let maybe_from_to = if a_playing && is_ready(b_state) {
                                Some((a.as_ref(), b.as_ref()))
                            } else if b_playing && is_ready(a_state) {
                                Some((b.as_ref(), a.as_ref()))
                            } else {
                                None
                            };
                            let Some((Some(from_ev), Some(to_ev))) = maybe_from_to else {
                                sam_below_threshold_since.clear();
                                continue;
                            };
                            let Some(from_deck) = deck_id_from_event(from_ev) else {
                                continue;
                            };
                            let Some(to_deck) = deck_id_from_event(to_ev) else {
                                continue;
                            };
                            sam_below_threshold_since.retain(|deck, _| *deck == from_deck);

                            let crossfade_cfg = {
                                let engine = state.engine.lock().unwrap();
                                engine.get_crossfade_config()
                            };
                            let remaining_ms =
                                from_ev.duration_ms.saturating_sub(from_ev.position_ms);
                            let trigger_mode_str = match crossfade_cfg.trigger_mode {
                                CrossfadeTriggerMode::AutoDetectDb => "auto_detect_db",
                                CrossfadeTriggerMode::FixedPointMs => "fixed_point_ms",
                                CrossfadeTriggerMode::Manual => "manual",
                            };

                            let should_trigger = match crossfade_cfg.trigger_mode {
                                CrossfadeTriggerMode::Manual => {
                                    autodj::set_last_transition_decision(TransitionDecisionDebug {
                                        engine: "sam_classic".to_string(),
                                        from_deck: Some(from_deck.to_string()),
                                        to_deck: Some(to_deck.to_string()),
                                        trigger_mode: Some(trigger_mode_str.to_string()),
                                        reason: "manual_trigger_mode_no_autostart".to_string(),
                                        outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                        threshold_db: None,
                                        outgoing_remaining_ms: Some(remaining_ms),
                                        fixed_point_ms: None,
                                        hold_ms: Some(SAM_HOLD_MS),
                                        skip_cause: None,
                                    });
                                    false
                                }
                                CrossfadeTriggerMode::FixedPointMs => {
                                    let fixed_point_ms = crossfade_cfg
                                        .fixed_crossfade_point_ms
                                        .unwrap_or(crossfade_cfg.fixed_crossfade_ms.max(500));
                                    let trigger = remaining_ms <= fixed_point_ms as u64;
                                    autodj::set_last_transition_decision(TransitionDecisionDebug {
                                        engine: "sam_classic".to_string(),
                                        from_deck: Some(from_deck.to_string()),
                                        to_deck: Some(to_deck.to_string()),
                                        trigger_mode: Some(trigger_mode_str.to_string()),
                                        reason: if trigger {
                                            "fixed_point_triggered".to_string()
                                        } else {
                                            "fixed_point_waiting".to_string()
                                        },
                                        outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                        threshold_db: None,
                                        outgoing_remaining_ms: Some(remaining_ms),
                                        fixed_point_ms: Some(fixed_point_ms),
                                        hold_ms: None,
                                        skip_cause: None,
                                    });
                                    trigger
                                }
                                CrossfadeTriggerMode::AutoDetectDb => {
                                    let in_window = from_ev.position_ms
                                        >= crossfade_cfg.auto_detect_min_ms as u64
                                        && remaining_ms <= crossfade_cfg.auto_detect_max_ms as u64;
                                    if !in_window {
                                        sam_below_threshold_since.remove(&from_deck);
                                        autodj::set_last_transition_decision(
                                            TransitionDecisionDebug {
                                                engine: "sam_classic".to_string(),
                                                from_deck: Some(from_deck.to_string()),
                                                to_deck: Some(to_deck.to_string()),
                                                trigger_mode: Some(trigger_mode_str.to_string()),
                                                reason: "auto_detect_outside_window".to_string(),
                                                outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                                threshold_db: Some(crossfade_cfg.auto_detect_db),
                                                outgoing_remaining_ms: Some(remaining_ms),
                                                fixed_point_ms: None,
                                                hold_ms: Some(SAM_HOLD_MS),
                                                skip_cause: None,
                                            },
                                        );
                                        false
                                    } else if from_ev.rms_db_pre_fader
                                        <= crossfade_cfg.auto_detect_db
                                    {
                                        let now = std::time::Instant::now();
                                        let since = sam_below_threshold_since
                                            .entry(from_deck)
                                            .or_insert(now);
                                        let held_ms = now.duration_since(*since).as_millis() as u32;
                                        let trigger = held_ms >= SAM_HOLD_MS;
                                        autodj::set_last_transition_decision(
                                            TransitionDecisionDebug {
                                                engine: "sam_classic".to_string(),
                                                from_deck: Some(from_deck.to_string()),
                                                to_deck: Some(to_deck.to_string()),
                                                trigger_mode: Some(trigger_mode_str.to_string()),
                                                reason: if trigger {
                                                    "auto_detect_triggered".to_string()
                                                } else {
                                                    "auto_detect_hold_wait".to_string()
                                                },
                                                outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                                threshold_db: Some(crossfade_cfg.auto_detect_db),
                                                outgoing_remaining_ms: Some(remaining_ms),
                                                fixed_point_ms: None,
                                                hold_ms: Some(held_ms),
                                                skip_cause: None,
                                            },
                                        );
                                        trigger
                                    } else if from_ev.rms_db_pre_fader
                                        <= crossfade_cfg.auto_detect_db + SAM_RELEASE_HYST_DB
                                    {
                                        let now = std::time::Instant::now();
                                        let since = sam_below_threshold_since
                                            .entry(from_deck)
                                            .or_insert(now);
                                        let held_ms = now.duration_since(*since).as_millis() as u32;
                                        let trigger = held_ms >= SAM_HOLD_MS;
                                        autodj::set_last_transition_decision(
                                            TransitionDecisionDebug {
                                                engine: "sam_classic".to_string(),
                                                from_deck: Some(from_deck.to_string()),
                                                to_deck: Some(to_deck.to_string()),
                                                trigger_mode: Some(trigger_mode_str.to_string()),
                                                reason: if trigger {
                                                    "auto_detect_triggered_hysteresis".to_string()
                                                } else {
                                                    "auto_detect_hysteresis_hold".to_string()
                                                },
                                                outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                                threshold_db: Some(crossfade_cfg.auto_detect_db),
                                                outgoing_remaining_ms: Some(remaining_ms),
                                                fixed_point_ms: None,
                                                hold_ms: Some(held_ms),
                                                skip_cause: None,
                                            },
                                        );
                                        trigger
                                    } else {
                                        sam_below_threshold_since.remove(&from_deck);
                                        autodj::set_last_transition_decision(
                                            TransitionDecisionDebug {
                                                engine: "sam_classic".to_string(),
                                                from_deck: Some(from_deck.to_string()),
                                                to_deck: Some(to_deck.to_string()),
                                                trigger_mode: Some(trigger_mode_str.to_string()),
                                                reason: "auto_detect_rms_above_threshold"
                                                    .to_string(),
                                                outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                                threshold_db: Some(crossfade_cfg.auto_detect_db),
                                                outgoing_remaining_ms: Some(remaining_ms),
                                                fixed_point_ms: None,
                                                hold_ms: Some(SAM_HOLD_MS),
                                                skip_cause: None,
                                            },
                                        );
                                        false
                                    }
                                }
                            };

                            if !should_trigger {
                                continue;
                            }

                            let mut fade_ms = crossfade_cfg
                                .fade_out_time_ms
                                .max(crossfade_cfg.fade_in_time_ms)
                                .max(crossfade_cfg.min_fade_time_ms)
                                .min(crossfade_cfg.max_fade_time_ms)
                                .max(100);
                            let mut short_track_fallback = false;
                            if let Some(skip_secs) = crossfade_cfg.skip_short_tracks_secs {
                                let skip_ms = (skip_secs as u64).saturating_mul(1000);
                                if from_ev.duration_ms <= skip_ms || to_ev.duration_ms <= skip_ms {
                                    short_track_fallback = true;
                                    fade_ms = fade_ms.min(250).max(120);
                                }
                            }

                            let incoming_near_end = to_ev.duration_ms > 0
                                && to_ev.position_ms
                                    >= to_ev.duration_ms.saturating_sub(SAM_RECUE_NEAR_END_MS);
                            if incoming_near_end {
                                let mut engine = state.engine.lock().unwrap();
                                let _ = engine.seek(to_deck, 0);
                            }

                            if to_ev.decoder_buffer_ms >= SAM_PREROLL_MIN_MS {
                                let mut engine = state.engine.lock().unwrap();
                                let _ =
                                    start_sam_transition(&mut engine, from_deck, to_deck, fade_ms);
                                autodj::set_last_transition_decision(TransitionDecisionDebug {
                                    engine: "sam_classic".to_string(),
                                    from_deck: Some(from_deck.to_string()),
                                    to_deck: Some(to_deck.to_string()),
                                    trigger_mode: Some(trigger_mode_str.to_string()),
                                    reason: "transition_started".to_string(),
                                    outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                    threshold_db: Some(crossfade_cfg.auto_detect_db),
                                    outgoing_remaining_ms: Some(remaining_ms),
                                    fixed_point_ms: crossfade_cfg.fixed_crossfade_point_ms,
                                    hold_ms: Some(SAM_HOLD_MS),
                                    skip_cause: short_track_fallback
                                        .then_some("short_track".to_string()),
                                });
                            } else {
                                pending_sam_start = Some(PendingSamTransition {
                                    from: from_deck,
                                    to: to_deck,
                                    fade_ms,
                                    short_track_fallback,
                                    trigger_mode: trigger_mode_str.to_string(),
                                    requested_at: std::time::Instant::now(),
                                });
                                autodj::set_last_transition_decision(TransitionDecisionDebug {
                                    engine: "sam_classic".to_string(),
                                    from_deck: Some(from_deck.to_string()),
                                    to_deck: Some(to_deck.to_string()),
                                    trigger_mode: Some(trigger_mode_str.to_string()),
                                    reason: "waiting_incoming_preroll".to_string(),
                                    outgoing_rms_db: Some(from_ev.rms_db_pre_fader),
                                    threshold_db: Some(crossfade_cfg.auto_detect_db),
                                    outgoing_remaining_ms: Some(remaining_ms),
                                    fixed_point_ms: crossfade_cfg.fixed_crossfade_point_ms,
                                    hold_ms: Some(SAM_HOLD_MS),
                                    skip_cause: short_track_fallback
                                        .then_some("short_track".to_string()),
                                });
                            }
                        }
                        AutodjTransitionEngine::MixxxPlanner => {
                            let maybe_from_to = if a_playing && is_ready(b_state) {
                                Some((a.as_ref(), b.as_ref()))
                            } else if b_playing && is_ready(a_state) {
                                Some((b.as_ref(), a.as_ref()))
                            } else {
                                None
                            };

                            if let Some((Some(from_ev), Some(to_ev))) = maybe_from_to {
                                let Some(from_deck) = deck_id_from_event(from_ev) else {
                                    continue;
                                };
                                let Some(to_deck) = deck_id_from_event(to_ev) else {
                                    continue;
                                };

                                let from_snapshot = DeckSnapshot {
                                    deck_id: from_deck,
                                    position_ms: from_ev.position_ms,
                                    duration_ms: from_ev.duration_ms,
                                };
                                let to_snapshot = DeckSnapshot {
                                    deck_id: to_deck,
                                    position_ms: to_ev.position_ms,
                                    duration_ms: to_ev.duration_ms,
                                };

                                let from_markers = load_transition_markers(
                                    &state,
                                    from_ev.song_id,
                                    from_ev.duration_ms,
                                    &mut marker_cache,
                                )
                                .await;
                                let to_markers = load_transition_markers(
                                    &state,
                                    to_ev.song_id,
                                    to_ev.duration_ms,
                                    &mut marker_cache,
                                )
                                .await;

                                let plan = calculate_transition_plan(
                                    &autodj_cfg.mixxx_planner_config,
                                    from_snapshot,
                                    to_snapshot,
                                    from_markers,
                                    to_markers,
                                    false,
                                );

                                if let Some(TransitionPlan {
                                    from_deck,
                                    to_deck,
                                    from_fade_begin_ms,
                                    from_fade_end_ms,
                                    to_start_ms,
                                    start_center,
                                    gap_ms,
                                }) = plan
                                {
                                    if from_ev.position_ms >= from_fade_begin_ms {
                                        if gap_ms > 0 {
                                            if from_ev.position_ms >= from_fade_end_ms {
                                                let mut engine = state.engine.lock().unwrap();
                                                let _ = engine.seek(to_deck, to_start_ms);
                                                let _ = engine.stop_with_completion(from_deck);
                                                pending_gap = Some(PendingGapTransition {
                                                    incoming: to_deck,
                                                    start_at: std::time::Instant::now()
                                                        + Duration::from_millis(gap_ms),
                                                });
                                            }
                                        } else {
                                            let mut engine = state.engine.lock().unwrap();
                                            let _ = engine.seek(to_deck, to_start_ms);
                                            if start_center {
                                                let _ = engine.set_manual_crossfade(0.0);
                                            }
                                            let _ = engine.start_crossfade(from_deck, to_deck);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Phase 1 — Deck control
            load_track,
            play_deck,
            pause_deck,
            stop_deck,
            next_deck,
            seek_deck,
            jog_deck,
            set_channel_gain,
            set_deck_bass,
            set_deck_filter,
            set_deck_pitch,
            set_deck_tempo,
            set_master_level,
            get_master_level,
            set_deck_loop,
            clear_deck_loop,
            get_deck_state,
            get_vu_readings,
            // Phase 1 — Crossfade
            get_crossfade_config,
            set_crossfade_config,
            start_crossfade,
            set_manual_crossfade,
            trigger_manual_fade,
            get_fade_curve_preview,
            // Phase 1 — DSP
            get_channel_dsp,
            set_channel_eq,
            set_channel_agc,
            set_channel_stem_filter,
            set_pipeline_settings,
            analyze_stems,
            get_stem_analysis,
            get_latest_stem_analysis,
            get_stems_runtime_status,
            install_stems_runtime,
            set_deck_stem_source,
            // Phase 1 — Cue points
            get_cue_points,
            set_cue_point,
            delete_cue_point,
            jump_to_cue,
            get_hot_cues,
            set_hot_cue,
            clear_hot_cue,
            trigger_hot_cue,
            rename_hot_cue,
            recolor_hot_cue,
            get_monitor_routing_config,
            set_monitor_routing_config,
            set_deck_cue_preview_enabled,
            // Controller
            list_controller_devices,
            get_controller_status,
            get_controller_config,
            save_controller_config_cmd,
            connect_controller,
            disconnect_controller,
            // Phase 1 — Queue / SAM
            get_queue,
            add_to_queue,
            remove_from_queue,
            reorder_queue,
            complete_queue_item,
            search_songs,
            get_songs_by_weight_range,
            get_song_types,
            get_history,
            get_songs_in_category,
            get_song,
            update_song,
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
            create_sam_category,
            // Phase 7 — Analytics
            get_top_songs,
            get_hourly_heatmap,
            get_song_play_history,
            get_listener_graph,
            get_listener_peak,
            get_event_log,
            clear_event_log,
            write_event_log,
            get_health_snapshot,
            get_health_history,
            generate_report,
            export_report_csv,
            // Waveform analysis/cache
            get_waveform_data,
            // Beat-grid analysis/cache
            analyze_beatgrid,
            get_beatgrid,
            // Phase 3 — Scheduler / AutoDJ / Requests
            get_dj_mode,
            set_dj_mode,
            get_autodj_transition_config,
            set_autodj_transition_config,
            recalculate_autodj_plan_now,
            get_last_transition_decision,
            get_rotation_rules,
            save_rotation_rule,
            delete_rotation_rule,
            get_clockwheel_config,
            save_clockwheel_config,
            get_song_directories,
            enqueue_next_clockwheel_track,
            get_playlists,
            save_playlist,
            set_active_playlist,
            get_next_autodj_track,
            get_shows,
            save_show,
            delete_show,
            get_upcoming_events,
            get_gap_killer_config,
            set_gap_killer_config,
            get_request_policy,
            set_request_policy,
            get_pending_requests,
            accept_request_p3,
            reject_request_p3,
            get_request_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct RuntimeTrackPick {
    song_id: i64,
    file_path: String,
    queue_id: Option<i64>,
    from_rotation: bool,
    declared_duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct PendingGapTransition {
    incoming: crate::audio::crossfade::DeckId,
    start_at: std::time::Instant,
}

#[derive(Debug, Clone)]
struct PendingSamTransition {
    from: crate::audio::crossfade::DeckId,
    to: crate::audio::crossfade::DeckId,
    fade_ms: u32,
    short_track_fallback: bool,
    trigger_mode: String,
    requested_at: std::time::Instant,
}

fn deck_id_from_event(
    ev: &crate::audio::engine::DeckStateEvent,
) -> Option<crate::audio::crossfade::DeckId> {
    match ev.deck.as_str() {
        "deck_a" => Some(crate::audio::crossfade::DeckId::DeckA),
        "deck_b" => Some(crate::audio::crossfade::DeckId::DeckB),
        _ => None,
    }
}

fn event_for_deck<'a>(
    a: &'a Option<crate::audio::engine::DeckStateEvent>,
    b: &'a Option<crate::audio::engine::DeckStateEvent>,
    deck: crate::audio::crossfade::DeckId,
) -> Option<&'a crate::audio::engine::DeckStateEvent> {
    match deck {
        crate::audio::crossfade::DeckId::DeckA => a.as_ref(),
        crate::audio::crossfade::DeckId::DeckB => b.as_ref(),
        _ => None,
    }
}

fn start_sam_transition(
    engine: &mut crate::audio::engine::AudioEngine,
    from: crate::audio::crossfade::DeckId,
    to: crate::audio::crossfade::DeckId,
    fade_ms: u32,
) -> Result<(), String> {
    use crate::audio::crossfade::DeckId;
    use crate::audio::engine::ManualFadeDirection;

    match (from, to) {
        (DeckId::DeckA, DeckId::DeckB) => {
            engine.trigger_manual_fade(ManualFadeDirection::AtoB, fade_ms)
        }
        (DeckId::DeckB, DeckId::DeckA) => {
            engine.trigger_manual_fade(ManualFadeDirection::BtoA, fade_ms)
        }
        _ => engine.start_crossfade(from, to),
    }
}

fn cue_value(cues: &[crate::db::local::CuePoint], names: &[&str]) -> Option<u64> {
    for name in names {
        if let Some(cp) = cues
            .iter()
            .find(|c| {
                c.cue_kind != crate::db::local::CueKind::Hotcue && c.name.eq_ignore_ascii_case(name)
            })
            .map(|c| c.position_ms.max(0) as u64)
        {
            return Some(cp);
        }
    }
    None
}

async fn load_transition_markers(
    state: &AppState,
    song_id: Option<i64>,
    duration_ms: u64,
    cache: &mut std::collections::HashMap<
        i64,
        crate::scheduler::transition_planner::TransitionMarkers,
    >,
) -> crate::scheduler::transition_planner::TransitionMarkers {
    let Some(song_id) = song_id else {
        return crate::scheduler::transition_planner::TransitionMarkers::default();
    };
    if let Some(cached) = cache.get(&song_id).copied() {
        return cached;
    }

    let mut markers = crate::scheduler::transition_planner::TransitionMarkers::default();
    if let Some(pool) = &state.local_db {
        if let Ok(cues) = crate::db::local::get_cue_points(pool, song_id).await {
            markers.intro_start_ms = cue_value(&cues, &["intro_start", "intro"]);
            markers.intro_end_ms = cue_value(&cues, &["intro_end"]);
            markers.outro_start_ms = cue_value(&cues, &["outro_start", "outro"]);
            markers.outro_end_ms = cue_value(&cues, &["outro_end"]);
            markers.first_sound_ms = cue_value(&cues, &["first_sound", "start"]);
            markers.last_sound_ms = cue_value(&cues, &["last_sound", "end"]);

            if markers.first_sound_ms.is_none() {
                markers.first_sound_ms = Some(0);
            }
            if markers.last_sound_ms.is_none() {
                markers.last_sound_ms = Some(duration_ms);
            }
        }
    }

    cache.insert(song_id, markers);
    markers
}

async fn translate_sam_file_path(local_pool: &sqlx::SqlitePool, input: String) -> String {
    if let Ok(cfg) = crate::db::local::get_sam_db_config(local_pool).await {
        if !cfg.path_prefix_from.is_empty() {
            return crate::db::sam::translate_path(
                &input,
                &cfg.path_prefix_from,
                &cfg.path_prefix_to,
            );
        }
    }
    input
}

async fn pick_next_track(
    state: &AppState,
    mode: crate::scheduler::autodj::DjMode,
    claimed_queue_ids: &std::collections::HashSet<i64>,
) -> Option<RuntimeTrackPick> {
    let local_pool = state.local_db.clone()?;
    let sam_pool = {
        let guard = state.sam_db.read().await;
        guard.as_ref().cloned()
    }?;

    if let Ok(queue) = crate::db::sam::get_queue(&sam_pool).await {
        for entry in queue {
            if claimed_queue_ids.contains(&entry.id) {
                continue;
            }
            let mut song = entry.song;
            if song.is_none() {
                song = crate::db::sam::get_song(&sam_pool, entry.song_id)
                    .await
                    .ok()
                    .flatten();
            }
            if let Some(song) = song {
                let translated = translate_sam_file_path(&local_pool, song.filename.clone()).await;
                return Some(RuntimeTrackPick {
                    song_id: song.id,
                    file_path: translated,
                    queue_id: Some(entry.id),
                    from_rotation: false,
                    declared_duration_ms: (song.duration > 0)
                        .then_some(song.duration as u64 * 1000),
                });
            }
        }
    }

    if mode == crate::scheduler::autodj::DjMode::Assisted {
        return None;
    }

    let rotation_pick = crate::scheduler::rotation::select_next_track(&local_pool, &sam_pool, None)
        .await
        .ok()
        .flatten()?;
    let translated = translate_sam_file_path(&local_pool, rotation_pick.file_path).await;

    Some(RuntimeTrackPick {
        song_id: rotation_pick.song_id,
        file_path: translated,
        queue_id: None,
        from_rotation: true,
        declared_duration_ms: (rotation_pick.duration > 0)
            .then_some(rotation_pick.duration as u64 * 1000),
    })
}

async fn top_up_rotation_queue(
    state: &AppState,
    claimed_queue_ids: &std::collections::HashSet<i64>,
) {
    let Some(local_pool) = state.local_db.clone() else {
        return;
    };

    let sam_pool = {
        let guard = state.sam_db.read().await;
        guard.as_ref().cloned()
    };
    let Some(sam_pool) = sam_pool else {
        return;
    };

    let clockwheel_cfg = crate::scheduler::rotation::get_clockwheel_config(&local_pool)
        .await
        .unwrap_or_default();
    let target_depth = clockwheel_cfg.rules.keep_songs_in_queue as usize;
    if target_depth == 0 {
        return;
    }

    let queue = match crate::db::sam::get_queue(&sam_pool).await {
        Ok(q) => q,
        Err(err) => {
            log::warn!("Failed to read queue for AutoDJ top-up: {}", err);
            return;
        }
    };

    let unclaimed_depth = queue
        .iter()
        .filter(|entry| !claimed_queue_ids.contains(&entry.id))
        .count();
    if unclaimed_depth >= target_depth {
        return;
    }

    let mut excluded_song_ids: std::collections::HashSet<i64> =
        queue.iter().map(|entry| entry.song_id).collect();
    {
        let engine = state.engine.lock().unwrap();
        for deck in [
            crate::audio::crossfade::DeckId::DeckA,
            crate::audio::crossfade::DeckId::DeckB,
            crate::audio::crossfade::DeckId::SoundFx,
            crate::audio::crossfade::DeckId::Aux1,
            crate::audio::crossfade::DeckId::Aux2,
            crate::audio::crossfade::DeckId::VoiceFx,
        ] {
            if let Some(song_id) = engine.get_deck_state(deck).and_then(|ev| ev.song_id) {
                excluded_song_ids.insert(song_id);
            }
        }
    }

    let mut needed = target_depth.saturating_sub(unclaimed_depth);
    let max_attempts = (needed.saturating_mul(8)).max(8);
    for _ in 0..max_attempts {
        if needed == 0 {
            break;
        }

        let next = match crate::scheduler::rotation::select_next_track_with_exclusions(
            &local_pool,
            &sam_pool,
            None,
            Some(&excluded_song_ids),
        )
        .await
        {
            Ok(Some(song)) => song,
            Ok(None) => break,
            Err(err) => {
                log::warn!("Clockwheel top-up selection failed: {}", err);
                break;
            }
        };

        if excluded_song_ids.contains(&next.song_id) {
            continue;
        }

        match crate::db::sam::add_to_queue(&sam_pool, next.song_id).await {
            Ok(_) => {
                excluded_song_ids.insert(next.song_id);
                needed = needed.saturating_sub(1);
            }
            Err(err) => {
                log::warn!(
                    "Failed to add rotation song {} to queue: {}",
                    next.song_id,
                    err
                );
                break;
            }
        }
    }
}

async fn claim_queue_item(state: &AppState, queue_id: i64) {
    let sam_pool = {
        let guard = state.sam_db.read().await;
        guard.as_ref().cloned()
    };
    let Some(sam_pool) = sam_pool else {
        return;
    };

    if let Err(err) = crate::db::sam::remove_from_queue(&sam_pool, queue_id).await {
        log::warn!(
            "Failed to claim queue item {} after deck load: {}",
            queue_id,
            err
        );
    }
}

async fn process_track_completions(
    state: &AppState,
    completed: Vec<crate::audio::engine::TrackCompletionEvent>,
) -> Vec<i64> {
    if completed.is_empty() {
        return Vec::new();
    }
    let sam_pool = {
        let guard = state.sam_db.read().await;
        guard.as_ref().cloned()
    };
    let Some(sam_pool) = sam_pool else {
        return Vec::new();
    };
    let local_pool = state.local_db.clone();
    let mut completed_queue_ids = Vec::new();
    let listeners_total: i64 = state
        .encoder_manager
        .get_all_runtime()
        .iter()
        .map(|r| r.listeners.unwrap_or(0) as i64)
        .sum();
    let listener_snapshot = listeners_total.clamp(0, i32::MAX as i64) as i32;

    for ev in completed {
        let song = match crate::db::sam::get_song(&sam_pool, ev.song_id)
            .await
            .ok()
            .flatten()
        {
            Some(s) => s,
            None => continue,
        };

        if let Some(queue_id) = ev.queue_id {
            completed_queue_ids.push(queue_id);
            if let Err(err) =
                crate::db::sam::complete_track(&sam_pool, queue_id, &song, listener_snapshot).await
            {
                log::warn!(
                    "Failed to complete queue track (queue_id={}, song_id={}): {}",
                    queue_id,
                    ev.song_id,
                    err
                );
                let _ = crate::db::sam::add_to_history_with_listeners(
                    &sam_pool,
                    &song,
                    listener_snapshot,
                )
                .await;
            }
        } else if let Err(err) =
            crate::db::sam::add_to_history_with_listeners(&sam_pool, &song, listener_snapshot).await
        {
            log::warn!(
                "Failed to append history for completed track (song_id={}): {}",
                ev.song_id,
                err
            );
        }

        if let Some(local) = &local_pool {
            if let Err(err) =
                crate::scheduler::rotation::apply_weight_delta_on_play(local, &sam_pool, ev.song_id)
                    .await
            {
                log::warn!(
                    "Failed to apply on-play weight adjustment (song_id={}): {}",
                    ev.song_id,
                    err
                );
            }
        }
    }

    completed_queue_ids
}

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
