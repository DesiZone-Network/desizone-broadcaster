pub const DEVICE_NAME_HINT: &str = "djcontrol starlight";

pub const SHIFT_STATUS: u8 = 0x90;
pub const SHIFT_NOTE: u8 = 0x03;

pub const DECK_A_NOTE_STATUS: u8 = 0x91;
pub const DECK_B_NOTE_STATUS: u8 = 0x92;
pub const DECK_A_SHIFT_NOTE_STATUS: u8 = 0x94;
pub const DECK_B_SHIFT_NOTE_STATUS: u8 = 0x95;

pub const DECK_A_PAD_STATUS: u8 = 0x96;
pub const DECK_B_PAD_STATUS: u8 = 0x97;

pub const PLAY_NOTE: u8 = 0x07;
pub const CUE_NOTE: u8 = 0x06;
pub const SYNC_NOTE: u8 = 0x05;

pub const PAD_1_NOTE: u8 = 0x00;
pub const PAD_4_NOTE: u8 = 0x03;
pub const PAD_SHIFT_1_NOTE: u8 = 0x08;
pub const PAD_SHIFT_4_NOTE: u8 = 0x0B;
pub const LOOP_PAD_1_NOTE: u8 = 0x10;
pub const LOOP_PAD_4_NOTE: u8 = 0x13;
pub const LOOP_PAD_SHIFT_1_NOTE: u8 = 0x18;
pub const LOOP_PAD_SHIFT_4_NOTE: u8 = 0x1B;

pub const XFADE_STATUS: u8 = 0xB0;
pub const XFADE_CC: u8 = 0x00;
pub const MASTER_VOLUME_CC: u8 = 0x01;

pub const DECK_A_CC_STATUS: u8 = 0xB1;
pub const DECK_B_CC_STATUS: u8 = 0xB2;
pub const DECK_A_SHIFT_CC_STATUS: u8 = 0xB4;
pub const DECK_B_SHIFT_CC_STATUS: u8 = 0xB5;

pub const CHANNEL_GAIN_CC: u8 = 0x00;
pub const FILTER_CC: u8 = 0x01;
pub const BASS_CC: u8 = 0x02;
pub const TEMPO_MSB_CC: u8 = 0x08;
pub const TEMPO_LSB_CC: u8 = 0x28;
pub const JOG_BEND_CC: u8 = 0x09;
pub const JOG_SCRATCH_CC: u8 = 0x0A;
