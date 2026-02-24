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
- **Multi-Encoder Streaming** — Native HTTP PUT streaming to multiple Icecast/Shoutcast servers simultaneously
- **AutoPilot & Scheduling** — Automated playlist rotation, weekly calendar scheduling, and gap killer
- **Lua Scripting** — Extend functionality with custom scripts for automation and logic
- **Voice FX & Recording** — Microphone input with voice processing and voice track recording
- **DBE Gateway Integration** — Cloud connectivity for remote DJ control, real-time state sync, and live talk mode
- **SAM MySQL Compatibility** — Read/write SAM Broadcaster MySQL schema (`songlist`, `queuelist`, `historylist`, etc.)
- **Local SQLite Database** — Per-song cue points, per-channel DSP settings, crossfade configuration
- **Modern React UI** — TypeScript frontend with Vite bundler in Tauri webview

## Key Features by Phase

### Phase 1: Audio Engine ✅
- Dual-deck playback with independent controls
- 5-channel mixer (Deck A/B, SFX, Aux, Voice FX)
- Per-channel DSP (EQ, AGC, Compressor)
- Advanced crossfading with 5 curve types
- Cue point system
- VU meter visualization

### Phase 2: Operator UI ✅
- Deck transport controls
- Waveform display
- Crossfade settings dialog
- Channel DSP dialog
- Audio pipeline diagram
- Queue & library panels

### Phase 3: Automation & Scheduling ✅
- Weekly calendar scheduler
- Rotation rules engine
- Request policy system
- GAP killer (silence detection)
- Playlist management

### Phase 4: Streaming & Encoders ✅
- Multi-encoder support (multiple streams)
- Recording to file
- Listener statistics
- Metadata push to streams
- Stream status monitoring

### Phase 5: Scripting & Advanced Audio ✅
- Lua scripting engine
- Voice FX strip with effects
- Microphone input
- Voice track recorder
- Script library management

### Phase 6: DBE Gateway Integration ✅
- WebSocket gateway connection
- Remote DJ control with granular permissions
- AutoPilot mode (rotation/queue/scheduled)
- Live talk mode with mix-minus
- Real-time state synchronization (queue, now playing, VU meters)
- Session logging and management

## Project Status

**Phase 1 (Audio Engine): Complete ✅**
- All 14 Rust modules implemented and tested
- Full DSP pipeline operational
- Crossfade state machine working
- Database schema defined

**Phase 2 (Operator UI): Complete ✅**
- Crossfade settings dialog
- EQ/AGC control panels
- Deck transport controls
- VU meter visualization
- Audio pipeline diagram

**Phase 3 (Automation & Scheduling): Complete ✅**
- Weekly calendar scheduler
- Rotation rules editor
- Request policy management
- GAP killer configuration

**Phase 4 (Streaming & Encoders): Complete ✅**
- Multi-encoder support
- Recording to file
- Listener statistics
- Metadata push

**Phase 5 (Scripting & Advanced Audio): Complete ✅**
- Lua scripting engine
- Voice FX strip
- Microphone input
- Voice track recorder

**Phase 6 (DBE Gateway Integration): Complete ✅**
- WebSocket gateway connection
- Remote DJ control with permissions
- AutoPilot mode
- Live talk with mix-minus
- Real-time state synchronization

**Phase 7 (Analytics & Operations): Complete ✅**
- Play history analytics
- Event log viewer
- System health monitoring
- Top songs tracking
- Listener statistics

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
- **sam_db_config** — SAM MySQL connection settings and credentials (stored locally; used for auto-connect on startup)

### MySQL (SAM Schema)
- **songlist** — Song library with full metadata (artist, title, album, BPM, ISRC, UPC, duration, etc.)
- **queuelist** — Broadcast queue ordered by `sortID`; entries are deleted (not flagged) when played
- **historylist** — Played songs with full metadata snapshot per play event

