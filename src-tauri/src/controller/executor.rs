use tauri::{AppHandle, Manager};

use crate::{
    audio::crossfade::DeckId,
    db::local::{BeatGridAnalysis, HotCue},
    state::AppState,
};

use super::types::ControllerAction;

const BEATGRID_CONFIDENCE_MIN: f32 = 0.55;
const LOOP_TOGGLE_TOLERANCE_MS: u64 = 35;

pub async fn execute_action(app_handle: AppHandle, action: ControllerAction) {
    let state = app_handle.state::<AppState>();

    match action {
        ControllerAction::TogglePlay { deck } => {
            let playing = {
                let engine = state.engine.lock().unwrap();
                engine
                    .get_deck_state(deck)
                    .map(|s| s.state == "playing" || s.state == "crossfading")
                    .unwrap_or(false)
            };
            let mut engine = state.engine.lock().unwrap();
            if playing {
                let _ = engine.pause(deck);
            } else {
                let _ = engine.play(deck);
            }
        }
        ControllerAction::CueToStart { deck } => {
            let mut engine = state.engine.lock().unwrap();
            let _ = engine.pause(deck);
            let _ = engine.seek(deck, 0);
        }
        ControllerAction::SyncToOther { deck } => {
            sync_deck_to_other(&state, deck).await;
        }
        ControllerAction::HotCueTrigger { deck, slot } => {
            trigger_hotcue(&state, deck, slot).await;
        }
        ControllerAction::HotCueSet { deck, slot } => {
            set_hotcue(&state, deck, slot).await;
        }
        ControllerAction::SetBeatLoop { deck, beats } => {
            set_beat_loop(&state, deck, beats).await;
        }
        ControllerAction::ClearLoop { deck } => {
            clear_loop_preserve_position(&state, deck);
        }
        ControllerAction::SetTempo { deck, tempo_pct, .. } => {
            let mut engine = state.engine.lock().unwrap();
            let _ = engine.set_deck_tempo(deck, tempo_pct.clamp(-8.0, 8.0));
        }
        ControllerAction::SetGain { deck, gain, .. } => {
            let mut engine = state.engine.lock().unwrap();
            let _ = engine.set_channel_gain(deck, gain.clamp(0.0, 1.0));
        }
        ControllerAction::SetBass { deck, bass_db, .. } => {
            let mut engine = state.engine.lock().unwrap();
            let _ = engine.set_deck_bass(deck, bass_db.clamp(-12.0, 12.0));
        }
        ControllerAction::SetFilter { deck, amount, .. } => {
            let mut engine = state.engine.lock().unwrap();
            let _ = engine.set_deck_filter(deck, amount.clamp(-1.0, 1.0));
        }
        ControllerAction::SetCrossfader { position, .. } => {
            let mut engine = state.engine.lock().unwrap();
            let _ = engine.set_manual_crossfade(position.clamp(-1.0, 1.0));
        }
        ControllerAction::SetMasterVolume { level, .. } => {
            let mut engine = state.engine.lock().unwrap();
            let _ = engine.set_master_level(level.clamp(0.0, 1.0));
        }
        ControllerAction::JogNudge { deck, delta_steps } => {
            jog_nudge(&state, deck, delta_steps);
        }
    }
}

fn jog_nudge(state: &AppState, deck: DeckId, delta_steps: i8) {
    let deck_state = {
        let engine = state.engine.lock().unwrap();
        engine.get_deck_state(deck)
    };

    let Some(deck_state) = deck_state else {
        return;
    };
    if deck_state.duration_ms == 0 {
        return;
    }
    let clamped_steps = delta_steps.clamp(-12, 12) as i64;
    if clamped_steps == 0 {
        return;
    }
    let step_ms: i64 = if deck_state.state == "playing" || deck_state.state == "crossfading" {
        25
    } else {
        150
    };

    let position = deck_state.position_ms as i64;
    let duration = deck_state.duration_ms as i64;
    let target = (position + (clamped_steps * step_ms)).clamp(0, duration) as u64;

    let mut engine = state.engine.lock().unwrap();
    let _ = engine.seek(deck, target);
}

async fn trigger_hotcue(state: &AppState, deck: DeckId, slot: u8) {
    let Some(pool) = &state.local_db else {
        return;
    };
    let deck_state = {
        let engine = state.engine.lock().unwrap();
        engine.get_deck_state(deck)
    };
    let Some(song_id) = deck_state.and_then(|s| s.song_id) else {
        return;
    };
    let cue = crate::db::local::get_hot_cue(pool, song_id, slot)
        .await
        .ok()
        .flatten();
    let Some(cue) = cue else {
        return;
    };
    let mut engine = state.engine.lock().unwrap();
    let _ = engine.seek(deck, cue.position_ms.max(0) as u64);
}

