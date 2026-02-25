# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is

**DesiZone Broadcaster** is a Tauri v2 desktop application — a full SAM Broadcaster Pro replacement with:
- Rust audio engine (CPAL + Symphonia) for local soundcard I/O and low-latency deck playback
- Advanced crossfading with SAM-parity curve types (Linear, Exponential, S-Curve, Logarithmic, Constant Power)
- Full audio mixer pipeline: 5 channels (Deck A, Deck B, Sound FX, Aux 1, Voice FX) → each with EQ → AGC → DSP → Mixer → Air Out + Icecast encoder
- Direct Icecast/Shoutcast streaming from Rust (no Liquidsoap dependency)
- SAM MySQL schema compatibility (reads/writes `songlist`, `queuelist`, `historylist`, etc.)
- Local SQLite for per-song cue points, per-channel DSP settings, crossfade config
- React + TypeScript frontend (Tauri webview)

**This is NOT a server-side project.** It runs locally on the operator's machine. The related server-side project is at `../desizone-broadcast-engine` (NestJS + Liquidsoap), but they are independent.

## Project Status

**All 7 Phases Complete!** 🎉 DesiZone Broadcaster is a fully-functional, production-ready SAM Broadcaster replacement.

### What's done
- ✅ Phase 1: Audio Engine (Rust CPAL + Symphonia + custom DSP + Mixers)
- ✅ Phase 2: Operator UI (React frontend, Deck controls, Waveforms, Crossfade/DSP Config)
- ✅ Phase 3: Automation & Scheduling (Weekly Scheduler, Rotation Rules, GAP Killer)
- ✅ Phase 4: Streaming & Encoders (Multiple Icecast streams, local recording)
- ✅ Phase 5: Scripting & Advanced Audio (Lua Scripting, Voice FX, Mic Input)
- ✅ Phase 6: DBE Gateway Integration (Remote DJ, AutoPilot, Live Talk)
- ✅ Phase 7: Analytics & Operations (Play history, Event logger, Metrics)

For full details, see `docs/PROJECT_COMPLETE.md`.

## Commands

```bash
# Install dependencies
npm install

# Dev mode (starts Vite + cargo build + Tauri window)
npm run tauri dev

# Build release
npm run tauri build

# Rust only — compile check (fast, no window)
cd src-tauri && cargo check

# Rust tests
cd src-tauri && cargo test

# Lint TypeScript frontend
npm run lint        # (once configured)

# Type check
npm run typecheck   # (once configured)
```

> **First `cargo build` takes 10–15 minutes** — it compiles Tauri, CPAL, Symphonia, sqlx, and all DSP crates from scratch. Subsequent builds are incremental.

## Architecture

### Stack
- **Frontend**: React 19 + TypeScript, Vite, Tauri webview
- **Backend**: Rust (Tauri v2 core process)
- **Audio I/O**: `cpal` 0.15 — CPAL handles WASAPI/ASIO (Windows), CoreAudio (macOS), ALSA (Linux)
- **Audio decode**: `symphonia` 0.5 — pure Rust MP3/AAC/FLAC/OGG/WAV decoder
- **DSP**: `biquad` (EQ filters), `dasp` (signal primitives), custom AGC + compressor
- **Ring buffers**: `ringbuf` 0.4 — lock-free SPSC, decoder thread → real-time audio thread
- **Database**: `sqlx` 0.7 — async SQLite (local) + MySQL (SAM schema)
- **Streaming**: `reqwest` (HTTP PUT to Icecast)

### Module Map

| File | Responsibility |
|------|---------------|
| `audio/engine.rs` | `AudioEngine` — owns CPAL output stream, 5 deck slots, crossfade state, calls pipelines each callback |
| `audio/deck.rs` | `Deck` — state machine: Idle → Loading → Ready → Playing → Crossfading; reads PCM from ring buffer |
| `audio/decoder.rs` | Symphonia decode loop on background thread → writes PCM into deck's ring buffer |
| `audio/crossfade.rs` | `FadeCurve` enum, `CrossfadeConfig`, `CrossfadeState` machine with auto-detect RMS trigger |
| `audio/mixer.rs` | Sums 5 channel buffers into master mix buffer with per-channel gains |
| `audio/dsp/eq.rs` | `ChannelEQ` — 3 biquad filters (low shelf, peak mid, high shelf) |
| `audio/dsp/agc.rs` | `GatedAGC` — RMS window, noise gate, smoothed gain, pre-emphasis 50μs/75μs |
| `audio/dsp/compressor.rs` | `MultibandCompressor` (5 bands + dual LF/HF) + `Clipper` |
| `audio/dsp/pipeline.rs` | `ChannelPipeline` — runs EQ → AGC → Compressor per channel |
| `stream/icecast.rs` | Reads from master output ring buffer, encodes MP3, HTTP PUT to Icecast |
| `db/local.rs` | SQLite: `cue_points`, `song_fade_overrides`, `channel_dsp_settings`, `crossfade_config` |
| `db/sam.rs` | MySQL: reads `songlist`, writes `queuelist`, `historylist` (SAM-compatible) |
| `commands/*.rs` | Tauri `#[tauri::command]` handlers exposed to the React frontend via IPC |
| `state.rs` | `AppState` — `Arc<Mutex<AudioEngine>>` passed to all command handlers |
| `src/lib/bridge.ts` | TypeScript: all `invoke()` and `listen()` wrappers with typed payloads |

### Audio Render Loop (real-time thread — no allocations allowed)

