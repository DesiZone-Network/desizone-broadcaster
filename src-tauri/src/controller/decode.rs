use std::collections::HashMap;

use crate::audio::crossfade::DeckId;

use super::{
    starlight_profile as map,
    types::ControllerAction,
};

#[derive(Default)]
pub struct DecodeState {
    pub shift_pressed: bool,
    tempo_msb: HashMap<DeckId, u8>,
    tempo_lsb: HashMap<DeckId, u8>,
}

pub fn decode_message(state: &mut DecodeState, message: &[u8]) -> Vec<ControllerAction> {
    if message.len() < 3 {
        return Vec::new();
    }

    let status = message[0];
    let data1 = message[1];
    let data2 = message[2];

    if status == map::SHIFT_STATUS && data1 == map::SHIFT_NOTE {
        state.shift_pressed = data2 >= 0x40;
        return Vec::new();
    }

    if is_note_status(status) && data2 == 0 {
        return Vec::new();
    }

    match status {
        map::DECK_A_NOTE_STATUS | map::DECK_B_NOTE_STATUS => {
            decode_transport(status, data1)
                .into_iter()
                .collect()
        }
        map::DECK_A_SHIFT_NOTE_STATUS | map::DECK_B_SHIFT_NOTE_STATUS => {
            decode_transport(shift_note_to_base(status), data1)
                .into_iter()
                .collect()
        }
        map::DECK_A_PAD_STATUS | map::DECK_B_PAD_STATUS => {
            decode_pads(state.shift_pressed, status, data1)
        }
        map::XFADE_STATUS => decode_master_and_crossfader(data1, data2)
            .into_iter()
            .collect(),
        map::DECK_A_CC_STATUS
        | map::DECK_B_CC_STATUS
        | map::DECK_A_SHIFT_CC_STATUS
        | map::DECK_B_SHIFT_CC_STATUS => decode_deck_cc(state, status, data1, data2),
        _ => Vec::new(),
    }
}

fn is_note_status(status: u8) -> bool {
    (status & 0xF0) == 0x90
}

fn shift_note_to_base(status: u8) -> u8 {
    match status {
        map::DECK_A_SHIFT_NOTE_STATUS => map::DECK_A_NOTE_STATUS,
        map::DECK_B_SHIFT_NOTE_STATUS => map::DECK_B_NOTE_STATUS,
        _ => status,
    }
}

fn deck_from_note_status(status: u8) -> Option<DeckId> {
    match status {
        map::DECK_A_NOTE_STATUS => Some(DeckId::DeckA),
        map::DECK_B_NOTE_STATUS => Some(DeckId::DeckB),
        _ => None,
    }
}

fn deck_from_cc_status(status: u8) -> Option<DeckId> {
    match status {
        map::DECK_A_CC_STATUS | map::DECK_A_SHIFT_CC_STATUS => Some(DeckId::DeckA),
        map::DECK_B_CC_STATUS | map::DECK_B_SHIFT_CC_STATUS => Some(DeckId::DeckB),
        _ => None,
    }
}

fn decode_transport(status: u8, note: u8) -> Option<ControllerAction> {
    let deck = deck_from_note_status(status)?;
    match note {
        map::PLAY_NOTE => Some(ControllerAction::TogglePlay { deck }),
        map::CUE_NOTE => Some(ControllerAction::CueToStart { deck }),
        map::SYNC_NOTE => Some(ControllerAction::SyncToOther { deck }),
        _ => None,
    }
}

