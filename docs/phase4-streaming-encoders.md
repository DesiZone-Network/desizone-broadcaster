# Phase 4 — Streaming, Encoders & Listener Analytics

## Goal
Multiple simultaneous encoder outputs to Icecast/Shoutcast, stream-to-file recording, live listen output to local soundcard, and a real-time listener stats graph.

---

## 4.1 Multiple Encoder Outputs (`src-tauri/src/stream/`)

### Architecture

The master audio output bus feeds a **broadcaster** that distributes to N encoder instances simultaneously:

```
Master Audio Output Buffer (ring buffer)
  └─ Broadcaster (fan-out)
       ├─ Encoder 1: MP3 128kbps → Icecast 1 (main stream)
       ├─ Encoder 2: AAC+ 64kbps → Icecast 1 /mobile
       ├─ Encoder 3: MP3 320kbps → Icecast 2 (backup/archive)
       └─ Encoder 4: PCM WAV → File (recording)
```

Each encoder runs in its own Tokio task. The broadcaster ring buffer is a multi-consumer design.

### Encoder Config (`db/local.rs` — new table)

```sql
CREATE TABLE encoders (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,           -- "Main 128kbps", "Mobile AAC", "Archive"
    enabled INTEGER DEFAULT 1,
    
    -- Codec
    codec TEXT NOT NULL,          -- 'mp3' | 'aac' | 'ogg' | 'wav' | 'flac'
    bitrate_kbps INTEGER,         -- not used for wav/flac
    sample_rate INTEGER DEFAULT 44100,
    channels INTEGER DEFAULT 2,    -- 1=mono, 2=stereo
    quality INTEGER,               -- codec quality 0-9 (for VBR)
    
    -- Output type
    output_type TEXT NOT NULL,     -- 'icecast' | 'shoutcast' | 'file'
    
    -- Icecast/Shoutcast
    server_host TEXT,
    server_port INTEGER,
    server_password TEXT,
    mount_point TEXT,              -- /stream for Icecast
    stream_name TEXT,
    stream_genre TEXT,
    stream_url TEXT,               -- station website
    stream_description TEXT,
    is_public INTEGER DEFAULT 0,   -- list in Shoutcast/Icecast YP directory
    
    -- File output
    file_output_path TEXT,         -- directory for recordings
    file_rotation TEXT DEFAULT 'hourly',  -- 'none' | 'hourly' | 'daily' | 'by_size'
    file_max_size_mb INTEGER DEFAULT 500,
    file_name_template TEXT DEFAULT '{date}-{time}-{station}.mp3',
    
    -- Metadata
    send_metadata INTEGER DEFAULT 1,  -- push song title/artist to stream
    icy_metadata_interval INTEGER DEFAULT 8192,
    
    -- Reconnect
    reconnect_delay_secs INTEGER DEFAULT 5,
    max_reconnect_attempts INTEGER DEFAULT 0,  -- 0 = infinite
    
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Rust Encoder Implementation (`stream/`)

```
stream/
├── mod.rs
├── broadcaster.rs      — fan-out ring buffer to N encoder consumers
├── icecast.rs          — HTTP PUT source protocol (Icecast 2.x)
├── shoutcast.rs        — SHOUTcast v1/v2 protocol
├── encoder_mp3.rs      — LAME MP3 encoding (lame-sys, LGPL dynamic link)
├── encoder_aac.rs      — AAC encoding via fdkaac-sys (or fdk-aac Rust port)
├── encoder_ogg.rs      — Ogg Vorbis via vorbis-sys
├── encoder_file.rs     — PCM/WAV/FLAC file writer with rotation
├── metadata_pusher.rs  — Icy-MetaData title/artist injection
└── encoder_manager.rs  — manages N EncoderInstance, handles reconnect
```

### Encoder State Machine

```
Disabled ──enable──► Connecting ──connected──► Streaming
                          │                        │
                    connect_failed            disconnected
                          │                        │
                     Retrying ◄──────────────────────┘
                          │
                    max_retries_exceeded
                          │
                       Failed
