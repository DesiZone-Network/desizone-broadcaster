# Phase 6 — DBE Gateway Integration

## Overview

Phase 6 bridges the DesiZone Broadcaster desktop app with the DesiZone Broadcast Engine (DBE) NestJS gateway, creating a hybrid architecture where the desktop handles high-fidelity local audio and the gateway handles web-based remote access, AutoPilot, and collaborative DJ sessions.

**Core principle:** The desktop app is always the audio authority. The gateway syncs state from the desktop, never the other way around when both are running.

---

## Hybrid Architecture

```
┌─────────────────────────────────────────────────────┐
│           DesiZone Broadcaster (Desktop)             │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │  Audio Engine │  │  Scheduler   │  │  Scripting│  │
│  │  CPAL/Decks  │  │  AutoDJ/Queue│  │  Lua mlua │  │
│  └──────┬───────┘  └──────┬───────┘  └─────┬─────┘  │
│         └─────────────────┴────────────────┘         │
│                      │ Desktop Bridge                 │
│               ┌──────▼──────┐                        │
│               │  Gateway     │  WebSocket sync        │
│               │  WS Client   │◄──────────────────►   │
│               └─────────────┘                        │
└─────────────────────────────────────────────────────┘
                         │
          ┌──────────────▼──────────────┐
          │    DBE Gateway (NestJS)      │
          │  REST + WebSocket + SAM MySQL│
          └──────────┬───────────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
  ┌─────▼─────┐            ┌──────▼──────┐
  │  Web DJ   │            │  AutoPilot  │
  │  (Remote) │            │  (Headless) │
  └───────────┘            └─────────────┘
```

---

## Operation Modes

### Mode 1: Desktop Primary (Full DJ)
- Desktop app is running with operator present
- Full crossfade, DSP, multiple decks, cue monitoring
- Gateway receives state pushes for web dashboard display
- Remote listeners see live "now playing" via gateway WebSocket
- Request queue managed by desktop or gateway (synced)

### Mode 2: AutoPilot (Headless via Gateway)
- Desktop app is running but operator is away
- AutoDJ rotation rules run on desktop
- Gateway web dashboard shows controls for AutoPilot management
- Operator can intervene via web dashboard (queue injection, override)
- Crossfade, DSP, gap killer all active on desktop

### Mode 3: Remote DJ (Web)
- Desktop app is running
- Remote DJ connects via DBE web dashboard
- Remote DJ can: load tracks from SAM library, queue management, adjust basic levels
- Actual playback remains on desktop (audio source of truth)
- Real-time waveform/VU feed streamed as WebSocket events from desktop → gateway → web

### Mode 4: Live Talk (Studio-in-a-Box)
- Desktop handles mic input, Voice FX pipeline (phase 5)
- Live talk button from desktop triggers gateway to notify listeners via WebSocket event
- Remote callers via WebRTC (gateway handles signalling)
- Mix-minus output: desktop sends audio without mic return to remote callers

---

## New Module: `src-tauri/src/gateway/`

```
src-tauri/src/gateway/
├── mod.rs
├── client.rs        # WebSocket client to DBE gateway
├── sync.rs          # State sync: queue, now-playing, VU push
├── remote_dj.rs     # Accept remote DJ commands from gateway
└── auth.rs          # JWT/token auth for gateway connection
```

### `gateway/client.rs`

```rust
pub struct GatewayClient {
    url: String,
    token: String,
    ws_sender: Option<mpsc::Sender<GatewayMessage>>,
    connected: Arc<AtomicBool>,
}

pub enum GatewayMessage {
    NowPlaying { song_id: i64, title: String, artist: String, duration_ms: u32 },
    QueueUpdated { queue: Vec<QueueItem> },
    DeckState { deck: String, state: String, position_ms: u32 },
    VuMeter { channel: String, left_db: f32, right_db: f32 },
    ListenerCount { count: u32 },
    RemoteDjCommand(RemoteDjCommand),
    CrossfadeProgress { progress: f32, outgoing: String, incoming: String },
}

pub enum RemoteDjCommand {
    LoadTrack { deck: String, song_id: i64 },
    PlayDeck { deck: String },
    PauseDeck { deck: String },
    SetVolume { channel: String, volume: f32 },
    AddToQueue { song_id: i64, position: Option<usize> },
    RemoveFromQueue { queue_id: i64 },
    TriggerCrossfade,
    SetAutoPilot { enabled: bool },
}
```