fn decode_pads(shift_pressed: bool, status: u8, note: u8) -> Vec<ControllerAction> {
    let deck = match status {
        map::DECK_A_PAD_STATUS => DeckId::DeckA,
        map::DECK_B_PAD_STATUS => DeckId::DeckB,
        _ => return Vec::new(),
    };

    if (map::LOOP_PAD_1_NOTE..=map::LOOP_PAD_4_NOTE).contains(&note) {
        if shift_pressed {
            return vec![ControllerAction::ClearLoop { deck }];
        }
        let beats = match note {
            map::LOOP_PAD_1_NOTE => 1,
            0x11 => 2,
            0x12 => 4,
            0x13 => 8,
            _ => 0,
        };
        if beats > 0 {
            return vec![ControllerAction::SetBeatLoop { deck, beats }];
        }
    }
    if (map::LOOP_PAD_SHIFT_1_NOTE..=map::LOOP_PAD_SHIFT_4_NOTE).contains(&note) {
        return vec![ControllerAction::ClearLoop { deck }];
    }

    let (slot, explicit_shift) = if (map::PAD_1_NOTE..=map::PAD_4_NOTE).contains(&note) {
        (note - map::PAD_1_NOTE + 1, false)
    } else if (map::PAD_SHIFT_1_NOTE..=map::PAD_SHIFT_4_NOTE).contains(&note) {
        (note - map::PAD_SHIFT_1_NOTE + 1, true)
    } else {
        return Vec::new();
    };

    let use_set = shift_pressed || explicit_shift;
    if use_set {
        vec![ControllerAction::HotCueSet { deck, slot }]
    } else {
        vec![ControllerAction::HotCueTrigger { deck, slot }]
    }
}

fn decode_master_and_crossfader(cc: u8, value: u8) -> Option<ControllerAction> {
    if cc == map::XFADE_CC {
        let normalized = (value as f32 / 127.0).clamp(0.0, 1.0);
        let position = normalized * 2.0 - 1.0;
        return Some(ControllerAction::SetCrossfader {
            position,
            normalized,
        });
    }
    if cc != map::XFADE_CC {
        let normalized = (value as f32 / 127.0).clamp(0.0, 1.0);
        let level = normalized.clamp(0.0, 1.0);
        return Some(ControllerAction::SetMasterVolume { level, normalized });
    }
    None
}

fn decode_deck_cc(
    state: &mut DecodeState,
    status: u8,
    cc: u8,
    value: u8,
) -> Vec<ControllerAction> {
    let Some(deck) = deck_from_cc_status(status) else {
        return Vec::new();
    };

    match cc {
        map::CHANNEL_GAIN_CC if status == map::DECK_A_CC_STATUS || status == map::DECK_B_CC_STATUS => {
            let normalized = (value as f32 / 127.0).clamp(0.0, 1.0);
            vec![ControllerAction::SetGain {
                deck,
                gain: normalized,
                normalized,
            }]
        }
        map::FILTER_CC if status == map::DECK_A_CC_STATUS || status == map::DECK_B_CC_STATUS => {
            let normalized = (value as f32 / 127.0).clamp(0.0, 1.0);
            let amount = normalized * 2.0 - 1.0;
            vec![ControllerAction::SetFilter {
                deck,
                amount,
                normalized,
            }]
        }
        map::BASS_CC if status == map::DECK_A_CC_STATUS || status == map::DECK_B_CC_STATUS => {
            let normalized = (value as f32 / 127.0).clamp(0.0, 1.0);
            let bass_db = normalized * 24.0 - 12.0;
            vec![ControllerAction::SetBass {
                deck,
                bass_db,
                normalized,
            }]
        }
        map::TEMPO_MSB_CC => {
            state.tempo_msb.insert(deck, value);
            let lsb = *state.tempo_lsb.get(&deck).unwrap_or(&0);
            vec![tempo_action(deck, value, lsb)]
        }
        map::TEMPO_LSB_CC => {
            state.tempo_lsb.insert(deck, value);
            let msb = *state.tempo_msb.get(&deck).unwrap_or(&0);
            vec![tempo_action(deck, msb, value)]
        }
        map::JOG_BEND_CC | map::JOG_SCRATCH_CC => {
            let delta_steps = jog_delta(value);
            if delta_steps == 0 {
                Vec::new()
            } else {
                vec![ControllerAction::JogNudge { deck, delta_steps }]
            }
        }
        _ => Vec::new(),
    }
}

