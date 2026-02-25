use serde::{Deserialize, Serialize};

use crate::audio::crossfade::DeckId;

use super::autodj::{AutoTransitionMode, MixxxPlannerConfig};

#[derive(Debug, Clone, Copy)]
pub struct DeckSnapshot {
    pub deck_id: DeckId,
    pub position_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransitionMarkers {
    pub intro_start_ms: Option<u64>,
    pub intro_end_ms: Option<u64>,
    pub outro_start_ms: Option<u64>,
    pub outro_end_ms: Option<u64>,
    pub first_sound_ms: Option<u64>,
    pub last_sound_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitionPlan {
    pub from_deck: DeckId,
    pub to_deck: DeckId,
    pub from_fade_begin_ms: u64,
    pub from_fade_end_ms: u64,
    pub to_start_ms: u64,
    pub start_center: bool,
    pub gap_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedMarkers {
    intro_start_ms: u64,
    intro_end_ms: u64,
    outro_start_ms: u64,
    outro_end_ms: u64,
    first_sound_ms: u64,
    last_sound_ms: u64,
}

fn clamp_ms(v: u64, duration_ms: u64) -> u64 {
    v.min(duration_ms)
}

fn resolve_markers(markers: TransitionMarkers, duration_ms: u64) -> ResolvedMarkers {
    let first_sound = clamp_ms(markers.first_sound_ms.unwrap_or(0), duration_ms);
    let mut intro_start = clamp_ms(markers.intro_start_ms.unwrap_or(first_sound), duration_ms);
    intro_start = intro_start.max(first_sound);

    let mut intro_end = clamp_ms(markers.intro_end_ms.unwrap_or(intro_start), duration_ms);
    intro_end = intro_end.max(intro_start);

    let mut last_sound = clamp_ms(markers.last_sound_ms.unwrap_or(duration_ms), duration_ms);
    last_sound = last_sound.max(intro_end);

    let mut outro_end = clamp_ms(markers.outro_end_ms.unwrap_or(last_sound), duration_ms);
    outro_end = outro_end.max(intro_end);
    outro_end = outro_end.min(duration_ms);

    let mut outro_start = clamp_ms(markers.outro_start_ms.unwrap_or(outro_end), duration_ms);
    outro_start = outro_start.max(intro_end);
    outro_start = outro_start.min(outro_end);

    last_sound = last_sound.max(outro_start).min(outro_end);

    ResolvedMarkers {
        intro_start_ms: intro_start,
        intro_end_ms: intro_end,
        outro_start_ms: outro_start,
        outro_end_ms: outro_end,
        first_sound_ms: first_sound,
        last_sound_ms: last_sound,
    }
}

fn cap_transition_len(
    requested_ms: u64,
    from: DeckSnapshot,
    to: DeckSnapshot,
    to_next_fade_begin_ms: u64,
    to_start_ms: u64,
    min_track_duration_ms: u64,
) -> u64 {
    let from_remaining = from.duration_ms.saturating_sub(from.position_ms);
    let mut capped = requested_ms.min(from_remaining);

    let to_window = to_next_fade_begin_ms.saturating_sub(to_start_ms);
    if to_window > 0 {
        let to_cap = to_window.saturating_sub((min_track_duration_ms / 2).max(1));
        capped = capped.min(to_cap);
    }

    capped.min(to.duration_ms)
}

pub fn calculate_transition_plan(
    config: &MixxxPlannerConfig,
    from: DeckSnapshot,
    to: DeckSnapshot,
    from_markers: TransitionMarkers,
    to_markers: TransitionMarkers,
    force_recue_to_start: bool,
) -> Option<TransitionPlan> {
    if !config.enabled {
        return None;
    }
    let min_track_duration_ms = config.min_track_duration_ms as u64;
    if from.duration_ms < min_track_duration_ms || to.duration_ms < min_track_duration_ms {
        return None;
    }

    let from_m = resolve_markers(from_markers, from.duration_ms);
    let to_m = resolve_markers(to_markers, to.duration_ms);

    let mut start_center = false;
    let mut gap_ms = 0_u64;
    let transition_abs_ms = (config.transition_time_sec.unsigned_abs() as u64) * 1000;

    let (default_to_start_ms, to_next_fade_begin_ms) = match config.mode {
        AutoTransitionMode::FullIntroOutro | AutoTransitionMode::FadeAtOutroStart => {
            (to_m.intro_start_ms, to_m.outro_start_ms)
        }
        AutoTransitionMode::FixedSkipSilence | AutoTransitionMode::FixedStartCenterSkipSilence => {
            (to_m.first_sound_ms, to_m.last_sound_ms)
        }
        AutoTransitionMode::FixedFullTrack => (0, to.duration_ms),
    };

    // Recue if the idle deck is already close to (or inside) its own next fade window.
    // Using transition length here avoids immediate re-transition loops after manual seeks.
    let recue_window_ms = min_track_duration_ms.max(transition_abs_ms.max(1_000));
    let should_recue = force_recue_to_start
        || to.position_ms >= to_next_fade_begin_ms.saturating_sub(recue_window_ms);
    let to_start_ms = if should_recue {
        default_to_start_ms
    } else {
        to.position_ms.min(to.duration_ms.saturating_sub(1))
    };

    let fade_begin_ms;
    let mut fade_end_ms;

    match config.mode {
        AutoTransitionMode::FullIntroOutro => {
            let outro_len = from_m.outro_end_ms.saturating_sub(from_m.outro_start_ms);
            let intro_len = to_m.intro_end_ms.saturating_sub(to_start_ms);

            let requested = if outro_len > 0 && intro_len > 0 {
                outro_len.min(intro_len)
            } else if outro_len > 0 {
                outro_len
            } else if intro_len > 0 {
                intro_len
            } else {
                transition_abs_ms
            };

            let len = cap_transition_len(
                requested,
                from,
                to,
                to_next_fade_begin_ms,
                to_start_ms,
                min_track_duration_ms,
            );
            fade_end_ms = from_m
                .outro_end_ms
                .max(from.position_ms)
                .min(from.duration_ms);
            fade_begin_ms = fade_end_ms.saturating_sub(len).max(from.position_ms);
        }
        AutoTransitionMode::FadeAtOutroStart => {
            let outro_len = from_m.outro_end_ms.saturating_sub(from_m.outro_start_ms);
            let intro_len = to_m.intro_end_ms.saturating_sub(to_start_ms);

            let requested = if outro_len > 0 {
                if intro_len > 0 {
                    outro_len.min(intro_len)
                } else {
                    outro_len
                }
            } else if intro_len > 0 {
                intro_len
            } else {
                transition_abs_ms
            };

            let len = cap_transition_len(
                requested,
                from,
                to,
                to_next_fade_begin_ms,
                to_start_ms,
                min_track_duration_ms,
            );
            fade_begin_ms = from_m
                .outro_start_ms
                .max(from.position_ms)
                .min(from.duration_ms);
            fade_end_ms = fade_begin_ms.saturating_add(len).min(from.duration_ms);
        }
        AutoTransitionMode::FixedStartCenterSkipSilence => {
            start_center = true;
            if config.transition_time_sec < 0 {
                gap_ms = transition_abs_ms;
            }
            let requested = if gap_ms > 0 { 0 } else { transition_abs_ms };
            let len = cap_transition_len(
                requested,
                from,
                to,
                to_next_fade_begin_ms,
                to_start_ms,
                min_track_duration_ms,
            );
            fade_end_ms = from_m
                .last_sound_ms
                .max(from.position_ms)
                .min(from.duration_ms);
            fade_begin_ms = fade_end_ms.saturating_sub(len).max(from.position_ms);
        }
        AutoTransitionMode::FixedSkipSilence => {
            if config.transition_time_sec < 0 {
                gap_ms = transition_abs_ms;
            }
            let requested = if gap_ms > 0 { 0 } else { transition_abs_ms };
            let len = cap_transition_len(
                requested,
                from,
                to,
                to_next_fade_begin_ms,
                to_start_ms,
                min_track_duration_ms,
            );
            fade_end_ms = from_m
                .last_sound_ms
                .max(from.position_ms)
                .min(from.duration_ms);
            fade_begin_ms = fade_end_ms.saturating_sub(len).max(from.position_ms);
        }
        AutoTransitionMode::FixedFullTrack => {
            if config.transition_time_sec < 0 {
                gap_ms = transition_abs_ms;
            }
            let requested = if gap_ms > 0 { 0 } else { transition_abs_ms };
            let len = cap_transition_len(
                requested,
                from,
                to,
                to_next_fade_begin_ms,
                to_start_ms,
                min_track_duration_ms,
            );
            fade_end_ms = from.duration_ms.max(from.position_ms).min(from.duration_ms);
            fade_begin_ms = fade_end_ms.saturating_sub(len).max(from.position_ms);
        }
    }

    if fade_end_ms < fade_begin_ms {
        fade_end_ms = fade_begin_ms;
    }

    Some(TransitionPlan {
        from_deck: from.deck_id,
        to_deck: to.deck_id,
        from_fade_begin_ms: fade_begin_ms,
        from_fade_end_ms: fade_end_ms,
        to_start_ms,
        start_center,
        gap_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(mode: AutoTransitionMode, transition_time_sec: i32) -> MixxxPlannerConfig {
        MixxxPlannerConfig {
            enabled: true,
            mode,
            transition_time_sec,
            min_track_duration_ms: 200,
        }
    }

    fn deck(deck_id: DeckId, pos: u64, dur: u64) -> DeckSnapshot {
        DeckSnapshot {
            deck_id,
            position_ms: pos,
            duration_ms: dur,
        }
    }

    #[test]
    fn full_intro_outro_longer_intro_uses_outro_length() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 0, 90_000);
        let from_m = TransitionMarkers {
            outro_start_ms: Some(80_000),
            outro_end_ms: Some(90_000),
            ..Default::default()
        };
        let to_m = TransitionMarkers {
            intro_start_ms: Some(0),
            intro_end_ms: Some(20_000),
            ..Default::default()
        };
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FullIntroOutro, 10),
            from,
            to,
            from_m,
            to_m,
            false,
        )
        .unwrap();
        assert_eq!(plan.from_fade_end_ms - plan.from_fade_begin_ms, 10_000);
    }

    #[test]
    fn full_intro_outro_longer_outro_uses_intro_length() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 0, 90_000);
        let from_m = TransitionMarkers {
            outro_start_ms: Some(70_000),
            outro_end_ms: Some(90_000),
            ..Default::default()
        };
        let to_m = TransitionMarkers {
            intro_start_ms: Some(0),
            intro_end_ms: Some(10_000),
            ..Default::default()
        };
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FullIntroOutro, 10),
            from,
            to,
            from_m,
            to_m,
            false,
        )
        .unwrap();
        assert_eq!(plan.from_fade_end_ms - plan.from_fade_begin_ms, 10_000);
    }

    #[test]
    fn fade_at_outro_start_starts_exactly_at_outro_start() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 0, 90_000);
        let from_m = TransitionMarkers {
            outro_start_ms: Some(70_000),
            outro_end_ms: Some(90_000),
            ..Default::default()
        };
        let to_m = TransitionMarkers {
            intro_start_ms: Some(0),
            intro_end_ms: Some(8_000),
            ..Default::default()
        };
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FadeAtOutroStart, 10),
            from,
            to,
            from_m,
            to_m,
            false,
        )
        .unwrap();
        assert_eq!(plan.from_fade_begin_ms, 70_000);
        assert_eq!(plan.from_fade_end_ms, 78_000);
    }

    #[test]
    fn fixed_full_track_positive_starts_before_end() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 0, 120_000);
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FixedFullTrack, 10),
            from,
            to,
            TransitionMarkers::default(),
            TransitionMarkers::default(),
            false,
        )
        .unwrap();
        assert_eq!(plan.from_fade_begin_ms, 90_000);
        assert_eq!(plan.from_fade_end_ms, 100_000);
        assert_eq!(plan.gap_ms, 0);
    }

    #[test]
    fn fixed_full_track_negative_inserts_gap() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 0, 120_000);
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FixedFullTrack, -7),
            from,
            to,
            TransitionMarkers::default(),
            TransitionMarkers::default(),
            false,
        )
        .unwrap();
        assert_eq!(plan.from_fade_begin_ms, 100_000);
        assert_eq!(plan.from_fade_end_ms, 100_000);
        assert_eq!(plan.gap_ms, 7_000);
    }

    #[test]
    fn fixed_skip_silence_uses_first_last_sound_fallback() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 89_000, 90_000);
        let from_m = TransitionMarkers {
            last_sound_ms: Some(85_000),
            ..Default::default()
        };
        let to_m = TransitionMarkers {
            first_sound_ms: Some(3_000),
            last_sound_ms: Some(82_000),
            ..Default::default()
        };
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FixedSkipSilence, 10),
            from,
            to,
            from_m,
            to_m,
            false,
        )
        .unwrap();
        assert_eq!(plan.to_start_ms, 3_000);
        assert_eq!(plan.from_fade_end_ms, 85_000);
    }

    #[test]
    fn fixed_start_center_sets_center_flag() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 0, 90_000);
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FixedStartCenterSkipSilence, 10),
            from,
            to,
            TransitionMarkers {
                last_sound_ms: Some(90_000),
                ..Default::default()
            },
            TransitionMarkers {
                first_sound_ms: Some(2_000),
                last_sound_ms: Some(80_000),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        assert!(plan.start_center);
    }

    #[test]
    fn short_incoming_track_clamps_transition_length() {
        let from = deck(DeckId::DeckA, 10_000, 100_000);
        let to = deck(DeckId::DeckB, 0, 12_000);
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FixedFullTrack, 10),
            from,
            to,
            TransitionMarkers::default(),
            TransitionMarkers::default(),
            false,
        )
        .unwrap();
        let len = plan.from_fade_end_ms - plan.from_fade_begin_ms;
        assert!(len <= 11_900);
    }

    #[test]
    fn seeked_idle_deck_near_end_forces_recue_to_mode_start() {
        let from = deck(DeckId::DeckA, 40_000, 100_000);
        let to = deck(DeckId::DeckB, 89_000, 90_000);
        let plan = calculate_transition_plan(
            &cfg(AutoTransitionMode::FixedFullTrack, 10),
            from,
            to,
            TransitionMarkers::default(),
            TransitionMarkers::default(),
            false,
        )
        .unwrap();
        assert_eq!(plan.to_start_ms, 0);
    }
}
