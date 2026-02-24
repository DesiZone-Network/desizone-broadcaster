# Plan: DesiZone Desktop — Tauri Audio Engine (Phase 1)

## Context

The existing DBE project (NestJS gateway + Next.js dashboard + Liquidsoap) is a solid server-side automation system but cannot deliver full SAM Broadcaster parity due to inherent web limitations: no local soundcard access, no <10ms cue monitoring latency, no hardware device enumeration, no ASIO/WASAPI exclusive mode. A Tauri desktop app with a Rust audio engine removes all of these limitations.

**Phase 1 goal:** Build the Rust audio engine and audio mixer pipeline first — decks, crossfading with all SAM curve types, per-channel DSP chain (EQ → AGC → DSP), mixer summing, and direct Icecast streaming. No Liquidsoap dependency. The React frontend UI for settings dialogs (crossfade dialog, pipeline view, EQ/AGC panels) comes in Phase 2.

**Mixxx decision:** Mixxx is GPL v2 — code cannot be used directly. All crossfade curves and DSP algorithms will be implemented from scratch in Rust. The math (biquad filters, fade curves, RMS-based AGC) is standard DSP theory with no IP issues.

---

## New Repository: `desizone-desktop`

Standalone repo, not inside the existing DBE monorepo. Shares no files with `desizone-broadcast-engine` at this stage (shared types can be extracted later if needed).

---

## Repository Structure

```
desizone-desktop/
├── src-tauri/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                  # Tauri bootstrap, plugin registration
│       ├── state.rs                 # AppState: Arc<Mutex<AudioEngine>>
│       ├── audio/
│       │   ├── mod.rs
│       │   ├── engine.rs            # AudioEngine: owns CPAL output stream, deck slots
│       │   ├── deck.rs              # Deck state machine (Loading→Ready→Playing→Crossfading)
│       │   ├── decoder.rs           # Symphonia decode loop → ring buffer per deck
│       │   ├── crossfade.rs         # FadeCurve enum + CrossfadeConfig + state machine
│       │   ├── mixer.rs             # Multi-channel summing, per-channel gain
│       │   └── dsp/
│       │       ├── mod.rs
│       │       ├── eq.rs            # 3-band biquad EQ (low shelf, peak mid, high shelf)
│       │       ├── agc.rs           # Gated AGC: RMS window, attack/release, pre-emphasis
│       │       ├── compressor.rs    # 5-band multiband + dual-band (LF/HF) + clipper
│       │       └── pipeline.rs      # ChannelPipeline: EQ → AGC → Compressor per channel
│       ├── stream/
│       │   ├── mod.rs
│       │   └── icecast.rs           # HTTP PUT streaming, MP3 encoding via lame-sys
│       ├── db/
│       │   ├── mod.rs
│       │   ├── sam.rs               # sqlx MySQL: reads songlist, writes queuelist/historylist
│       │   └── local.rs             # sqlx SQLite: cue points, DSP settings, crossfade config
│       └── commands/
│           ├── mod.rs
│           ├── audio_commands.rs    # load_track, play, pause, stop, seek, volume
│           ├── crossfade_commands.rs# get/set crossfade config, get fade curve preview data
│           ├── dsp_commands.rs      # get/set channel EQ, AGC, compressor
│           ├── cue_commands.rs      # get/set cue points per song
│           ├── queue_commands.rs    # queue CRUD (via SAM MySQL)
│           └── stream_commands.rs  # start/stop Icecast stream, get status
├── src/                             # React + TypeScript frontend (Phase 2 UI focus)
│   ├── lib/
│   │   └── bridge.ts               # All Tauri invoke() / listen() wrappers
│   └── components/                 # (Phase 2) CrossfadeSettings, AudioPipeline, etc.
├── package.json
└── tauri.conf.json
```

---

## Rust Crate Dependencies (`src-tauri/Cargo.toml`)

