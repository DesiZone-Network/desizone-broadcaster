# Phase 3 — Automation, Scheduling & Requests

## Goal
Make the station run itself. DJs should be able to walk away and have the station play content intelligently using rotation rules, show scheduling, and configurable request policies.

## DJ Operation Modes

Three modes, switchable from the top bar — persisted per session:

| Mode | Behaviour |
|------|-----------|
| **AutoDJ** | Station runs fully automated. Rotation rules select next track. Requests auto-accepted per policy. Shows fire on schedule. |
| **Assisted / Queue** | DJ loads queue manually. AutoDJ fills gaps when queue is empty. DJ can interrupt. |
| **Manual** | Nothing plays unless DJ actively controls decks. AutoDJ disabled. |

Mode switch is non-destructive — switching from Manual to AutoDJ picks up from current state without interruption.

---

## 3.1 Playlist Rotation Rules (`db/local.rs` + `src-tauri/src/scheduler/rotation.rs`)

### Rule Types

```rust
pub enum RotationRule {
    // Don't play same artist within N songs
    ArtistSeparation { min_songs: u32 },

    // Don't play same artist within N minutes
    ArtistSeparationTime { min_minutes: u32 },

    // Don't repeat same song within N songs
    SongSeparation { min_songs: u32 },

    // Don't repeat same song within N minutes
    SongSeparationTime { min_minutes: u32 },

    // Don't repeat same album within N songs
    AlbumSeparation { min_songs: u32 },

    // Category rotation: cycle through categories in order
    CategoryRotation { sequence: Vec<String> },

    // Energy level rotation (requires BPM/energy metadata)
    EnergyRotation { pattern: Vec<EnergyLevel> },  // e.g. High, High, Low, Medium

    // Maximum plays per song per N hours
    MaxPlaysPerHour { song_id: i64, max: u32, window_hours: u32 },
}
```

### Rotation Engine

The rotation engine selects the next track when AutoDJ needs it:

1. Fetch candidate songs from `songlist` matching active category/playlist
2. Filter out songs violating any active rotation rules (check against `historylist`)
3. Score remaining candidates (prefer less-recently-played, weight by energy level if enabled)
4. Return highest-scoring candidate

### SQLite Schema (new tables)

```sql
CREATE TABLE rotation_rules (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    rule_type TEXT NOT NULL,    -- 'artist_separation', 'song_separation', etc.
    config_json TEXT NOT NULL,  -- serialized rule parameters
    enabled INTEGER DEFAULT 1,
    priority INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE rotation_playlists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    is_active INTEGER DEFAULT 0,
    config_json TEXT NOT NULL   -- { categories: [], rules: [], shuffle: bool }
);

CREATE TABLE playlist_songs (
    playlist_id INTEGER NOT NULL,
    song_id INTEGER NOT NULL,
    position INTEGER,
    weight REAL DEFAULT 1.0,    -- higher weight = more likely to be selected
    PRIMARY KEY (playlist_id, song_id)
);
```

### UI (`src/components/automation/`)
- `RotationRulesEditor.tsx` — list of rules with enable/disable toggles, add/edit/remove
- `PlaylistEditor.tsx` — create playlists, assign categories, drag-reorder songs
- Rule editor modal: per rule type, show relevant controls (min songs, min minutes, category sequence)

---

## 3.2 Show Scheduler (`src-tauri/src/scheduler/show_scheduler.rs`)

### Data Model (SAM-compatible — writes to SAM MySQL `shows`, `events`, `eventtime`)

```sql
-- Reads/writes SAM tables directly for compatibility:
-- shows: id, name, start_type, start_time, ...
-- events: show_id, song_id, ...  
-- eventtime: event_id, day_of_week, start_time, duration
```

### Show Actions

Each show can trigger:
- `PlayPlaylist { playlist_id }` — switch AutoDJ to a specific playlist
- `PlaySong { song_id }` — play specific song immediately
- `StartStream { encoder_id }` — start/switch encoder
- `StopStream { encoder_id }` — stop encoder
- `SetVolume { channel, volume }` — fade down/up a channel
- `RunScript { script_id }` — execute a script (Phase 5)
- `SwitchMode { mode }` — switch DJ mode
- `PlayJingle { song_id }` — interrupt and play jingle, then resume

### Scheduler Engine
- Runs as a background Tokio task
- Checks schedule every second against current time
- Fires `ShowTriggered` event to audio engine
- Supports one-time shows and recurring (weekly schedule)

### UI (`src/components/scheduler/`)
- `WeeklyCalendar.tsx` — 7-column grid showing scheduled shows, drag to reschedule
- `ShowEditor.tsx` — create/edit shows: name, days, start time, duration, actions list
- `EventLog.tsx` — shows past show triggers with status (fired/skipped/error)

---

## 3.3 GAP Killer Configuration

The GAP killer detects and skips silence in tracks. Configuration at two levels:

### Global GAP Killer (`crossfade_config` table extended)

```sql
ALTER TABLE crossfade_config ADD COLUMN gap_killer_mode TEXT DEFAULT 'smart';
-- 'off'    = never trim silence
-- 'smart'  = trim leading/trailing silence only
-- 'aggressive' = trim any gap > 200ms within track
ALTER TABLE crossfade_config ADD COLUMN gap_killer_threshold_db REAL DEFAULT -50.0;
ALTER TABLE crossfade_config ADD COLUMN gap_killer_min_silence_ms INTEGER DEFAULT 500;
```

