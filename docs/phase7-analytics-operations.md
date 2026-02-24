# Phase 7 — Analytics & Operations

## Overview

Phase 7 adds the observability and operational layer: comprehensive play history analytics, listener statistics graphs, event log viewer, system health monitoring, and reporting. This phase transforms DesiZone Broadcaster from a playback tool into a full broadcast operations platform.

---

## Analytics Modules

### 1. Play History Analytics

Reads from SAM `historylist` MySQL table (already populated by Phase 1 audio engine) and the local SQLite `remote_sessions_log`.

**Metrics tracked:**
- Total plays per song (all time, last 7/30/90 days)
- Top 50 most played songs (by period)
- Genre/category breakdown
- Peak hours (plays per hour heatmap)
- Average song duration vs actual played duration (skip detection)
- Crossfade trigger counts per song

**New SQLite tables:**

```sql
-- Aggregated play stats cache (refreshed hourly, avoids repeated MySQL scans)
CREATE TABLE play_stats_cache (
    song_id INTEGER NOT NULL,
    period TEXT NOT NULL,          -- 'all', '7d', '30d', '90d'
    play_count INTEGER DEFAULT 0,
    total_played_ms INTEGER DEFAULT 0,
    last_played_at INTEGER,        -- Unix ms
    skip_count INTEGER DEFAULT 0,  -- played < 30s
    PRIMARY KEY (song_id, period)
);

-- Hourly play counts (for heatmap)
CREATE TABLE hourly_play_counts (
    date TEXT NOT NULL,            -- YYYY-MM-DD
    hour INTEGER NOT NULL,         -- 0–23
    play_count INTEGER DEFAULT 0,
    unique_songs INTEGER DEFAULT 0,
    PRIMARY KEY (date, hour)
);
```

### 2. Listener Statistics Graph

Polls Icecast/Shoutcast stats endpoint (per encoder from Phase 4) and stores snapshots.

**Uses `listener_snapshots` table (defined in Phase 4):**

```sql
-- Already defined in phase4 (shown here for reference)
CREATE TABLE listener_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    encoder_id INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    listener_count INTEGER NOT NULL,
    peak_listeners INTEGER,
    FOREIGN KEY (encoder_id) REFERENCES encoders(id)
);
```

**Graph types:**
- Real-time listener count (last 1 hour, updates every 30s)
- Daily peak vs average listener chart (bar + line combo)
- Per-encoder listener comparison (multi-line)
- Listener count at track change moments (correlate songs with audience)

**Charting library:** `recharts` (already available in React ecosystem, lightweight)

### 3. Request Analytics

Reads from SAM `requestlist` MySQL table.

**Metrics:**
- Most requested songs (by period)
- Request acceptance rate (approved / denied / expired)
- Requests by hour of day
- Top requesters (anonymised — by hash of IP or session)
- Request response time (submitted → played)

### 4. Event Log Viewer

A searchable, filterable log of all significant system events stored locally.

**New SQLite table:**

```sql
CREATE TABLE event_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,    -- Unix ms
    level TEXT NOT NULL,           -- 'info', 'warn', 'error', 'debug'
    category TEXT NOT NULL,        -- 'audio', 'stream', 'scheduler', 'gateway', 'scripting', 'database'
    event TEXT NOT NULL,           -- short event key e.g. 'track_loaded', 'stream_disconnected'
    message TEXT NOT NULL,
    metadata_json TEXT,            -- optional structured context
    deck TEXT,                     -- nullable: which deck (if applicable)
    song_id INTEGER,               -- nullable: which song (if applicable)
    encoder_id INTEGER             -- nullable: which encoder (if applicable)
);

CREATE INDEX idx_event_log_timestamp ON event_log(timestamp DESC);
CREATE INDEX idx_event_log_category ON event_log(category);
CREATE INDEX idx_event_log_level ON event_log(level);
```

**Events logged automatically (from all previous phases):**
| Category | Events |
|----------|--------|
| `audio` | `track_loaded`, `track_played`, `track_paused`, `track_ended`, `crossfade_started`, `crossfade_completed`, `cue_point_hit`, `deck_error` |
| `stream` | `stream_connected`, `stream_disconnected`, `stream_error`, `encoder_started`, `encoder_stopped`, `recording_started`, `recording_stopped` |
| `scheduler` | `show_started`, `show_ended`, `rotation_rule_applied`, `queue_empty`, `autopilot_activated`, `autopilot_deactivated` |
| `gateway` | `gateway_connected`, `gateway_disconnected`, `remote_dj_joined`, `remote_dj_left`, `remote_command_received` |
| `scripting` | `script_triggered`, `script_completed`, `script_error` |
| `database` | `mysql_connected`, `mysql_disconnected`, `sam_sync_error` |