```toml
[dependencies]
tauri = { version = "2", features = ["protocol-asset"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }

# Audio I/O
cpal = { version = "0.15", features = ["asio"] }          # ASIO/WASAPI/CoreAudio

# Audio decoding (MPL v2 — commercial OK)
symphonia = { version = "0.5", features = ["mp3", "aac", "flac", "ogg", "wav", "aiff"] }

# Lock-free ring buffer (audio-safe — no alloc on real-time thread)
ringbuf = "0.4"

# DSP — biquad IIR filters for EQ
biquad = "0.4"

# DSP — signal processing primitives
dasp = { version = "0.11", features = ["all"] }

# Database (MIT/Apache)
sqlx = { version = "0.7", features = ["mysql", "sqlite", "runtime-tokio", "chrono"] }

# MP3 encoding for Icecast stream
# lame-sys = "0.1"    (wraps LAME; LGPL — dynamically linked OK for commercial)
# Alternative: minimp3_fixed for decode only; use shine-rs for encode

# HTTP client for Icecast PUT
reqwest = { version = "0.12", features = ["stream", "blocking"] }

# BPM detection
aubio-rs = "0.2"     # wraps aubio C lib; GPL but can be replaced with stratum-dsp

[build-dependencies]
tauri-build = "2"
```

> **Note on aubio-rs / GPL:** If the app is commercial, replace `aubio-rs` with `stratum-dsp` (check its license) or defer BPM detection to Phase 2 when licensing is confirmed.

---

## Audio Engine Design (`audio/engine.rs`)

The `AudioEngine` struct owns:
- 5 deck slots: `deck_a`, `deck_b`, `sound_fx`, `aux_1`, `voice_fx`
- 5 per-channel `ChannelPipeline` (EQ → AGC → DSP)
- 1 master `ChannelPipeline` (for the Mixer bus)
- `CrossfadeState` — active fade between two decks
- CPAL output stream handle
- Ring buffer sender to encoder thread (for Icecast)

The CPAL callback runs on a **dedicated real-time thread** (no allocations, no locks except try_lock):

```
CPAL output callback (real-time):
  for each active deck:
    → fill PCM from deck's ring buffer (decoder writes, engine reads)
    → apply crossfade gain (from CrossfadeState)
    → apply ChannelPipeline (EQ → AGC)
    → accumulate into mix buffer
  → apply master ChannelPipeline
  → apply Limiter
  → write to CPAL output buffer (Air Out)
  → write copy to encoder ring buffer (Icecast feed)
```

Decoder threads run separately (one per deck), filling ring buffers from Symphonia decode loops.

---

## Crossfade Engine (`audio/crossfade.rs`)

### Fade Curve Implementations (pure math — no Mixxx code)

Maps exactly to SAM's curve selector dropdowns:

| SAM Curve | Formula (t = 0.0→1.0 fade progress) |
|-----------|--------------------------------------|
| `Linear` | `1.0 - t` |
| `Exponential` | `(1.0 - t).powi(2)` (quadratic) |
| `SCurve` | `0.5 * (1.0 + cos(π * t))` (cosine) |
| `Logarithmic` | `log10(1 + 9*(1-t)) / log10(10)` |
| `ConstantPower` | `cos(t * π/2)` out, `sin(t * π/2)` in |

### CrossfadeConfig struct

Maps 1:1 to SAM's Cross-Fading dialog:

```rust
pub struct CrossfadeConfig {
    // Fade Out (left side of SAM dialog)
    pub fade_out_enabled: bool,
    pub fade_out_curve: FadeCurve,       // Exponential default
    pub fade_out_time_ms: u32,           // 0–10000
    pub fade_out_level_pct: u8,          // 0–100 (80% default)

    // Fade In (right side of SAM dialog)
    pub fade_in_enabled: bool,
    pub fade_in_curve: FadeCurve,        // SCurve default
    pub fade_in_time_ms: u32,
    pub fade_in_level_pct: u8,

    // Cross-fade section
    pub crossfade_mode: CrossfadeMode,   // AutoDetect | Fixed
    pub fixed_crossfade_ms: u32,         // 8000 default
    pub auto_detect_db: f32,             // -3.0 dB default (trigger level)
    pub min_fade_time_ms: u32,           // 3000
    pub max_fade_time_ms: u32,           // 10000
    pub skip_short_tracks_secs: Option<u32>, // "do not crossfade ≤ N seconds"
}
```

