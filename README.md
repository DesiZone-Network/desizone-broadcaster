# DesiZone Broadcaster

A modern **SAM Broadcaster Pro replacement** — a full-featured Tauri v2 desktop application for professional radio broadcast automation with local audio processing and Icecast streaming.

## What This Project Is

**DesiZone Broadcaster** is a cross-platform desktop application that provides comprehensive radio broadcast automation capabilities. It features:

- **Rust Audio Engine** — Built with CPAL for native soundcard I/O (WASAPI/ASIO on Windows, CoreAudio on macOS, ALSA on Linux) and Symphonia for low-latency audio decoding
- **Advanced Crossfading** — SAM-compatible fade curves (Linear, Exponential, S-Curve, Logarithmic, Constant Power) with auto-detection and customizable timing
- **Full Audio Mixer Pipeline** — 5-channel mixer (Deck A, Deck B, Sound FX, Aux 1, Voice FX) with per-channel processing:
  - 3-band parametric EQ
  - Gated AGC with pre-emphasis
  - 5-band multiband compressor + dual-band + clipper
- **Direct Icecast/Shoutcast Streaming** — Native HTTP PUT streaming with MP3 encoding (no Liquidsoap dependency)
- **SAM MySQL Compatibility** — Read/write SAM Broadcaster MySQL schema (`songlist`, `queuelist`, `historylist`, etc.)
- **Local SQLite Database** — Per-song cue points, per-channel DSP settings, crossfade configuration
- **Modern React UI** — TypeScript frontend with Vite bundler in Tauri webview

## Project Status

**Phase 1 (Audio Engine): Complete ✅**
- All 14 Rust modules implemented and tested
- Full DSP pipeline operational
- Crossfade state machine working
- Database schema defined

**Phase 2 (Frontend UI): In Progress**
- Crossfade settings dialog
- EQ/AGC control panels
- Deck transport controls
- VU meter visualization

## Requirements

### System Requirements
- **macOS 10.13+** (Intel or Apple Silicon)
- **Windows 10+** (Windows 7 SP1 supported by Tauri)
- **Linux** — Ubuntu 18.04+, Fedora 32+, Debian 9+ (ALSA development libraries required)

### Development Requirements

- **Node.js 18+** — for TypeScript/React tooling
- **Rust 1.70+** — for Tauri and audio engine compilation
- **Git** — for version control

#### macOS-specific
```bash
# Install Xcode Command Line Tools if needed
xcode-select --install
```

#### Windows-specific
- Visual Studio 2019 or later (Build Tools for Visual Studio 2019 minimum)
- Windows 10 SDK

#### Linux-specific
```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev libasound2-dev libx11-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora
sudo dnf install openssl-devel alsa-lib-devel libX11-devel libxcb-devel libxcb-render-devel libxcb-shape-devel libxcb-xfixes-devel
```

## Installation

### 1. Clone the Repository
```bash
git clone <repository-url>
cd "DesiZone Broadcaster"
```

### 2. Install Node Dependencies
```bash
npm install
```

### 3. First Build (Optional — automatic on first `tauri dev`)
```bash
cd src-tauri
cargo check
```
This performs a full dependency compilation (~10–15 minutes on first run). Subsequent builds are incremental.

## Running

### Development Mode
```bash
npm run tauri dev
```
This starts:
- **Vite dev server** — hot-reloading React frontend on `localhost:5173`
- **Cargo dev build** — compiles Rust code with debug symbols
- **Tauri window** — opens the application window with DevTools enabled

Press `Cmd+Shift+I` (macOS) or `Ctrl+Shift+I` (Windows/Linux) to open DevTools.

### Production Build
```bash
npm run tauri build
```
Builds an optimized, distributable application:
- **macOS**: `.dmg` installer and `.app` bundle in `src-tauri/target/release/bundle/dmg/`
- **Windows**: `.msi` installer in `src-tauri/target/release/bundle/msi/`
- **Linux**: `.deb` and `.AppImage` in `src-tauri/target/release/bundle/`

## Common Commands

```bash
# Rust only — compile check without window (fast)
cd src-tauri && cargo check

# Run Rust tests
cd src-tauri && cargo test

# Type-check TypeScript frontend
npm run typecheck

# Lint TypeScript
npm run lint
```

## Project Structure

```
DesiZone Broadcaster/
├── src/                          # React + TypeScript frontend
│   ├── components/               # React components
│   ├── lib/bridge.ts            # Typed Tauri IPC wrappers
│   ├── index.css                # Global styles
│   └── main.tsx                 # App entry point
├── src-tauri/                   # Tauri backend (Rust)
│   ├── src/
│   │   ├── audio/               # Audio engine module
│   │   │   ├── crossfade.rs    # Fade curves & state machine
│   │   │   ├── deck.rs         # Deck playback logic
│   │   │   ├── mixer.rs        # 5-channel mixer
│   │   │   ├── engine.rs       # CPAL stream & RT loop
│   │   │   └── dsp/            # EQ, AGC, Compressor
│   │   ├── db/                  # Database layer
│   │   │   ├── local.rs        # SQLite (cue points, DSP)
│   │   │   └── sam.rs          # MySQL (SAM schema)
│   │   ├── stream/              # Streaming
│   │   │   └── icecast.rs      # HTTP PUT to Icecast
│   │   ├── commands/            # Tauri IPC handlers
│   │   ├── state.rs            # App state
│   │   └── main.rs             # Tauri setup
│   ├── Cargo.toml              # Rust dependencies
│   └── tauri.conf.json         # Tauri config
├── docs/
│   ├── phase1-audio-engine.md  # Architecture & design
│   └── adr/                     # Architecture Decision Records
├── vite.config.ts              # Vite config
├── package.json
└── README.md
```