---

## New Tauri Commands (`commands/gateway_commands.rs`)

```typescript
// Gateway connection
invoke('connect_gateway', { url: string, token: string }) → { connected: boolean }
invoke('disconnect_gateway')
invoke('get_gateway_status') → GatewayStatus

// AutoPilot
invoke('set_autopilot', { enabled: boolean, mode: 'rotation' | 'queue' | 'scheduled' })
invoke('get_autopilot_status') → AutoPilotStatus

// Remote DJ
invoke('get_remote_sessions') → RemoteSession[]
invoke('kick_remote_dj', { sessionId: string })
invoke('set_remote_dj_permissions', { sessionId: string, permissions: DjPermissions })

// Live Talk
invoke('start_live_talk', { channel: 'voice_fx' | 'aux_1' })
invoke('stop_live_talk')
invoke('set_mix_minus', { enabled: boolean })
```

### New Events (Rust → Frontend)

```typescript
listen('gateway_connected',     (e) => GatewayStatus)
listen('gateway_disconnected',  (e) => { reason: string })
listen('remote_dj_joined',      (e) => RemoteSession)
listen('remote_dj_left',        (e) => { sessionId: string })
listen('remote_command_received',(e) => RemoteDjCommand)
listen('listener_count_updated', (e) => { count: number })
```

---

## DBE Gateway Side Changes

The gateway needs a new WebSocket namespace or event channel for desktop sync (this is in the `desizone-broadcast-engine` project, not this desktop project):

```typescript
// apps/gateway/src/modules/desktop-bridge/
// New module: DesktopBridgeModule
// - Accepts WebSocket connection from desktop app (authenticated)
// - Broadcasts state to web dashboard clients
// - Forwards remote DJ commands to desktop

// Events gateway receives from desktop:
'desktop:now_playing'       // { songId, title, artist, duration }
'desktop:deck_state'        // { deck, state, positionMs }
'desktop:vu_meter'          // { channel, leftDb, rightDb }
'desktop:queue_updated'     // { queue[] }
'desktop:crossfade_progress'// { progress, outgoing, incoming }
'desktop:stream_status'     // { connected, mount, listeners }

// Events gateway sends to desktop:
'remote:command'            // RemoteDjCommand
'remote:dj_joined'          // { sessionId, userId, displayName }
'remote:dj_left'            // { sessionId }
'remote:request_received'   // { songId, requestedBy }
```

---

## New SQLite Tables (`db/local.rs` additions)

```sql
-- Gateway connection settings
CREATE TABLE gateway_config (
    id INTEGER PRIMARY KEY DEFAULT 1,
    url TEXT,                          -- e.g. ws://localhost:3000/api/v1/live
    token TEXT,                        -- JWT or API key
    auto_connect INTEGER DEFAULT 0,   -- connect on startup
    sync_queue INTEGER DEFAULT 1,     -- push queue changes to gateway
    sync_vu INTEGER DEFAULT 1,        -- push VU meter to gateway
    vu_throttle_ms INTEGER DEFAULT 200 -- throttle VU updates (don't flood gateway)
);

-- Remote DJ permissions (per session/user)
CREATE TABLE remote_dj_permissions (
    user_id TEXT PRIMARY KEY,
    can_load_track INTEGER DEFAULT 0,
    can_play_pause INTEGER DEFAULT 1,
    can_seek INTEGER DEFAULT 0,
    can_set_volume INTEGER DEFAULT 1,
    can_queue_add INTEGER DEFAULT 1,
    can_queue_remove INTEGER DEFAULT 0,
    can_trigger_crossfade INTEGER DEFAULT 0,
    can_set_autopilot INTEGER DEFAULT 0
);

-- Remote DJ session log
CREATE TABLE remote_sessions_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    display_name TEXT,
    connected_at INTEGER NOT NULL,   -- Unix ms
    disconnected_at INTEGER,
    commands_sent INTEGER DEFAULT 0
);
```

