use tauri::State;

use crate::{
    audio::dsp::{
        agc::AgcConfig, eq::EqConfig, pipeline::PipelineSettings, stem_filter::StemFilterMode,
    },
    state::AppState,
};

use super::audio_commands::parse_deck;

enum ChannelTarget {
    Deck(crate::audio::crossfade::DeckId),
    Master,
}

fn parse_channel_target(channel: &str) -> Result<ChannelTarget, String> {
    if channel == "master" {
        Ok(ChannelTarget::Master)
    } else {
        Ok(ChannelTarget::Deck(parse_deck(channel)?))
    }
}

#[tauri::command]
pub async fn get_channel_dsp(
    channel: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::db::local::ChannelDspRow>, String> {
    let pool = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    crate::db::local::get_channel_dsp(pool, &channel)
        .await
        .map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub async fn set_channel_eq(
    channel: String,
    low_gain_db: f32,
    mid_gain_db: f32,
    high_gain_db: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target = parse_channel_target(&channel)?;
    let mut settings = get_pipeline_settings(&channel, &state).await?;
    settings.eq.low_gain_db = low_gain_db;
    settings.eq.mid_gain_db = mid_gain_db;
    settings.eq.high_gain_db = high_gain_db;
    apply_and_persist(target, settings, &channel, &state).await
}

#[tauri::command]
pub async fn set_channel_agc(
    channel: String,
    enabled: bool,
    gate_db: Option<f32>,
    max_gain_db: Option<f32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target = parse_channel_target(&channel)?;
    let mut settings = get_pipeline_settings(&channel, &state).await?;
    settings.agc.enabled = enabled;
    if let Some(g) = gate_db {
        settings.agc.gate_db = g;
    }
    if let Some(m) = max_gain_db {
        settings.agc.max_gain_db = m;
    }
    apply_and_persist(target, settings, &channel, &state).await
}

#[tauri::command]
pub async fn set_pipeline_settings(
    channel: String,
    settings: PipelineSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target = parse_channel_target(&channel)?;
    apply_and_persist(target, settings, &channel, &state).await
}

#[tauri::command]
pub async fn set_channel_stem_filter(
    channel: String,
    mode: StemFilterMode,
    amount: Option<f32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target = parse_channel_target(&channel)?;
    let mut settings = get_pipeline_settings(&channel, &state).await?;
    settings.stem_filter.mode = mode;
    if let Some(v) = amount {
        settings.stem_filter.amount = v.clamp(0.0, 1.0);
    }
    apply_and_persist(target, settings, &channel, &state).await
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn get_pipeline_settings(
    channel: &str,
    state: &AppState,
) -> Result<PipelineSettings, String> {
    if let Some(pool) = &state.local_db {
        if let Some(row) = crate::db::local::get_channel_dsp(pool, channel)
            .await
            .map_err(|e| format!("DB: {e}"))?
        {
            if let Some(json) = &row.pipeline_settings_json {
                if let Ok(settings) = serde_json::from_str::<PipelineSettings>(json) {
                    return Ok(settings);
                }
            }
            return Ok(row_to_settings(&row));
        }
    }
    Ok(default_pipeline_for_channel(channel))
}

fn default_pipeline_for_channel(channel: &str) -> PipelineSettings {
    let mut settings = PipelineSettings::default();
    match channel {
        "deck_a" | "deck_b" => {
            settings.stem_filter.amount = 0.82;
        }
        "voice_fx" => {
            settings.stem_filter.amount = 0.55;
        }
        "sound_fx" | "aux_1" | "aux_2" | "master" => {
            settings.stem_filter.amount = 0.70;
        }
        _ => {}
    }
    settings
}

async fn apply_and_persist(
    target: ChannelTarget,
    settings: PipelineSettings,
    channel: &str,
    state: &AppState,
) -> Result<(), String> {
    // Apply to audio engine
    match target {
        ChannelTarget::Deck(deck_id) => {
            state
                .engine
                .lock()
                .unwrap()
                .set_channel_pipeline(deck_id, settings.clone())?;
        }
        ChannelTarget::Master => {
            state
                .engine
                .lock()
                .unwrap()
                .set_master_pipeline(settings.clone())?;
        }
    }

    // Persist to SQLite
    if let Some(pool) = &state.local_db {
        let row = settings_to_row(channel.to_string(), &settings);
        crate::db::local::upsert_channel_dsp(pool, &row)
            .await
            .map_err(|e| format!("DB persist: {e}"))?;
    }
    Ok(())
}

fn row_to_settings(r: &crate::db::local::ChannelDspRow) -> PipelineSettings {
    use crate::audio::dsp::agc::PreEmphasis;
    let mut settings = PipelineSettings {
        eq: EqConfig {
            low_gain_db: r.eq_low_gain_db as f32,
            low_freq_hz: r.eq_low_freq_hz as f32,
            mid_gain_db: r.eq_mid_gain_db as f32,
            mid_freq_hz: r.eq_mid_freq_hz as f32,
            mid_q: r.eq_mid_q as f32,
            high_gain_db: r.eq_high_gain_db as f32,
            high_freq_hz: r.eq_high_freq_hz as f32,
        },
        agc: AgcConfig {
            enabled: r.agc_enabled,
            gate_db: r.agc_gate_db as f32,
            max_gain_db: r.agc_max_gain_db as f32,
            target_db: -18.0,
            attack_ms: r.agc_attack_ms as f32,
            release_ms: r.agc_release_ms as f32,
            pre_emphasis: match r.agc_pre_emphasis.as_str() {
                "50us" => PreEmphasis::Us50,
                "75us" => PreEmphasis::Us75,
                _ => PreEmphasis::None,
            },
        },
        ..Default::default()
    };

    if let Some(comp_json) = &r.comp_settings_json {
        if let Ok(mut mb) =
            serde_json::from_str::<crate::audio::dsp::compressor::MultibandConfig>(comp_json)
        {
            mb.enabled = r.comp_enabled;
            settings.multiband = mb;
        }
    } else {
        settings.multiband.enabled = r.comp_enabled;
    }

    settings
}

fn settings_to_row(channel: String, s: &PipelineSettings) -> crate::db::local::ChannelDspRow {
    crate::db::local::ChannelDspRow {
        channel,
        eq_low_gain_db: s.eq.low_gain_db as f64,
        eq_low_freq_hz: s.eq.low_freq_hz as f64,
        eq_mid_gain_db: s.eq.mid_gain_db as f64,
        eq_mid_freq_hz: s.eq.mid_freq_hz as f64,
        eq_mid_q: s.eq.mid_q as f64,
        eq_high_gain_db: s.eq.high_gain_db as f64,
        eq_high_freq_hz: s.eq.high_freq_hz as f64,
        agc_enabled: s.agc.enabled,
        agc_gate_db: s.agc.gate_db as f64,
        agc_max_gain_db: s.agc.max_gain_db as f64,
        agc_attack_ms: s.agc.attack_ms as f64,
        agc_release_ms: s.agc.release_ms as f64,
        agc_pre_emphasis: match s.agc.pre_emphasis {
            crate::audio::dsp::agc::PreEmphasis::Us50 => "50us".to_string(),
            crate::audio::dsp::agc::PreEmphasis::Us75 => "75us".to_string(),
            crate::audio::dsp::agc::PreEmphasis::None => "none".to_string(),
        },
        comp_enabled: s.multiband.enabled,
        comp_settings_json: serde_json::to_string(&s.multiband).ok(),
        pipeline_settings_json: serde_json::to_string(s).ok(),
    }
}