fn tempo_action(deck: DeckId, msb: u8, lsb: u8) -> ControllerAction {
    let value14 = ((msb as u16) << 7) | (lsb as u16);
    let normalized = (value14 as f32 / 16383.0).clamp(0.0, 1.0);
    let tempo_pct = normalized * 16.0 - 8.0;
    ControllerAction::SetTempo {
        deck,
        tempo_pct,
        normalized,
    }
}

fn jog_delta(value: u8) -> i8 {
    if value == 0 || value == 0x40 {
        0
    } else if value < 0x40 {
        1
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_crossfader_linear_range() {
        let mut state = DecodeState::default();
        let min = decode_message(&mut state, &[map::XFADE_STATUS, map::XFADE_CC, 0]);
        let max = decode_message(&mut state, &[map::XFADE_STATUS, map::XFADE_CC, 127]);
        match (&min[0], &max[0]) {
            (
                ControllerAction::SetCrossfader { position: p0, .. },
                ControllerAction::SetCrossfader { position: p1, .. },
            ) => {
                assert!((*p0 + 1.0).abs() < 0.001);
                assert!((*p1 - 1.0).abs() < 0.001);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn decode_shift_plus_pad_sets_hotcue() {
        let mut state = DecodeState::default();
        decode_message(&mut state, &[map::SHIFT_STATUS, map::SHIFT_NOTE, 0x7F]);
        let actions = decode_message(&mut state, &[map::DECK_A_PAD_STATUS, map::PAD_1_NOTE, 0x7F]);
        assert!(matches!(
            actions.first(),
            Some(ControllerAction::HotCueSet {
                deck: DeckId::DeckA,
                slot: 1
            })
        ));
    }

    #[test]
    fn decode_tempo_14_bit() {
        let mut state = DecodeState::default();
        let _ = decode_message(&mut state, &[map::DECK_A_CC_STATUS, map::TEMPO_MSB_CC, 0x7F]);
        let actions = decode_message(&mut state, &[map::DECK_A_CC_STATUS, map::TEMPO_LSB_CC, 0x7F]);
        assert!(matches!(
            actions.first(),
            Some(ControllerAction::SetTempo {
                deck: DeckId::DeckA,
                tempo_pct,
                ..
            }) if (*tempo_pct - 8.0).abs() < 0.02
        ));
    }

    #[test]
    fn decode_loop_pad_sets_loop_action() {
        let mut state = DecodeState::default();
        let actions = decode_message(
            &mut state,
            &[map::DECK_A_PAD_STATUS, map::LOOP_PAD_1_NOTE + 2, 0x7F],
        );
        assert!(matches!(
            actions.first(),
            Some(ControllerAction::SetBeatLoop {
                deck: DeckId::DeckA,
                beats: 4
            })
        ));
    }

    #[test]
    fn decode_bass_filter_and_master() {
        let mut state = DecodeState::default();

        let bass = decode_message(&mut state, &[map::DECK_A_CC_STATUS, map::BASS_CC, 0x7F]);
        assert!(matches!(
            bass.first(),
            Some(ControllerAction::SetBass {
                deck: DeckId::DeckA,
                bass_db,
                ..
            }) if (*bass_db - 12.0).abs() < 0.2
        ));

        let filter = decode_message(&mut state, &[map::DECK_B_CC_STATUS, map::FILTER_CC, 0x00]);
        assert!(matches!(
            filter.first(),
            Some(ControllerAction::SetFilter {
                deck: DeckId::DeckB,
                amount,
                ..
            }) if (*amount + 1.0).abs() < 0.02
        ));

        let master = decode_message(&mut state, &[map::XFADE_STATUS, map::MASTER_VOLUME_CC, 0x40]);
        assert!(matches!(
            master.first(),
            Some(ControllerAction::SetMasterVolume { .. })
        ));
    }
}
