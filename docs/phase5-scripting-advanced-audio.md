# Phase 5 — Modern Scripting Engine & Advanced Audio

## Goal
Replace SAM's legacy PAL (Pascal-based) script engine with a modern embedded scripting system. Add live mic input processing, voice tracking, and advanced DSP not covered in Phase 1 (VST-like plugin support, voice FX chain).

---

## 5.1 Modern Scripting Engine

### Language Choice: Lua via `mlua`

Lua is the industry standard for embedded scripting in broadcast/media software:
- `mlua` crate: safe, async-capable Lua 5.4 bindings for Rust (MIT license)
- Lightweight: ~200KB Lua runtime
- Easy to learn: simpler syntax than JavaScript for non-programmers
- Industry precedent: used in Liquidsoap, OBS Studio, game engines

**JavaScript alternative:** `deno_core` crate (V8-based) — more powerful but significantly heavier. Consider as opt-in "advanced" scripting in a later sub-phase.

### Script Triggers (Events that fire scripts)

```lua
-- Track events
on_track_start(function(track)
    -- track.id, track.title, track.artist, track.album, track.bpm, track.duration_ms
    -- track.category, track.cue_points
end)

on_track_end(function(track)
    -- called when track finishes naturally
end)

on_crossfade_start(function(outgoing, incoming)
end)

-- Queue events
on_queue_empty(function()
    -- e.g. switch to emergency playlist
    queue.add_playlist(42)
end)

on_request_received(function(request)
    -- request.song_id, request.requester, request.song_title
    -- return true to auto-accept, false to auto-reject, nil for manual review
    if request.requester == "banned_user" then
        return false
    end
    return nil
end)

-- Show/schedule events
on_show_start(function(show)
    -- show.name, show.id
end)

on_hour(function(hour)
    -- fires at the start of each hour (0-23)
    if hour == 6 then
        encoder.set_stream_title("Morning Show")
    end
end)

-- Encoder events
on_encoder_connect(function(encoder_id) end)
on_encoder_disconnect(function(encoder_id, reason) end)
```

### Script API Surface (Lua globals)

```lua
-- Deck control
deck.play("deck_a")
deck.stop("deck_a")
deck.load("deck_a", song_id)
deck.crossfade("deck_a", "deck_b", duration_ms)
deck.get_position("deck_a")  -- returns ms

-- Queue
queue.get()             -- returns list of songs
queue.add(song_id)
queue.add_at(song_id, position)
queue.remove(position)
queue.clear()
queue.add_playlist(playlist_id)

-- Media
media.search(query)        -- returns list of songs
media.get(song_id)         -- returns song info
media.get_random(category) -- random song from category

-- Encoders
encoder.start(id)
encoder.stop(id)
encoder.set_stream_title(id, title)
encoder.get_listeners(id)

-- Scheduling
schedule.add_once(datetime_iso, action_fn)
schedule.add_cron(cron_expr, action_fn)  -- "0 * * * *" = every hour
schedule.remove(id)

-- Station
station.set_mode("autodj" | "assisted" | "manual")
station.emergency_stop()

-- Logging
log.info("message")
log.warn("message")
log.error("message")

-- HTTP (for webhooks, Discord notifications, etc.)
http.get(url)             -- returns {status, body}
http.post(url, body_json) -- returns {status, body}

-- Storage (per-script key/value)
store.set("key", value)
store.get("key")
store.delete("key")
```

### Script Storage (`db/local.rs`)

```sql
CREATE TABLE scripts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    content TEXT NOT NULL,        -- Lua source code
    enabled INTEGER DEFAULT 1,
    trigger_type TEXT NOT NULL,   -- 'on_track_start' | 'on_hour' | 'manual' | etc.
    last_run_at DATETIME,
    last_error TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE script_store (
    script_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,          -- JSON-encoded value
    PRIMARY KEY (script_id, key)
);
```

### Script Runtime (`src-tauri/src/scripting/`)

```
scripting/
├── mod.rs
├── engine.rs        — Lua VM manager, one VM per script (isolation)
├── api.rs           — registers all Lua globals (deck, queue, media, etc.)
├── trigger.rs       — maps Rust events to Lua callbacks
└── sandbox.rs       — restricts filesystem/network access per script settings
```

Scripts run in their own Lua VMs (no shared global state between scripts).
Long-running scripts run in separate Tokio tasks.
Script errors are caught and logged — never crash the audio engine.

### Script Editor UI (`src/components/scripting/`)

- `ScriptList.tsx` — list of scripts with enabled toggle, trigger type badge, last run time, last error indicator
- `ScriptEditor.tsx` — full editor modal:
  - Code editor with Lua syntax highlighting (`@codemirror/lang-lua` or Monaco editor)
  - Trigger selector dropdown
  - Test run button (fires script manually with mock data)
  - Output log panel (shows log.info/warn/error output)
  - Error display with line number

---

## 5.2 Live Microphone Input & Voice FX

### Voice FX Pipeline (`audio/`)

The Voice FX channel in the mixer pipeline is fed from a **microphone input** (CPAL input stream):