### 5. System Health Monitor

**Real-time metrics panel:**
- CPU usage of audio engine thread (CPAL callback timing)
- Ring buffer fill levels per deck (early warning for buffer underrun)
- Decoder thread latency
- Memory usage
- MySQL connection pool status
- SQLite vacuum status
- Stream uptime / downtime ratio (last 24h)
- Encoder bitrate stability

**New SQLite table:**

```sql
CREATE TABLE system_health_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    cpu_pct REAL,
    memory_mb REAL,
    ring_buffer_fill_deck_a REAL,   -- 0.0–1.0
    ring_buffer_fill_deck_b REAL,
    decoder_latency_ms REAL,
    stream_connected INTEGER,
    mysql_connected INTEGER,
    active_encoders INTEGER
);
```

**Alerting (desktop notifications via Tauri):**
- Ring buffer < 10% → "Buffer underrun risk on Deck A"
- Stream disconnected for > 30s → "Icecast stream down"
- MySQL unreachable → "SAM database connection lost"
- Disk space for recordings < 1 GB → "Recording disk space low"

### 6. Reports

**Export formats:** CSV, PDF (via `printable` HTML page in webview)

**Report types:**
- **Daily Broadcast Report**: shows summary, top songs, listener peaks, events
- **Song Play History**: all plays of a specific song with timestamps
- **Listener Trend Report**: weekly/monthly listener graph with statistics
- **Request Log Report**: all requests for a period with accept/deny status
- **Stream Uptime Report**: uptime %, disconnection events, duration

---

## New Module: `src-tauri/src/analytics/`

```
src-tauri/src/analytics/
├── mod.rs
├── play_stats.rs     # Aggregates historylist data into play_stats_cache
├── listener_stats.rs # Polls Icecast XML stats endpoint per encoder
├── event_logger.rs   # Writes to event_log SQLite table from all modules
├── health_monitor.rs # Collects system metrics on a background Tokio task
└── reports.rs        # Generates report data structs for frontend rendering
```

### Event Logger API (used internally by all phases)

```rust
// Called from anywhere in the Rust backend
pub fn log_event(
    pool: &SqlitePool,
    level: LogLevel,
    category: EventCategory,
    event: &str,
    message: &str,
    metadata: Option<serde_json::Value>,
    deck: Option<&str>,
    song_id: Option<i64>,
    encoder_id: Option<i64>,
)
```

---

## New Tauri Commands (`commands/analytics_commands.rs`)

```typescript
// Play history
invoke('get_top_songs', { period: '7d' | '30d' | '90d' | 'all', limit: number })
  → TopSong[]  // { songId, title, artist, playCount, totalPlayedMs }

invoke('get_hourly_heatmap', { startDate: string, endDate: string })
  → HeatmapData[]  // { date, hour, playCount }

invoke('get_song_play_history', { songId: number, limit: number })
  → PlayHistoryEntry[]

// Listener stats
invoke('get_listener_graph', { encoderId: number, period: '1h' | '24h' | '7d' })
  → ListenerSnapshot[]  // { timestamp, listenerCount }

invoke('get_listener_peak', { encoderId: number, period: string })
  → { peak: number, average: number, timestamp: number }

// Event log
invoke('get_event_log', {
  limit: number,
  offset: number,
  level?: string,
  category?: string,
  startTime?: number,
  endTime?: number,
  search?: string
}) → { events: EventLogEntry[], total: number }

invoke('clear_event_log', { olderThanDays: number })

// System health
invoke('get_health_snapshot') → SystemHealthSnapshot
invoke('get_health_history', { periodMinutes: number }) → SystemHealthSnapshot[]

// Reports
invoke('generate_report', { type: ReportType, params: ReportParams }) → ReportData
invoke('export_report_csv', { reportData: ReportData }) → { filePath: string }
```

### New Events (Rust → Frontend)

```typescript
listen('event_logged',       (e) => EventLogEntry)    // real-time log stream
listen('health_updated',     (e) => SystemHealthSnapshot) // every 5s
listen('alert_triggered',    (e) => SystemAlert)       // buffer underrun, disconnect, etc.
```

---

## UI Components (Phase 7 Additions)

### Analytics Dashboard (new main nav section)