```

---

## 4.2 Stream-to-File Recording (`stream/encoder_file.rs`)

- Writes master output to disk in configured codec (WAV, MP3, FLAC)
- File rotation: hourly, daily, or by size threshold
- File name template supports: `{date}`, `{time}`, `{datetime}`, `{station}`, `{bitrate}`, `{codec}`
- On rotation: closes current file, opens new file — no gap
- Backfill cue sheet: writes a `.cue` file alongside recording with track markers from `historylist`

---

## 4.3 Stream Metadata Push

When a track changes, push ICY metadata to all active Icecast/Shoutcast encoders:
- Format: `StreamTitle='{Artist} - {Title}';` (standard ICY)
- Icecast: `/admin/metadata?mount=/stream&mode=updinfo&song=Artist+-+Title`
- Shoutcast: `GET /admin.cgi?pass=...&mode=updinfo&song=...`
- Also update OGG/Vorbis `TITLE`/`ARTIST` comments mid-stream if supported

---

## 4.4 Live Listen Output

Separate from the Air monitoring (which is the main soundcard output). Live Listen is an additional output that can be:
- A second soundcard output device (e.g. headphones jack)
- Used by a remote listener connected to a local port

```rust
pub struct LiveListenOutput {
    pub device_name: Option<String>,  // None = default device
    pub volume: f32,
    pub channel_source: LiveListenSource,  // Master | Cue | DeckA | DeckB | Aux1
}
```

The audio engine maintains a secondary CPAL output stream for live listen, sourced from the selected channel independently of the main Air output.

---

## 4.5 Listener Stats — Real-Time Graph (`src-tauri/src/stats/`)

### Stats Collector (`stats/icecast_stats.rs`)

Poll each active Icecast/Shoutcast server's admin API every 30 seconds:

**Icecast:** `GET http://host:port/status-json.xsl` (or `/admin/stats`)
Returns: mount point listeners, peak listeners, stream bitrate, server version

**Shoutcast:** `GET http://host:port/statistics?json=1`
Returns: current listeners, peak, unique listeners, max listeners

Store in SQLite:
```sql
CREATE TABLE listener_snapshots (
    id INTEGER PRIMARY KEY,
    encoder_id INTEGER NOT NULL,
    snapshot_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    current_listeners INTEGER DEFAULT 0,
    peak_listeners INTEGER DEFAULT 0,
    unique_listeners INTEGER DEFAULT 0,
    stream_bitrate INTEGER,
    FOREIGN KEY (encoder_id) REFERENCES encoders(id)
);
```

### UI — Listener Graph (`src/components/stats/`)

- `ListenerGraph.tsx` — SVG/canvas line chart (rechartsJS or custom canvas)
  - X axis: time (last 1h / 6h / 24h / 7d selector)
  - Y axis: listener count
  - One line per encoder/mount, colour-coded
  - Hover tooltip showing exact count + timestamp
- `StreamStatusBar.tsx` — compact row in TopBar showing per-encoder: ● Connected / ● Disconnected / listener count
- `EncoderStatusCards.tsx` — full encoder status view: connection state, uptime, listeners, bitrate, bytes sent, reconnect history

---

## 4.6 Encoder Settings UI (`src/components/encoders/`)

- `EncoderList.tsx` — list of all configured encoders with status badges and quick enable/disable
- `EncoderEditor.tsx` — full editor dialog:
  - Tabs: General | Codec | Server | Metadata | Recording | Advanced
  - Server type toggle: Icecast / Shoutcast / File
  - Live connection test button: attempts connect and shows result
  - Codec preview: estimated bitrate / file size per hour

---

## 4.7 Tauri Commands (Phase 4)

```typescript
// Encoders
invoke('get_encoders') → Encoder[]
invoke('save_encoder', { encoder }) → id
invoke('delete_encoder', { id }) → void
invoke('start_encoder', { id }) → void
invoke('stop_encoder', { id }) → void
invoke('start_all_encoders') → void
invoke('stop_all_encoders') → void
invoke('test_encoder_connection', { id }) → { success: bool, error?: string }

// Recording
invoke('start_recording', { encoderId }) → void
invoke('stop_recording', { encoderId }) → void
invoke('get_recording_status', { encoderId }) → RecordingStatus

// Live Listen
invoke('get_live_listen_config') → LiveListenConfig
invoke('set_live_listen_config', { config }) → void
invoke('get_audio_output_devices') → AudioDevice[]  // list CPAL output devices

// Stats
invoke('get_listener_stats', { encoderId, period: '1h'|'6h'|'24h'|'7d' }) → ListenerSnapshot[]
invoke('get_current_listeners', { encoderId }) → number

// Events
listen('encoder_status_changed', handler)  // { id, status, listeners?, error? }
listen('listener_count_updated', handler)  // { encoderId, count } every 30s
listen('recording_rotation', handler)      // { encoderId, closedFile, newFile }
```

## Acceptance Criteria

1. Two encoders configured → both stream simultaneously to two different Icecast mounts → both playable in VLC
2. Stream-to-file encoder → recording file grows while playing → on hourly rotation, new file starts without audio gap
3. Metadata push → VLC or Winamp shows "Artist - Title" updating when track changes
4. Listener graph → shows real-time listener count updating every 30s; historical data shows last 24h
5. Live listen config → selecting second soundcard → audio playable on that device independently
6. Encoder disconnected → automatic reconnect fires after configured delay → status card shows "Retrying (2/5)"
7. Shoutcast AND Icecast both work: v1 Shoutcast protocol + Icecast 2.x HTTP source both tested