```
Microphone (CPAL input stream)
  └─ Decoder: none (raw PCM from input)
       └─ Voice FX Pipeline:
            ├─ Noise Gate (threshold, attack, release)
            ├─ EQ (cut low rumble, boost presence)
            ├─ AGC / Compressor (voice-specific settings)
            ├─ De-esser (optional — reduce harsh sibilance)
            └─ Reverb (optional — room effect)
       └─ Voice FX output → Mixer (Voice FX channel)
```

### Mic Input Config (`db/local.rs`)

```sql
CREATE TABLE mic_config (
    id INTEGER PRIMARY KEY DEFAULT 1,
    device_name TEXT,           -- CPAL input device name, NULL = default
    sample_rate INTEGER DEFAULT 44100,
    channels INTEGER DEFAULT 1,  -- mono mic
    
    -- Noise gate
    gate_enabled INTEGER DEFAULT 1,
    gate_threshold_db REAL DEFAULT -40.0,
    gate_attack_ms REAL DEFAULT 5.0,
    gate_release_ms REAL DEFAULT 200.0,
    
    -- Voice compressor
    comp_enabled INTEGER DEFAULT 1,
    comp_ratio REAL DEFAULT 4.0,
    comp_threshold_db REAL DEFAULT -18.0,
    comp_attack_ms REAL DEFAULT 10.0,
    comp_release_ms REAL DEFAULT 100.0,
    
    -- Push-to-talk
    ptt_enabled INTEGER DEFAULT 0,
    ptt_hotkey TEXT              -- e.g. "Space" or "F1"
);
```

### Push-to-Talk (PTT)
- Global hotkey registered with Tauri's `tauri-plugin-global-shortcut`
- While hotkey held: Voice FX channel is active in mixer
- On release: brief fade-out (100ms) then silence
- Visual indicator in UI: large "ON AIR" mic indicator when active

### Voice Tracking
Pre-record voice segments for automation:
- Record a voice segment (saved to local file)
- Tag it as a "Voice Track" in `songlist` (SAM-compatible — use `category` field)
- AutoDJ rotation can include voice tracks between songs
- Voice track plays from Voice FX channel (so it goes through Voice FX DSP chain)

### UI (`src/components/voice/`)
- `VoiceFXStrip.tsx` — always visible in main UI, shows mic level VU, PTT button, mute
- `MicSettings.tsx` — dialog for input device selection + noise gate + compressor settings
- `VoiceTrackRecorder.tsx` — simple recorder: record, preview, name, save-to-library

---

## 5.3 Advanced DSP — De-esser & Reverb

Added specifically for the Voice FX chain (not on music channels by default):

### De-esser (`audio/dsp/deesser.rs`)
Detects and reduces harsh sibilance (6–10kHz range) in voice:
- Sidechain: band-pass filter detects level in sibilance range
- When level exceeds threshold: apply attenuation to that band only
- Parameters: Frequency, Threshold, Ratio, Range

### Reverb (`audio/dsp/reverb.rs`)
Simple Schroeder reverb for voice FX:
- Room size (small/medium/large/hall presets)
- Wet/dry mix
- Damping
- Implementation: allpass + comb filters (standard Schroeder design — public domain math)
- Crate option: `freeverb` or custom implementation

---

## 5.4 Tauri Commands (Phase 5)

```typescript
// Scripts
invoke('get_scripts') → Script[]
invoke('save_script', { script }) → id
invoke('delete_script', { id }) → void
invoke('run_script', { id }) → { success: bool, output: string[], error?: string }
invoke('get_script_log', { id, limit }) → ScriptLogEntry[]

// Microphone
invoke('get_audio_input_devices') → AudioDevice[]
invoke('get_mic_config') → MicConfig
invoke('set_mic_config', { config }) → void
invoke('start_mic') → void
invoke('stop_mic') → void
invoke('set_ptt', { active: bool }) → void   // for UI PTT button (fallback if hotkey unavailable)

// Voice Tracking
invoke('start_voice_recording') → void
invoke('stop_voice_recording') → { filePath: string, durationMs: number }
invoke('save_voice_track', { filePath, title }) → songId

// Events
listen('ptt_state_changed', handler)        // { active: bool }
listen('script_log', handler)               // { scriptId, level, message, timestamp }
listen('script_error', handler)             // { scriptId, error, line }
listen('mic_level', handler)                // { leftDb, rightDb } at 80ms
```

## Acceptance Criteria

1. Script `on_track_start` fires for every track change → `log.info(track.title)` appears in script log panel
2. Script `on_queue_empty` auto-adds emergency playlist → queue populates automatically when empty
3. Script `http.post` sends Discord webhook when a new track starts (test with webhook URL)
4. Mic input active → voice heard in Air output → VU meter shows level
5. PTT hotkey held → Voice FX activates, visible "MIC LIVE" indicator → release → fades out
6. Voice FX noise gate: mic below threshold → silent in output; above threshold → heard
7. Voice track recorded → appears in library as "Voice Track" category → AutoDJ picks it up in rotation
8. De-esser on Voice FX: harsh 's' sounds reduced without dulling overall voice quality
9. Script error (syntax error) → shown in editor with line number → audio engine unaffected