## Audio Architecture

### Real-Time Render Loop
```
CPAL Output Callback (every ~10ms @ 48kHz):
├─ For each channel (Deck A/B, SoundFX, Aux1, VoiceFX):
│  ├─ Read PCM from deck's ring buffer
│  ├─ Apply crossfade gain envelope
│  └─ Process through pipeline: EQ → AGC → Compressor
├─ Mix channels to master bus
├─ Apply master limiter
└─ Output to soundcard + Icecast encoder
```

### Crossfade Curves
All SAM Broadcaster curve types are supported with pure math (no external library):

| Curve | Formula (t: 0→1) |
|-------|-----------------|
| Linear | `1.0 - t` |
| Exponential | `(1.0 - t)²` |
| S-Curve | `0.5 × (1 + cos(π×t))` |
| Logarithmic | `log₁₀(1 + 9×(1−t)) / log₁₀(10)` |
| Constant Power | `cos(t × π/2)` out / `sin(t × π/2)` in |

## Technology Stack

| Component | Technology |
|-----------|-----------|
| **Desktop Framework** | Tauri v2 |
| **Frontend** | React 19 + TypeScript |
| **Build Tool** | Vite |
| **Audio I/O** | CPAL 0.15 |
| **Audio Decode** | Symphonia 0.5 (MP3, AAC, FLAC, OGG, WAV) |
| **DSP** | Biquad filters, DASP, custom Rust |
| **Ring Buffers** | ringbuf 0.4 (lock-free SPSC) |
| **Local DB** | SQLx 0.7 + SQLite |
| **SAM DB** | SQLx + MySQL |
| **HTTP Streaming** | Reqwest |

## Database Schema

### SQLite (Local Settings)
- **cue_points** — Per-song cue markers (Start, End, Intro, Outro, Fade, XFade, Custom 0–9)
- **song_fade_overrides** — Per-song crossfade settings (NULL = use global config)
- **channel_dsp_settings** — Per-channel EQ, AGC, compressor settings
- **crossfade_config** — Global crossfade defaults (Linear, time, levels)

### MySQL (SAM Schema)
- **songlist** — Song library (read-only in this app)
- **queuelist** — Broadcast queue (written by this app)
- **historylist** — Played songs (written by this app)

## Key Commands (Tauri IPC)

### Playback Control
```
invoke('load_track', { deck: 'deck_a', filePath, songId })
invoke('play_deck', { deck })
invoke('pause_deck', { deck })
invoke('seek_deck', { deck, positionMs })
```

### Crossfade
```
invoke('get_crossfade_config') → CrossfadeConfig
invoke('set_crossfade_config', { config })
invoke('get_fade_curve_preview', { curve, timeMsOut, timeMsIn })
```

### DSP
```
invoke('get_channel_dsp', { channel })
invoke('set_channel_eq', { channel, lowGainDb, midGainDb, highGainDb })
invoke('set_channel_agc', { channel, enabled, gateDb, maxGainDb })
```

### Events (listen from frontend)
```
listen('deck_state_changed')     // { deck, state, positionMs }
listen('crossfade_progress')     // { progress 0.0–1.0 }
listen('vu_meter')               // { channel, leftDb, rightDb }
listen('stream_connected')       // { mount }
```

## Troubleshooting

### First build takes 10–15 minutes
Normal behavior — Tauri, CPAL, Symphonia, and all dependencies compile from scratch. Subsequent builds are incremental.

### `cargo check` shows icon errors
Icon files must be RGBA PNG. Use `src-tauri/icon_gen.py` to regenerate.

### macOS: "App cannot be opened because it is from an unidentified developer"
Right-click the `.app` → **Open** → click **Open** button in security dialog.

### Windows: Missing WASAPI drivers
Ensure Windows Audio Service is running:
```bash
net start audiosrv
```

### Linux: ALSA "permission denied"
Add user to `audio` group:
```bash
sudo usermod -a -G audio $USER
# Then log out and log back in
```

## Documentation

- **[phase1-audio-engine.md](docs/phase1-audio-engine.md)** — Complete Phase 1 architecture, design decisions, all DSP algorithms
- **[adr/001-no-liquidsoap.md](docs/adr/001-no-liquidsoap.md)** — Why Liquidsoap was removed; native streaming approach

## Contributing

This project is actively developed. See [CLAUDE.md](CLAUDE.md) for internal development guidelines.

## License

TBD

## Support

For issues, feature requests, or questions:
- Check existing [GitHub Issues](https://github.com/DesiZone/Broadcaster/issues)
- Create a new issue with detailed reproduction steps