```
CPAL output callback:
  for each active channel (deck_a, deck_b, sound_fx, aux_1, voice_fx):
    → deck.fill_buffer(output)          // reads from lock-free ring buffer
    → apply crossfade gain              // from CrossfadeState progress
    → pipeline.process(buffer)         // EQ → AGC → Compressor
    → accumulate into mix_buffer
  → master_pipeline.process(mix_buffer)
  → limiter.process(mix_buffer)
  → copy to CPAL output
  → copy to encoder ring buffer        // encoder thread reads asynchronously
```

### Crossfade Curves (pure math — no external library)

| SAM Name | Formula (t = 0.0 → 1.0) |
|----------|--------------------------|
| Linear | `1.0 - t` |
| Exponential | `(1.0 - t)²` |
| S-Curve | `0.5 * (1 + cos(π·t))` |
| Logarithmic | `log₁₀(1 + 9·(1−t)) / log₁₀(10)` |
| Constant Power | `cos(t · π/2)` out / `sin(t · π/2)` in |

### Tauri IPC — Key Commands

```typescript
// Deck control
invoke('load_track', { deck: 'deck_a', filePath, songId })
invoke('play_deck', { deck })
invoke('pause_deck', { deck })
invoke('seek_deck', { deck, positionMs })

// Crossfade
invoke('get_crossfade_config') → CrossfadeConfig
invoke('set_crossfade_config', { config })
invoke('get_fade_curve_preview', { curve, timeMsOut, timeMsIn }) → [{t, gainOut, gainIn}]

// DSP
invoke('get_channel_dsp', { channel }) → ChannelDspSettings
invoke('set_channel_eq', { channel, lowGainDb, midGainDb, highGainDb })
invoke('set_channel_agc', { channel, enabled, gateDb, maxGainDb })

// Cue points
invoke('get_cue_points', { songId }) → CuePoint[]
invoke('set_cue_point', { songId, name, positionMs })

// Streaming
invoke('start_stream', { host, port, mount, password, bitrateKbps })
invoke('stop_stream')

// Events emitted from Rust → listen in frontend
'deck_state_changed'   // { deck, state, positionMs, durationMs }
'crossfade_progress'   // { progress 0.0–1.0, outgoingDeck, incomingDeck }
'vu_meter'             // { channel, leftDb, rightDb } every ~80ms
'stream_connected'     // { mount }
```

### SQLite Schema (local.rs)

```sql
-- Per-song cue points (Start, End, Intro, Outro, Fade, XFade, Custom 0-9)
CREATE TABLE cue_points (
    id INTEGER PRIMARY KEY,
    song_id INTEGER NOT NULL,
    name TEXT NOT NULL,       -- 'start','end','intro','outro','fade','xfade','custom_0'...'custom_9'
    position_ms INTEGER NOT NULL,
    UNIQUE(song_id, name)
);

-- Per-song fade overrides (NULL = inherit from global config)
CREATE TABLE song_fade_overrides (
    song_id INTEGER PRIMARY KEY,
    fade_out_enabled INTEGER, fade_out_curve TEXT, fade_out_time_ms INTEGER,
    fade_in_enabled INTEGER, fade_in_curve TEXT, fade_in_time_ms INTEGER,
    crossfade_mode TEXT, gain_db REAL
);

-- Per-channel DSP settings (one row per channel)
CREATE TABLE channel_dsp_settings (
    channel TEXT PRIMARY KEY,  -- 'deck_a','deck_b','sound_fx','aux_1','voice_fx','mixer','output'
    eq_low_gain_db REAL DEFAULT 0.0,  eq_low_freq_hz REAL DEFAULT 100.0,
    eq_mid_gain_db REAL DEFAULT 0.0,  eq_mid_freq_hz REAL DEFAULT 1000.0, eq_mid_q REAL DEFAULT 0.707,
    eq_high_gain_db REAL DEFAULT 0.0, eq_high_freq_hz REAL DEFAULT 8000.0,
    agc_enabled INTEGER DEFAULT 0, agc_gate_db REAL DEFAULT -31.0,
    agc_max_gain_db REAL DEFAULT 5.0, agc_attack_ms REAL DEFAULT 100.0,
    agc_release_ms REAL DEFAULT 500.0, agc_pre_emphasis TEXT DEFAULT '75us',
    comp_enabled INTEGER DEFAULT 0, comp_settings_json TEXT
);

-- Global crossfade config (single row id=1)
CREATE TABLE crossfade_config (
    id INTEGER PRIMARY KEY DEFAULT 1,
    config_json TEXT NOT NULL
);
```

## Key Files

- `docs/phase1-audio-engine.md` — full Phase 1 architecture plan with all design decisions
- `docs/adr/001-no-liquidsoap.md` — why Liquidsoap was dropped
- `src-tauri/Cargo.toml` — all Rust dependencies
- `src-tauri/src/audio/crossfade.rs` — **START HERE for Phase 1 implementation**
- `src-tauri/src/audio/engine.rs` — most complex file; implement last in the audio chain

## SAM Broadcaster Parity Reference

The screenshots driving this design show SAM Broadcaster's:
1. **Cross-Fading dialog** — fade out/in curve types, time, level%, auto-detect dB trigger, fixed crossfade point, min/max fade time
2. **Song Settings tab** — cue points (Start/End/Intro/Outro/Fade/XFade + custom), BPM, Gap killer, Gain
3. **Audio Mixer Pipeline** — Deck A/B/SoundFX/Aux1/VoiceFX → EQ→AGC→DSP per channel → Mixer → Air Out + Encoders
4. **Audio Settings AGC tab** — Gated AGC, 5-band processor, Dual-band, Stereo expander, Clipper

## Project Management

- ADO Project: **Minhaj Prayer Project** (see parent `CLAUDE.md` at `~/CLAUDE.md`)
- Assign work items to: Minhaj Services (services@minhaj.work)
- Use gemini-cli for generating tests and documentation