async fn set_hotcue(state: &AppState, deck: DeckId, slot: u8) {
    let Some(pool) = &state.local_db else {
        return;
    };
    let deck_state = {
        let engine = state.engine.lock().unwrap();
        engine.get_deck_state(deck)
    };
    let Some(deck_state) = deck_state else {
        return;
    };
    let Some(song_id) = deck_state.song_id else {
        return;
    };

    let cue = HotCue {
        song_id,
        slot,
        position_ms: deck_state.position_ms as i64,
        label: format!("Cue {slot}"),
        color_hex: "#f59e0b".to_string(),
        quantized: false,
    };
    let _ = crate::db::local::upsert_hot_cue(pool, &cue).await;
}

async fn sync_deck_to_other(state: &AppState, deck: DeckId) {
    let Some(other) = other_deck(deck) else {
        return;
    };

    let (this_state, other_state) = {
        let engine = state.engine.lock().unwrap();
        (engine.get_deck_state(deck), engine.get_deck_state(other))
    };

    let (Some(this_state), Some(other_state)) = (this_state, other_state) else {
        return;
    };

    let mut target_tempo_pct = other_state.tempo_pct;

    if let (Some(pool), Some(this_song_id), Some(other_song_id)) =
        (&state.local_db, this_state.song_id, other_state.song_id)
    {
        let this_grid = crate::db::local::get_latest_beatgrid_by_song_id(pool, this_song_id)
            .await
            .ok()
            .flatten();
        let other_grid = crate::db::local::get_latest_beatgrid_by_song_id(pool, other_song_id)
            .await
            .ok()
            .flatten();
        if let (Some(a), Some(b)) = (this_grid, other_grid) {
            if a.confidence >= BEATGRID_CONFIDENCE_MIN
                && b.confidence >= BEATGRID_CONFIDENCE_MIN
                && a.bpm > 0.0
                && b.bpm > 0.0
            {
                target_tempo_pct = ((b.bpm / a.bpm) - 1.0) * 100.0;
            }
        }
    }

    let mut engine = state.engine.lock().unwrap();
    let _ = engine.set_deck_tempo(deck, target_tempo_pct.clamp(-50.0, 50.0));
}

fn other_deck(deck: DeckId) -> Option<DeckId> {
    match deck {
        DeckId::DeckA => Some(DeckId::DeckB),
        DeckId::DeckB => Some(DeckId::DeckA),
        _ => None,
    }
}