### Crossfade State Machine

```
Idle
  └─ track loaded on outgoing deck
     → Playing(outgoing)
          ├─ AutoDetect: monitor RMS; when drops below auto_detect_db → Crossfading
          ├─ Fixed: at (duration - fixed_crossfade_ms) → Crossfading
          └─ Manual: trigger_crossfade() command → Crossfading
               ↓
          Crossfading { outgoing, incoming, progress: 0.0→1.0 }
          │  per-sample: outgoing_gain = fade_out_curve(progress)
          │              incoming_gain = fade_in_curve(progress)
          │  progress advances by (1 / (sample_rate * fade_time_secs))
               ↓
          Playing(incoming)  [outgoing deck released]
```

---

## Audio Mixer Pipeline (`audio/dsp/pipeline.rs`)

Maps to SAM's "Audio Mixer Pipeline" screenshot:

```
Sources (5 channels)
  Deck A   → [EQ] → [AGC] → [DSP/Comp] ─┐
  Deck B   → [EQ] → [AGC] → [DSP/Comp] ─┤
  Sound FX → [EQ] → [AGC] → [DSP/Comp] ─┼──► [Mixer] → [EQ] → [AGC] → [DSP/Comp] ──► Air Out
  Aux 1    → [EQ] → [AGC] → [DSP/Comp] ─┤                                           └──► Encoder
  Voice FX → [EQ] → [AGC] → [DSP/Comp] ─┘
```

`ChannelPipeline` struct holds per-channel settings:

```rust
pub struct ChannelPipeline {
    pub eq: ChannelEQ,          // 3-band biquad (low shelf, mid peak, high shelf)
    pub agc: GatedAGC,          // enabled/disabled, gated AGC with configurable params
    pub compressor: Option<Compressor>, // optional per-channel compressor
    pub gain_db: f32,           // manual gain trim (-6 to +6 dB)
    pub enabled: bool,
}
```

---

## DSP Components

### EQ (`dsp/eq.rs`) — using `biquad` crate
3-band per channel (matches SAM Equalizer tab). Full parametric EQ on master bus.
- Low shelf filter (gain, frequency)
- Parametric mid (gain, frequency, Q)
- High shelf filter (gain, frequency)
- Coefficients recalculated only on parameter change (not per-sample)