### Per-Song Override (in `song_fade_overrides`)
```sql
ALTER TABLE song_fade_overrides ADD COLUMN gap_killer TEXT;  -- NULL = inherit global
```

### Rust Implementation (`audio/decoder.rs`)
During decode, the decoder can:
1. **Trim leading silence**: skip samples below threshold at start of track
2. **Trim trailing silence**: detect when track goes silent and trigger crossfade early
3. **Fill gap**: if a gap is detected mid-track (e.g. hidden track gap), jump over it

### UI
- Gap killer settings card inside `CrossfadeSettingsDialog` (additional section)
- Per-song gap killer override in Song Information Editor → Settings tab

---

## 3.4 Request Policy Engine (`src-tauri/src/scheduler/request_policy.rs`)

### Policy Rules

```rust
pub struct RequestPolicy {
    // Song-level limits
    pub max_requests_per_song_per_day: u32,         // default: 3
    pub min_minutes_between_same_song: u32,          // default: 60

    // Artist-level limits
    pub max_requests_per_artist_per_hour: u32,       // default: 2
    pub min_minutes_between_same_artist: u32,        // default: 30

    // Album-level limits
    pub max_requests_per_album_per_day: u32,         // default: 5

    // Requester limits
    pub max_requests_per_requester_per_day: u32,     // default: 5
    pub max_requests_per_requester_per_hour: u32,    // default: 2

    // Queue position: where does accepted request go?
    pub queue_position: RequestQueuePosition,
    // After(n) = insert after n-th position, End = append, Next = after current

    // Blacklist
    pub blacklisted_song_ids: Vec<i64>,
    pub blacklisted_categories: Vec<String>,

    // Hours when requests are accepted (e.g. only 8am-10pm)
    pub active_hours: Option<(u8, u8)>,  // (start_hour, end_hour) 24h

    // Auto-accept if all policy checks pass
    pub auto_accept: bool,
}
```

### SQLite Schema

```sql
CREATE TABLE request_policy (
    id INTEGER PRIMARY KEY DEFAULT 1,
    policy_json TEXT NOT NULL
);

CREATE TABLE request_log (
    id INTEGER PRIMARY KEY,
    song_id INTEGER NOT NULL,
    requester_name TEXT,
    requester_ip TEXT,
    requested_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    status TEXT DEFAULT 'pending',   -- 'pending','accepted','rejected','played'
    rejection_reason TEXT,
    played_at DATETIME
);
```

### UI (`src/components/requests/`)
- `RequestPolicyEditor.tsx` — form for all policy fields grouped by category
- `RequestsPanel.tsx` (Phase 2 panel extended) — shows rejection reason on hover, policy summary badge
- `RequestHistory.tsx` — searchable log of all requests with status

---

## 3.5 New Tauri Commands (Phase 3)

```typescript
// Rotation
invoke('get_rotation_rules') → RotationRule[]
invoke('save_rotation_rule', { rule }) → id
invoke('delete_rotation_rule', { id }) → void
invoke('get_next_autodj_track') → SongResult  // preview what AutoDJ would pick next

// Playlists
invoke('get_playlists') → Playlist[]
invoke('save_playlist', { playlist }) → id
invoke('set_active_playlist', { playlistId }) → void

// Mode
invoke('set_dj_mode', { mode: 'autodj' | 'assisted' | 'manual' }) → void
invoke('get_dj_mode') → DjMode

// Shows
invoke('get_shows') → Show[]
invoke('save_show', { show }) → id
invoke('delete_show', { id }) → void
invoke('get_upcoming_events', { hours: 24 }) → ScheduledEvent[]

// GAP Killer
invoke('set_gap_killer_config', { config }) → void
invoke('get_gap_killer_config') → GapKillerConfig

// Requests
invoke('get_request_policy') → RequestPolicy
invoke('set_request_policy', { policy }) → void
invoke('get_requests', { status }) → Request[]
invoke('accept_request', { id }) → void
invoke('reject_request', { id, reason }) → void
invoke('get_request_history', { limit, offset }) → RequestLog[]
```

## New Rust Module

Add `src-tauri/src/scheduler/` with:
- `mod.rs`
- `rotation.rs` — rotation engine (track selection algorithm)
- `show_scheduler.rs` — Tokio task for show timing
- `request_policy.rs` — policy evaluation engine
- `autodj.rs` — AutoDJ mode controller (bridges rotation → deck commands)

## Acceptance Criteria

1. Switch to AutoDJ mode → station plays tracks continuously from rotation rules without any manual input
2. Set artist separation to 3 songs → verify in play history that same artist never appears within 3 positions
3. Create a show at HH:MM → it fires at that time and plays the configured playlist
4. Request a song → request policy correctly rejects if same requester already made 2 requests today
5. GAP killer set to Smart → track with 3 seconds of trailing silence crossfades into next track 3 seconds early
6. Weekly calendar shows all scheduled shows; drag to move a show updates its `eventtime` in DB