> See **[SAM Broadcaster Database Integration](#sam-broadcaster-database-integration)** below for full setup instructions.

## SAM Broadcaster Database Integration

DesiZone Broadcaster reads and writes the **SAM Broadcaster Pro MySQL database** directly, giving it full access to your existing song library, queue, and play history without any migration or data export.

### How It Works

- Connects to SAM's MySQL schema (`samdb` by default) and reads/writes the same tables SAM Pro uses
- Your existing songs in `songlist` are immediately available for search and queuing
- When a track completes, the app removes it from `queuelist` and writes a full metadata snapshot to `historylist` — exactly as SAM does
- Runs alongside the local SQLite database; the two are fully independent
- If the SAM DB is unavailable, the app continues working — SAM features show "not connected" until you reconnect

### Prerequisites

- SAM Broadcaster Pro (or compatible) MySQL database accessible on your network or `localhost`
- Default SAM port: `3306` | Default database name: `samdb`
- MySQL credentials (SAM typically uses `sabroadcaster` / `sabroadcaster` by default)

### Connecting the SAM Database

#### Via the Settings UI

Open **Settings → SAM Database** and fill in the connection form:

| Field | Description | Default |
|-------|-------------|---------|
| **Host** | MySQL server hostname or IP | `127.0.0.1` |
| **Port** | MySQL port | `3306` |
| **Username** | MySQL username | `sabroadcaster` |
| **Password** | MySQL password | `sabroadcaster` |
| **Database** | Database name | `samdb` |
| **Auto-Connect** | Reconnect automatically on every app launch | `false` |
| **Path Prefix From** | Windows path root to replace (optional) | `C:\Music\` |
| **Path Prefix To** | Local path root to use instead (optional) | `/Volumes/Music/` |

Click **Test Connection** first to verify credentials without saving, then **Connect**.

#### Via Tauri IPC (programmatic)

```typescript
import {
  testSamDbConnection, connectSamDb, disconnectSamDb,
  getSamDbStatus, getSamDbConfig, saveSamDbConfig, getSamCategories
} from './lib/bridge'

// Test credentials without connecting or saving
const result = await testSamDbConnection({
  host: '127.0.0.1', port: 3306,
  username: 'sabroadcaster', password: 'sabroadcaster',
  database: 'samdb', auto_connect: false,
})
// result.connected === true means credentials are valid

// Connect and persist config (password saved in local SQLite)
await connectSamDb({
  host: '127.0.0.1', port: 3306,
  username: 'sabroadcaster', password: 'sabroadcaster',
  database: 'samdb',
  auto_connect: true,          // reconnect automatically on next launch
  path_prefix_from: 'C:\\Music\\',   // optional Windows→local path mapping
  path_prefix_to: '/Volumes/Music/', // optional
})

// Check live connection status
const { connected, host, database, error } = await getSamDbStatus()

// Disconnect at runtime (no restart required)
await disconnectSamDb()

// Read saved config (password excluded)
const cfg = await getSamDbConfig()

// Browse SAM categories
const categories = await getSamCategories()
```

### Auto-Connect on Startup

When **Auto-Connect** is enabled:

1. Connection credentials are saved to the **local SQLite** database (`sam_db_config` table)
2. On every app launch, DesiZone Broadcaster attempts to reconnect **before the UI renders**
3. If the SAM DB is unreachable (server offline, network issue), the app starts normally — a warning is printed to the console and SAM-dependent features degrade gracefully
4. You can manually reconnect at any time via Settings or `connectSamDb()` — no restart required

Look for `[startup] SAM DB auto-connected` or `[startup] SAM DB auto-connect failed` in the application console to diagnose startup issues.

### Windows Path Translation

SAM Broadcaster stores Windows absolute paths in `songlist.filename`, for example:

```
C:\Music\Bollywood\Artist - Song.mp3
```

If you run DesiZone Broadcaster on **macOS or Linux**, or if the music library is mounted under a different root, configure path translation so the app can locate the files:

| Setting | Example value |
|---------|--------------|
| **Path Prefix From** (Windows root) | `C:\Music\` |
| **Path Prefix To** (local mount) | `/Volumes/NAS/Music/` |

The substitution is applied automatically to all `filename` fields returned from SAM queries (song search, queue, history). Leave both fields empty if your paths are already correct on the local system.

### Queue & History Workflow

DesiZone Broadcaster follows the **exact same semantics as SAM Broadcaster Pro**:

| Step | What happens |
|------|-------------|
| **Load queue** | `SELECT … FROM queuelist ORDER BY sortID ASC` |
| **Add to queue** | `INSERT INTO queuelist (songID, sortID, …)` — `sortID` auto-appended after current tail |
| **Remove from queue** | `DELETE FROM queuelist WHERE ID = ?` |
| **Complete a track** | Atomically deletes from `queuelist` **and** inserts full metadata into `historylist` |

> SAM does **not** use a "played" flag — queue entries are physically deleted when played. `complete_queue_item` mirrors this exactly.

```typescript
import { getQueue, addToQueue, removeFromQueue, completeQueueItem, getHistory } from './lib/bridge'

// Load queue
const queue = await getQueue()          // QueueEntry[], sorted by sortId

// Add a song
await addToQueue({ songId: 12345 })

// After playback completes
await completeQueueItem(queue[0].id, queue[0].songId)  // delete + history

// Browse play history
const history = await getHistory(50)    // last 50 entries
```

### SAM DB IPC Reference

#### Connection Management
```
invoke('test_sam_db_connection', { args }) → SamDbStatus
invoke('connect_sam_db',         { args }) → SamDbStatus
invoke('disconnect_sam_db')               → void
invoke('get_sam_db_status')               → SamDbStatus
invoke('get_sam_db_config_cmd')           → SamDbConfig
invoke('save_sam_db_config_cmd', { config, password }) → void
invoke('get_sam_categories')              → SamCategory[]
```

#### Song Library & Queue
```
invoke('search_songs',       { query, categoryId, limit, offset }) → SamSong[]
invoke('get_queue')                                                 → QueueEntry[]
invoke('add_to_queue',       { songId })                           → void
invoke('remove_from_queue',  { queueId })                          → void
invoke('complete_queue_item',{ queueId, songId })                  → void
invoke('get_history',        { limit })                            → HistoryEntry[]
```

#### TypeScript Types (`src/lib/bridge.ts`)

```typescript
interface SamSong {
  id: number;             // songlist.ID
  filename: string;       // path (with prefix translation applied)
  songtype: string;       // 'S'=Song, 'J'=Jingle, 'N'=News, etc.
  artist: string; title: string; album: string; genre: string;
  albumyear: string; duration: number; bpm: number;
  xfade: string; mood: string; moodAi?: string;
  rating: number; countPlayed: number; datePlayed?: string;
  label: string; isrc: string; upc: string;
  picture?: string; overlay: string; weight: number;
}

interface QueueEntry {
  id: number;       // queuelist.ID
  songId: number;   // queuelist.songID
  sortId: number;   // queuelist.sortID (float ordering)
  requests: number; requestId: number;
  plotw: number;    // 0=Song(PLO), 1=VoiceBreak(TW)
  dedication: number;
}

interface SamDbConnectArgs {
  host: string; port: number; username: string; password: string;
  database: string; auto_connect: boolean;
  path_prefix_from?: string;   // optional Windows path root
  path_prefix_to?: string;     // optional local path root
}

interface SamDbStatus {
  connected: boolean;
  host: string | null;
  database: string | null;
  error: string | null;
}
```

### Docker Setup (Development / Testing)

A pre-seeded SAM MySQL container is available at `~/dz-docker-db`:

```bash
# Start the SAM MySQL container
cd ~/dz-docker-db
docker compose up -d

# Default connection details
Host:     127.0.0.1
Port:     3306
Database: samdb
User:     dbe_app
Password: (see ~/dz-docker-db/.env)
```

Use **Test Connection** in Settings to verify before connecting.

### SAM DB Troubleshooting

| Symptom | Resolution |
|---------|-----------|
| "SAM DB not connected" in queue/search | Open Settings → SAM Database → Connect |
| App starts without SAM DB | Normal — check console for `[startup]` messages; connect manually |
| Songs fail to load (file not found) | Configure Path Prefix From/To to map Windows paths to local mount |
| Special characters in password fail | Handled automatically (URL-encoded internally) — no action needed |
| Auto-connect worked last launch but not today | SAM MySQL server may be offline; check Docker or SAM Pro process |

---

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

### Gateway (Phase 6)
```
invoke('connect_gateway', { url, token })
invoke('disconnect_gateway')
invoke('get_gateway_status')
invoke('set_autopilot', { enabled, mode })
invoke('get_autopilot_status')
invoke('get_remote_sessions')
invoke('kick_remote_dj', { sessionId })
invoke('set_remote_dj_permissions', { sessionId, permissions })
invoke('start_live_talk', { channel })
invoke('stop_live_talk')
invoke('set_mix_minus', { enabled })
```

### SAM Database
```
invoke('test_sam_db_connection', { args }) → SamDbStatus
invoke('connect_sam_db',         { args }) → SamDbStatus
invoke('disconnect_sam_db')               → void
invoke('get_sam_db_status')               → SamDbStatus
invoke('get_sam_db_config_cmd')           → SamDbConfig
invoke('save_sam_db_config_cmd', { config, password }) → void
invoke('get_sam_categories')              → SamCategory[]
invoke('search_songs',    { query, categoryId, limit, offset }) → SamSong[]
invoke('get_queue')                       → QueueEntry[]
invoke('add_to_queue',    { songId })     → void
invoke('remove_from_queue',{ queueId })  → void
invoke('complete_queue_item',{ queueId, songId }) → void
invoke('get_history',     { limit })      → HistoryEntry[]
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