---

## UI Components (Phase 6 Additions)

### Gateway Panel (Settings → Gateway tab)
- URL input + Token field
- Connect/Disconnect button
- Connection status indicator (green/amber/red)
- Active remote sessions list with kick button
- Permission matrix per remote user

### Remote DJ Overlay
- "Remote DJ active" banner when a remote session is connected
- Commands from remote DJ shown as ghost actions in the deck UI before execution
- Operator approve/reject for sensitive commands (crossfade trigger, volume changes)

### AutoPilot Mode Switcher
- Toggle in main toolbar: Desktop DJ ↔ AutoPilot
- AutoPilot shows which rotation rule is active
- Gateway web dashboard can toggle this (with operator permission)

---

## Sync Strategy

| Data | Direction | Frequency |
|------|-----------|-----------|
| Now Playing (song metadata) | Desktop → Gateway | On track change |
| Deck state (play/pause/position) | Desktop → Gateway | On state change |
| Queue (full list) | Desktop → Gateway | On queue change |
| VU meter readings | Desktop → Gateway | 200ms (configurable) |
| Crossfade progress | Desktop → Gateway | Every 100ms during fade |
| Stream status | Desktop → Gateway | On connect/disconnect |
| Remote DJ commands | Gateway → Desktop | Immediately |
| Listener count | Gateway → Desktop | Every 30s |

**Conflict resolution:** If desktop and gateway both have queue changes (e.g., request added via web + manual add via desktop in same second), desktop queue wins. Gateway sends deltas and desktop applies them after its own changes.

---

## Acceptance Criteria

- [ ] Desktop connects to gateway via WebSocket with JWT auth
- [ ] Web dashboard shows real-time "now playing" pushed from desktop
- [ ] Remote DJ can load a track via web dashboard → track loads in desktop
- [ ] AutoPilot mode toggle works from both desktop and web dashboard
- [ ] VU meter events visible in web dashboard (piped from desktop → gateway → web)
- [ ] Remote DJ permission matrix enforced (e.g. cannot trigger crossfade without permission)
- [ ] Live talk: mic + Voice FX audio flows to encoder when live talk active
- [ ] Gateway disconnect does not affect desktop audio playback
- [ ] Listener count from gateway Icecast poll shown in desktop status bar
- [ ] Session log persisted to SQLite on every remote DJ connect/disconnect

---

## Dependencies

- Phase 1: Audio engine (deck control commands available)
- Phase 3: AutoDJ / rotation rules (AutoPilot mode references rotation engine)
- Phase 4: Multi-encoder (stream status sync)
- Phase 5: Live Talk / Voice FX (mic input commands available)
- DBE gateway must have `DesktopBridgeModule` implemented (separate project: `desizone-broadcast-engine`)

---

## Estimated Effort

| Component | Complexity | Est. Days |
|-----------|-----------|-----------|
| `gateway/client.rs` WebSocket client | Medium | 2 |
| `gateway/sync.rs` state push | Low | 1 |
| `gateway/remote_dj.rs` command handler | Medium | 2 |
| SQLite gateway config + permissions tables | Low | 0.5 |
| Tauri gateway commands + events | Low | 1 |
| UI: Gateway settings panel | Low | 1 |
| UI: Remote DJ overlay + permission matrix | Medium | 2 |
| UI: AutoPilot mode switcher | Low | 0.5 |
| DBE gateway: DesktopBridgeModule (separate project) | High | 3 |
| **Total** | | **~13 days** |