```
┌─────────────────────────────────────────────────────────────┐
│  📊 Analytics                              [Export ▼] [⚙]   │
├──────────┬──────────────────────────────────────────────────┤
│ Overview │  🎵 Top Songs    📡 Listeners   📋 Events         │
│ Play Stats│                                                  │
│ Listeners │   [period: 7d ▼]                                 │
│ Event Log │                                                  │
│ Health    │   ┌──────────────────────────────────────────┐   │
│ Reports   │   │  Top 10 Songs — Last 7 Days              │   │
│           │   │  1. Song Title — Artist (42 plays)       │   │
│           │   │  2. ...                                  │   │
│           │   └──────────────────────────────────────────┘   │
│           │                                                  │
│           │   ┌──────────────────────────────────────────┐   │
│           │   │  Hourly Play Heatmap                     │   │
│           │   │  Mon ██▓▒░░▓█████▓░░░░░                 │   │
│           │   │  Tue ██████▓▓▓▓▓▓▓▓░░░                  │   │
│           │   └──────────────────────────────────────────┘   │
└──────────┴──────────────────────────────────────────────────┘
```

### Listener Graph Panel

```
┌──────────────────────────────────────────────────────┐
│  📡 Listeners  [Encoder 1 ▼]  [24h ▼]               │
│                                                      │
│  Peak: 847  Average: 312  Now: 429                   │
│                                                      │
│   900 ┤                  ╭─╮                         │
│   600 ┤        ╭─╮      ╭╯  ╰─╮  ╭╮                 │
│   300 ┤  ╭─╮  ╭╯  ╰────╯     ╰──╯ ╰──               │
│     0 ┼──┴──┴──────────────────────────              │
│       00:00  06:00  12:00  18:00  24:00              │
└──────────────────────────────────────────────────────┘
```

### Event Log Panel

```
┌──────────────────────────────────────────────────────┐
│  📋 Event Log  [All ▼] [All ▼] [Search...]  [Clear]  │
├──────┬──────────┬──────────┬───────────────────────┤
│ Time │ Level    │ Category │ Message                │
├──────┼──────────┼──────────┼───────────────────────┤
│ 14:32│ 🟢 info  │ audio    │ track_loaded: Song XYZ │
│ 14:31│ 🔴 error │ stream   │ stream_disconnected... │
│ 14:30│ 🟡 warn  │ buffer   │ ring_buffer fill < 20% │
└──────┴──────────┴──────────┴───────────────────────┘
```

### System Health Panel

```
┌──────────────────────────────────────────────────────┐
│  💻 System Health                    [History 1h ▼]  │
│                                                      │
│  CPU: 12%  ████░░░░░░░░░░░░░░░░░░                   │
│  RAM: 245MB████████░░░░░░░░░░░░░░░                   │
│                                                      │
│  Deck A Buffer: 87%  ███████████████████░            │
│  Deck B Buffer: 92%  ████████████████████            │
│                                                      │
│  MySQL: ✅ Connected  SQLite: ✅ OK                  │
│  Stream: ✅ Live (4h 23m uptime)                    │
│  Encoders: 3/3 active                               │
└──────────────────────────────────────────────────────┘
```

---

## Acceptance Criteria

- [ ] Top songs chart shows correct play counts (verified against SAM historylist)
- [ ] Hourly heatmap renders 7-day play pattern correctly
- [ ] Listener graph updates every 30s with real Icecast listener counts
- [ ] Event log captures track_loaded, stream_connected/disconnected in real-time
- [ ] Event log search/filter by level and category works
- [ ] System health shows ring buffer fill levels updating every 5s
- [ ] Alert fires when ring buffer drops below 10%
- [ ] Alert fires (desktop notification) when stream disconnects for > 30s
- [ ] CSV export of play history report downloads to ~/Documents
- [ ] Daily broadcast report PDF renders correctly
- [ ] play_stats_cache refreshes every hour without blocking audio thread
- [ ] Event log auto-clears entries older than configurable days (default: 90)

---

## Dependencies

- Phase 1: Audio engine events feed event_logger
- Phase 3: Scheduler events feed event_logger
- Phase 4: Encoder listener snapshots feed listener_stats
- Phase 5: Scripting events feed event_logger
- Phase 6: Gateway events feed event_logger

---

## Estimated Effort

| Component | Complexity | Est. Days |
|-----------|-----------|-----------|
| `analytics/event_logger.rs` + schema | Low | 1 |
| `analytics/play_stats.rs` aggregation | Medium | 2 |
| `analytics/listener_stats.rs` polling | Low | 1 |
| `analytics/health_monitor.rs` | Medium | 2 |
| `analytics/reports.rs` data generation | Medium | 2 |
| Tauri analytics commands | Low | 1 |
| UI: Analytics dashboard + Top Songs | Medium | 2 |
| UI: Listener graph (recharts) | Medium | 1.5 |
| UI: Event Log panel (virtual list) | Medium | 2 |
| UI: System Health panel | Low | 1 |
| UI: Reports + CSV/PDF export | Medium | 2 |
| **Total** | | **~17.5 days** |