async fn set_beat_loop(state: &AppState, deck: DeckId, beats: u8) {
    if beats == 0 {
        return;
    }
    let deck_state = {
        let engine = state.engine.lock().unwrap();
        engine.get_deck_state(deck)
    };
    let Some(deck_state) = deck_state else {
        return;
    };
    if deck_state.duration_ms == 0 {
        return;
    }

    let beatgrid = if let (Some(pool), Some(song_id)) = (&state.local_db, deck_state.song_id) {
        crate::db::local::get_latest_beatgrid_by_song_id(pool, song_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let song_meta_bpm = if let Some(song_id) = deck_state.song_id {
        let sam_pool = {
            let guard = state.sam_db.read().await;
            guard.clone()
        };
        if let Some(pool) = sam_pool {
            crate::db::sam::get_song(&pool, song_id)
                .await
                .ok()
                .flatten()
                .map(|song| song.bpm as f32)
                .filter(|bpm| *bpm > 0.0)
        } else {
            None
        }
    } else {
        None
    };

    let Some((start_ms, end_ms)) = build_loop_range_ms(
        deck_state.position_ms,
        deck_state.duration_ms,
        deck_state.state.as_str(),
        beatgrid.as_ref(),
        song_meta_bpm,
        beats,
    ) else {
        return;
    };

    if deck_state.loop_enabled {
        if let (Some(current_start), Some(current_end)) =
            (deck_state.loop_start_ms, deck_state.loop_end_ms)
        {
            let start_diff = current_start.abs_diff(start_ms);
            let end_diff = current_end.abs_diff(end_ms);
            let current_len = current_end.saturating_sub(current_start);
            let requested_len = end_ms.saturating_sub(start_ms);
            let len_diff = current_len.abs_diff(requested_len);
            if (start_diff <= LOOP_TOGGLE_TOLERANCE_MS && end_diff <= LOOP_TOGGLE_TOLERANCE_MS)
                || len_diff <= LOOP_TOGGLE_TOLERANCE_MS
            {
                clear_loop_preserve_position(state, deck);
                return;
            }
        }
    }

    let mut engine = state.engine.lock().unwrap();
    if engine.set_deck_loop(deck, start_ms, end_ms).is_ok() {
        let _ = engine.seek(deck, start_ms);
    }
}

fn build_loop_range_ms(
    position_ms: u64,
    duration_ms: u64,
    state: &str,
    beatgrid: Option<&BeatGridAnalysis>,
    song_meta_bpm: Option<f32>,
    beats: u8,
) -> Option<(u64, u64)> {
    if duration_ms == 0 || beats == 0 {
        return None;
    }
    let playing = state == "playing" || state == "crossfading";
    if let Some(grid) = beatgrid {
        if grid.beat_times_ms.len() >= 2 {
            let mut beats_ms = grid.beat_times_ms.clone();
            let analyzed_bpm = if grid.bpm > 0.0 { grid.bpm } else { 0.0 };
            let meta_bpm = song_meta_bpm.filter(|b| *b > 0.0).unwrap_or(0.0);

            if analyzed_bpm > 0.0 && meta_bpm > 0.0 {
                let mismatch = (meta_bpm - analyzed_bpm).abs() / meta_bpm;
                if mismatch > 0.08 {
                    let period_ms = 60_000.0 / meta_bpm;
                    let first_grid_beat = beats_ms.first().copied().unwrap_or(0).max(0);
                    let anchor_ms = grid.first_beat_ms.max(0).max(first_grid_beat) as f32;
                    let pos_beats = (position_ms as f32 - anchor_ms) / period_ms;
                    let quantized = if playing {
                        (pos_beats - 1e-6).ceil()
                    } else {
                        pos_beats.round()
                    };
                    let start_ms = (anchor_ms + quantized * period_ms).round().max(0.0) as u64;
                    let end_ms = (start_ms as f32 + beats as f32 * period_ms)
                        .round()
                        .max(start_ms as f32) as u64;
                    let clamped_end = end_ms.min(duration_ms);
                    if clamped_end > start_ms + 25 {
                        return Some((start_ms.min(duration_ms), clamped_end));
                    }
                }

                let ratio = meta_bpm / analyzed_bpm;
                if (1.8..=2.2).contains(&ratio) {
                    let mut expanded = Vec::with_capacity(beats_ms.len().saturating_mul(2));
                    for idx in 0..beats_ms.len().saturating_sub(1) {
                        let a = beats_ms[idx];
                        let b = beats_ms[idx + 1];
                        expanded.push(a);
                        expanded.push(((a + b) as f32 * 0.5).round() as i64);
                    }
                    if let Some(last) = beats_ms.last().copied() {
                        expanded.push(last);
                    }
                    if expanded.len() >= 2 {
                        beats_ms = expanded;
                    }
                } else if (0.45..=0.55).contains(&ratio) {
                    let reduced: Vec<i64> = beats_ms
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, beat)| if idx % 2 == 0 { Some(*beat) } else { None })
                        .collect();
                    if reduced.len() >= 2 {
                        beats_ms = reduced;
                    }
                }
            }

            let mut start_idx = 0usize;
            if playing {
                let mut next_idx = beats_ms
                    .iter()
                    .position(|ms| *ms >= position_ms as i64)
                    .unwrap_or(beats_ms.len().saturating_sub(1));
                if next_idx >= beats_ms.len() {
                    next_idx = beats_ms.len().saturating_sub(1);
                }
                start_idx = next_idx;
            } else {
                for (idx, beat) in beats_ms.iter().enumerate() {
                    if *beat <= position_ms as i64 {
                        start_idx = idx;
                    } else {
                        break;
                    }
                }
                if start_idx + 1 < beats_ms.len() {
                    let cur = beats_ms[start_idx];
                    let next = beats_ms[start_idx + 1];
                    if (next - position_ms as i64).abs() < (position_ms as i64 - cur).abs() {
                        start_idx += 1;
                    }
                }
            }

            let start_ms = beats_ms.get(start_idx).copied().unwrap_or(0).max(0) as u64;
            let end_idx = start_idx.saturating_add(beats as usize);
            let end_ms = if let Some(end) = beats_ms.get(end_idx) {
                (*end).max(0) as u64
            } else {
                let fallback_beat_ms = if grid.bpm > 0.0 {
                    ((60_000.0 / grid.bpm) * beats as f32).round() as u64
                } else {
                    500 * beats as u64
                };
                start_ms.saturating_add(fallback_beat_ms)
            };
            let clamped_end = end_ms.min(duration_ms);
            if clamped_end > start_ms + 25 {
                return Some((start_ms, clamped_end));
            }
        }
    }

    let fallback_bpm = beatgrid
        .map(|g| g.bpm)
        .filter(|b| *b > 0.0)
        .or(song_meta_bpm.filter(|b| *b > 0.0))
        .unwrap_or(120.0);
    let beat_ms = (60_000.0 / fallback_bpm).round() as u64;
    let loop_ms = beat_ms.saturating_mul(beats as u64);
    let start_ms = position_ms.min(duration_ms);
    let end_ms = start_ms.saturating_add(loop_ms).min(duration_ms);
    if end_ms <= start_ms + 25 {
        return None;
    }
    Some((start_ms, end_ms))
}

fn clear_loop_preserve_position(state: &AppState, deck: DeckId) {
    let mut engine = state.engine.lock().unwrap();
    let current_pos = engine.get_deck_state(deck).map(|s| s.position_ms);
    if engine.clear_deck_loop(deck).is_err() {
        return;
    }
    if let Some(position_ms) = current_pos {
        let _ = engine.seek(deck, position_ms);
    }
}