### Gated AGC (`dsp/agc.rs`) — custom Rust implementation
Matches SAM's AGC panel (Gated AGC, Gate slider, Max gain, Pre-emphasis 50uS/75uS):
- RMS measurement over sliding window (~100ms)
- Noise gate: if RMS < gate_threshold, hold gain (don't amplify noise)
- Smoothed gain adjustment with configurable attack/release time constants
- Pre-emphasis filter: shelving filter at 3.18kHz (50μs) or 2.12kHz (75μs)
- Bass EQ section: peaking filter at 60Hz (configurable)
- Stereo expander: mid-side processing (Level, Depth, Threshold params)

### 5-Band Multiband Compressor (`dsp/compressor.rs`) — using `biquad` for crossovers
Matches SAM's "5 Bands processor" section:
- 5 crossover pairs split the signal into bands 1–5
- Per-band: Ratio, Threshold, Attack, Release, Hold, Band Gain
- Mode: Compressor / Expander / Limiter per band (toggle)
- Overdrive gain input
- Band link controls (1→2, 4→3, 5→4)
- Dual-band (LF/HF) variant with same controls
- Clipper: soft/hard clip at configurable level (+6 dB default)
- Output limiter: brickwall at 0dBFS

---

## Local Data Storage (`db/local.rs`)

SQLite via sqlx for app-local data (not in SAM MySQL):

```sql
-- Per-song cue points (maps to SAM Settings tab: Start/End/Intro/Outro/Fade/XFade + custom)
CREATE TABLE cue_points (
    id INTEGER PRIMARY KEY,
    song_id INTEGER NOT NULL,
    name TEXT NOT NULL,          -- 'start','end','intro','outro','fade','xfade','custom_0'...'custom_9'
    position_ms INTEGER NOT NULL,
    UNIQUE(song_id, name)
);

-- Per-song fade overrides (SAM's per-song "Fading" tab — overrides global crossfade config)
CREATE TABLE song_fade_overrides (
    song_id INTEGER PRIMARY KEY,
    fade_out_enabled INTEGER,    -- NULL = inherit from global CrossfadeConfig
    fade_out_curve TEXT,
    fade_out_time_ms INTEGER,
    fade_in_enabled INTEGER,
    fade_in_curve TEXT,
    fade_in_time_ms INTEGER,
    crossfade_mode TEXT,
    gain_db REAL                 -- Gap killer / Gain from SAM Settings tab
);

-- Per-channel DSP settings (persists EQ/AGC/Compressor params)
CREATE TABLE channel_dsp_settings (
    channel TEXT PRIMARY KEY,    -- 'deck_a','deck_b','sound_fx','aux_1','voice_fx','mixer','output'
    eq_low_gain_db REAL DEFAULT 0.0,
    eq_low_freq_hz REAL DEFAULT 100.0,
    eq_mid_gain_db REAL DEFAULT 0.0,
    eq_mid_freq_hz REAL DEFAULT 1000.0,
    eq_mid_q REAL DEFAULT 0.707,
    eq_high_gain_db REAL DEFAULT 0.0,
    eq_high_freq_hz REAL DEFAULT 8000.0,
    agc_enabled INTEGER DEFAULT 0,
    agc_gate_db REAL DEFAULT -31.0,
    agc_max_gain_db REAL DEFAULT 5.0,
    agc_attack_ms REAL DEFAULT 100.0,
    agc_release_ms REAL DEFAULT 500.0,
    agc_pre_emphasis TEXT DEFAULT '75us',
    comp_enabled INTEGER DEFAULT 0,
    comp_settings_json TEXT       -- serialized band settings
);

-- Global crossfade config (single row, id=1)
CREATE TABLE crossfade_config (
    id INTEGER PRIMARY KEY DEFAULT 1,
    config_json TEXT NOT NULL     -- serialized CrossfadeConfig
);
```

The `songlist.xfade` field already exists in SAM MySQL (varchar 50). **Do not overwrite it** — it's SAM's own format and may be used by SAM if running in parallel. Store crossfade overrides in local SQLite only.

---

## Icecast Streaming (`stream/icecast.rs`)

No Liquidsoap. Rust encodes PCM → MP3 and streams via HTTP PUT (Icecast source protocol):

```
Master output ring buffer
  └─ Encoder thread:
       ├─ Read PCM chunks from ring buffer
       ├─ Encode to MP3 via lame-sys (LGPL, dynamically linked)
       └─ HTTP PUT to http://host:port/mount
            Headers: Authorization: Basic base64(source:password)
                     Content-Type: audio/mpeg
                     Ice-Name, Ice-Genre, Ice-Bitrate
            Body: streaming chunked MP3 bytes
```

Encoder thread is separated from audio thread via `ringbuf` — audio engine writes, encoder reads, no blocking on real-time thread.

---

## Tauri IPC Surface (`commands/`)

Key commands exposed to the React frontend (Phase 1 focus):

```typescript
// Deck control
invoke('load_track', { deck, filePath, songId })
invoke('play_deck', { deck })
invoke('pause_deck', { deck })
invoke('stop_deck', { deck })
invoke('seek_deck', { deck, positionMs })
invoke('set_deck_volume', { deck, volume })   // 0.0–1.0

// Crossfade
invoke('get_crossfade_config') → CrossfadeConfig
invoke('set_crossfade_config', { config: CrossfadeConfig })
invoke('trigger_crossfade')   // manual
invoke('get_fade_curve_preview', { curve, timeMsOut, timeMsIn })  // returns [{t,gainOut,gainIn}] for graph

// DSP per channel
invoke('get_channel_dsp', { channel }) → ChannelDspSettings
invoke('set_channel_eq', { channel, lowGainDb, midGainDb, highGainDb, ... })
invoke('set_channel_agc', { channel, enabled, gateDb, maxGainDb, ... })
invoke('set_channel_compressor', { channel, enabled, bandsJson })

// Cue points
invoke('get_cue_points', { songId }) → CuePoint[]
invoke('set_cue_point', { songId, name, positionMs })
invoke('delete_cue_point', { songId, name })

// Streaming
invoke('start_stream', { host, port, mount, password, bitrateKbps })
invoke('stop_stream')
invoke('get_stream_status') → { connected, listeners, bitrate }

// Events (Rust → Frontend via listen())
'deck_state_changed'     // { deck, state, positionMs, durationMs }
'crossfade_progress'     // { progress, outgoingDeck, incomingDeck }
'vu_meter'               // { channel, leftDb, rightDb } at 80ms interval
'stream_connected'       // { mount }
'stream_disconnected'    // { reason }
```

---

## What is Achievable vs. What Needs More Research

| Feature (from SAM screenshots) | Phase 1 Status | Notes |
|-------------------------------|---------------|-------|
| Fade curves: Linear, Exponential, S-Curve | ✅ Full | Pure math |
| Auto-detect crossfade (dB trigger) | ✅ Full | RMS analysis on real-time thread |
| Fixed crossfade point | ✅ Full | Timing-based trigger |
| Per-song fade overrides (Fading tab) | ✅ Full | Local SQLite |
| 3-band EQ per channel | ✅ Full | biquad crate |
| Gated AGC with pre-emphasis | ✅ Full | Custom Rust |
| Stereo expander | ✅ Full | Mid-side processing |
| 5-band multiband compressor | ✅ Full | Custom via biquad crossovers |
| Dual-band processor (LF/HF) | ✅ Full | Subset of multiband |
| Clipper | ✅ Full | Simple amplitude clip |
| Mixer pipeline routing diagram | 🔶 Phase 2 | UI only; engine exists |
| Cue points (Start/End/Intro/Outro/Fade/XFade) | ✅ Full | Local SQLite |
| BPM tap / auto-detect | 🔶 Phase 2 | Need to confirm aubio-rs license |
| Gap killer | ✅ Full | Silence trim during decode |
| Direct Icecast streaming (no Liquidsoap) | ✅ Full | HTTP PUT + lame-sys |
| ASIO on Windows | ✅ Full | CPAL `asio` feature flag |
| VU meters (real, not simulated) | ✅ Full | RMS from audio engine via events |
| Fade curve preview graph | ✅ Full | `get_fade_curve_preview` command |

---

## Docs Folder

The plan file lives at `docs/phase1-audio-engine.md` in the repo root so any AI agent (Claude, Gemini, Copilot, etc.) picking up the project can read architecture decisions without context from this conversation.

Update the repo structure to include:
```
desizone-desktop/
├── docs/
│   ├── phase1-audio-engine.md     ← copy of this plan
│   ├── phase2-ui-dialogs.md       ← (future: CrossfadeSettings, AudioPipeline, EQ/AGC UI)
│   └── adr/                       ← Architecture Decision Records (ADRs)
│       └── 001-no-liquidsoap.md   ← Why Liquidsoap was dropped
...
```

---

## Subagent Usage for Implementation

Subagents can and should be used to parallelize implementation work. Recommended split:

| Task | Agent Type | Model |
|------|-----------|-------|
| Scaffold repo, `cargo add` deps, create directory structure | `Bash` | haiku |
| Write boilerplate `mod.rs`, `state.rs`, `main.rs` | `Bash` | haiku |
| Implement `crossfade.rs` (pure math, self-contained) | `general-purpose` | sonnet |
| Implement `dsp/eq.rs` (biquad wrappers) | `general-purpose` | haiku |
| Implement `dsp/agc.rs` (custom logic) | `general-purpose` | sonnet |
| Implement `dsp/compressor.rs` (multiband) | `general-purpose` | sonnet |
| Implement `audio/decoder.rs` (Symphonia) | `general-purpose` | sonnet |
| Implement `audio/engine.rs` (CPAL + mixer) | `general-purpose` | opus |
| Implement `stream/icecast.rs` | `general-purpose` | sonnet |
| Set up SQLite schema + migrations | `Bash` | haiku |
| Write Tauri command handlers | `general-purpose` | haiku |
| Write `src/lib/bridge.ts` IPC wrappers | `general-purpose` | haiku |
| Run `cargo build` and fix compile errors | `Bash` | sonnet |

**Parallelisation opportunities (can run simultaneously):**
- `crossfade.rs` + `dsp/eq.rs` + `dsp/agc.rs` — independent modules, no shared state
- `db/local.rs` schema setup + `stream/icecast.rs` — independent of audio engine
- `commands/*.rs` boilerplate + `bridge.ts` — once module signatures are defined

---

## Initialisation Steps

```bash
# 1. Create Tauri v2 project
npm create tauri-app@latest desizone-desktop
# choose: React + TypeScript frontend, Rust backend

# 2. Add Rust dependencies
cd desizone-desktop/src-tauri
cargo add cpal symphonia ringbuf biquad dasp sqlx serde serde_json tokio reqwest

# 3. Create docs folder and copy this plan
mkdir -p docs/adr
cp /Users/km53uh/.claude/plans/mutable-prancing-reddy.md docs/phase1-audio-engine.md

# 4. Scaffold module files (engine, decoder, crossfade, mixer, dsp/*)

# 5. Implement audio engine (engine.rs + deck.rs + decoder.rs)

# 6. Implement crossfade module (crossfade.rs)

# 7. Implement DSP pipeline (dsp/eq.rs, dsp/agc.rs, dsp/compressor.rs, dsp/pipeline.rs)

# 8. Set up SQLite schema (db/local.rs migrations)

# 9. Implement Tauri commands (commands/*.rs)

# 10. Wire frontend bridge (src/lib/bridge.ts)

# 11. Implement Icecast encoder (stream/icecast.rs)
```

---

## Verification (Phase 1)

1. **Audio engine plays audio:** Load an MP3 on Deck A via `load_track`, call `play_deck` → hear audio from soundcard.
2. **Crossfade fires automatically:** Load tracks on Deck A and B. Set auto-detect at -3dB. Track on Deck A fades out, Deck B fades in at the right level. `crossfade_progress` events fire with 0.0→1.0 progress.
3. **Fade curves correct:** Call `get_fade_curve_preview` for each curve type. Plot the returned `[{t, gainOut, gainIn}]` array — verify shapes match SAM's preview graph (S-curve should be smooth sigmoid, Exponential should be steep at start).
4. **EQ works:** Set mid gain to +6dB on Deck A — audible mid-boost. Set to -6dB — audible cut.
5. **AGC works:** Play a quiet track with AGC enabled → gain increases to hit target. Loud track → gain stays constrained by max_gain_db.
6. **Icecast stream:** Start stream → VLC connects to mount → plays audio. Stop stream → VLC disconnects.
7. **Cue points persist:** Set a `fade` cue point on song_id=1 → restart app → `get_cue_points` returns it.
8. **VU meter events:** Frontend listener for `vu_meter` receives events every ~80ms with real dBFS values (not simulated).
