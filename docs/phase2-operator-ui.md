# Phase 2 — Operator UI (Modern Dark Console)

## Goal
Build the main operator window and all settings dialogs. Phase 1 gave us the Rust audio engine. Phase 2 wires it to a full visual interface that matches SAM Broadcaster's feature set but with a modern dark broadcast console aesthetic.

## UI Philosophy
- Dark theme (#0d0d0d background, #1a1a1a panels, #2a2a2a borders)
- Accent: amber/orange (#f59e0b) for on-air, cyan (#06b6d4) for cue, green (#22c55e) for active
- Inspired by: Mixxx 2.4, Traktor Pro, modern DAWs (not Windows 95 grey)
- All meters are canvas-based (real pixel rendering, not CSS faking)
- Keyboard shortcuts for every critical action

## Main Window Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ TOP BAR: Station name │ ON AIR ● │ VU Master L/R │ Clock │ Stream status │
├────────────────┬───────────────────────────────────┬────────────────────┤
│  DECK A        │         CROSSFADE BAR              │  DECK B            │
│  [waveform]    │  [====●=========]  AUTO  8.0s      │  [waveform]        │
│  ▶ 02:34/04:12 │  Exp ──╮  ╭── S-Curve             │  ■ 00:00/03:58     │
│  VU: ██████░░  │         ╰──╯                       │  VU: ░░░░░░░░      │
│  [▶][■][⏮][⏭]  │                                    │  [▶][■][⏮][⏭]     │
│  Vol: ──●───   │  [DECK A → DECK B]  [FORCE XFADE]  │  Vol: ────●──      │
│  Air  Cue      │                                    │  Air  Cue          │
├────────────────┴───────────────────────────────────┴────────────────────┤
│  AUX 1  [▶][■] Vol:──●──  │  SOUND FX  [▶][■] Vol:──●──  │  VOICE FX ON │
├──────────────────────────────────────────────────────────────────────────┤
│  QUEUE (drag to reorder)    │  MEDIA LIBRARY             │  REQUESTS      │
│  ▶ 1. Now Playing...        │  [Search: ___________]     │  ● 3 pending   │
│    2. Next song...          │  Artist / Title / BPM      │  1. Song req   │
│    3. ...                   │  [Add to queue] [Edit]     │  2. Song req   │
└──────────────────────────────────────────────────────────────────────────┘
```

## Components to Build

### 2.1 Main Layout Shell (`src/components/layout/`)
- `MainWindow.tsx` — root layout, panel sizing, keyboard event capture
- `TopBar.tsx` — station name, on-air badge, master VU, clock, stream status badges
- `DeckArea.tsx` — houses two deck panels + crossfade bar
- `SourceRow.tsx` — AUX 1, Sound FX, Voice FX strip
- `BottomPanel.tsx` — tabbed: Queue | Library | Requests | History | Logs

### 2.2 Deck Panel (`src/components/deck/`)
- `DeckPanel.tsx` — full deck with waveform, controls, VU
- `WaveformCanvas.tsx` — canvas element, draws PCM waveform from preloaded data + playhead
- `VUMeter.tsx` — canvas, receives `vu_meter` events from Rust, draws L/R bars at 60fps
- `DeckTransport.tsx` — Play/Pause/Stop/Cue buttons + loop toggle + pitch control
- `DeckInfo.tsx` — title, artist, album art, BPM, time elapsed/remaining
- `AirCueToggle.tsx` — toggles between Air monitoring and Cue preview

### 2.3 Crossfade Bar (`src/components/crossfade/`)
- `CrossfadeBar.tsx` — visual crossfade slider + auto/manual mode + curve mini-preview
- `CrossfadeSettingsDialog.tsx` — **SAM parity dialog** (see detail below)
- `FadeCurveGraph.tsx` — canvas, draws the two-curve preview (green fade-out, blue fade-in, red dashed crossfade point) using `get_fade_curve_preview` command data

### CrossfadeSettingsDialog — SAM Exact Parity
Maps 1:1 to the SAM Broadcaster Cross-Fading dialog screenshots:

```
Left column (Fade Out):
  ☑ Enable fade out
  Curve: [Exponential ▼]   [preview swatch]
  Time:  [════════●══] 10000 ms
  Level: [══════●════]  80 %

Right column (Fade In):
  ☑ Enable fade in
  Curve: [S-Curve ▼]     [preview swatch]
  Time:  [════════●══] 10000 ms
  Level: [══════●════]  80 %

Cross-fade:
  [Auto detect (dB level) ▼]
  Fixed cross-fade point
  Time: [══════════●] 8000 ms

Cross-fade point detection:
  Trigger at: [-3 ↕] dB  [indicator]
  Min. fade time:  3000 ms
  Max. fade time: 10000 ms

☐ Do not crossfade or fade [65 ↕] seconds or less in duration

[Large canvas preview graph — same as SAM screenshot]

[Restore Defaults]   [OK]  [Cancel]
```

### 2.4 Audio Pipeline Diagram (`src/components/pipeline/`)
- `AudioPipelineDiagram.tsx` — SVG/canvas node graph matching SAM's Audio Mixer Pipeline screenshot
  - 5 source nodes on left (Deck A/B, Sound FX, Aux 1, Voice FX)
  - Each source connects to: EQ → AGC → DSP node chain
  - All chains converge to Mixer node
  - Mixer connects to: EQ → AGC → DSP → Air Out + Encoders
  - Clicking any node opens its settings panel
  - Green dots = signal flowing, grey = inactive

### 2.5 Per-Channel Audio Settings Dialog (`src/components/dsp/`)
Maps to SAM's Audio Settings dialog (tabs: Equalizer | AGC | DSP):

**Equalizer tab:**
- 3-band visual EQ (low shelf, mid peak, high shelf)
- Frequency response curve canvas
- Per-band: gain slider, frequency knob, Q knob (for mid)
- Presets (Flat, Bass Boost, Presence, etc.)

**AGC tab** (maps to SAM screenshots exactly):
- ☑ Gated AGC: Gate slider (-31 dB), Max gain slider (5 dB), AGC meter (+25 dB range)
- Pre-emphasis: [50 uS] [75 uS] toggle buttons
- ☐ Stereo expander: Level / Depth / Thr sliders + Img/Act meters
- ☑ Bass EQ: Gain slider, Frequency slider, [Shelve] [Peak] toggle
- ☑ 5 Bands processor: per-band Ratio, Thr, Attack, Release, Hold, Band Gain + Links + meters
- Mode: [Compressor] [Expander] [Limiter] toggle per band
- ☑ Dual band processor: LF/HF split, same controls
- ☐ Clipper: Level + Clipper sliders
- Levels meters: In/Out dB VU

### 2.6 Song Information Editor (`src/components/songs/`)
Tabbed modal — maps to SAM's Song information editor:
- **Info tab**: Title, Artist, Album, Year, Genre, Track #, Comment fields
- **Picture tab**: Album art upload/display
- **Lyrics tab**: Lyrics text area
- **Comments tab**: Station notes
- **Details tab**: File path, format, bitrate, duration, file size, fingerprint
- **Reporting fields tab**: Copyright, ISRC, composer, publisher (for royalty reporting)
- **Settings tab** (maps to SAM screenshots):
  - Mini player with Air/Cue buttons and VU meter
  - Cue points: Start, End, Intro, Outro, Fade, XFade (each: [Set] [value] [jump])
  - Custom cue points 0–9
  - BPM: [Tap beats] [Auto detect] + metronome toggle + manual BPM field
  - Gap killer: [Default ▼] dropdown
  - Gain: [-6 dB] ──●── [+6 dB] slider
- **Fading tab** (per-song override of global crossfade config — maps to SAM Fading screenshots):
  - All fields have ☐ override checkbox (unchecked = inherit global)
  - Same layout as CrossfadeSettingsDialog

### 2.7 Queue Panel (`src/components/queue/`)
- `QueuePanel.tsx` — drag-to-reorder list, now-playing highlight, estimated end time
- Context menu: Remove, Edit, Move to top, Properties
- Columns: #, Title, Artist, Duration, BPM, Intro time, Fade out time, Category

### 2.8 Media Library Panel (`src/components/library/`)
- Search: real-time search against SAM `songlist` via `search_songs` command
- Columns: Title, Artist, Album, Duration, BPM, Category, Last Played
- Double-click: open Song Information Editor
- Right-click: Add to Queue, Add to Playlist, Properties
- Drag to queue

### 2.9 Requests Panel (`src/components/requests/`)
- Pending requests list with requester info
- [Accept] → auto-adds to queue respecting request policy
- [Reject] with optional reason
- Request badge count on tab

## New Tauri Commands Needed (Phase 2)

```typescript
// Waveform data (for canvas drawing)
invoke('get_waveform_data', { filePath, resolution: 1000 }) → Float32Array (L peak data)

// Media library
invoke('search_songs', { query, limit, offset }) → SongResult[]
invoke('get_song', { songId }) → SongDetail
invoke('update_song', { songId, fields }) → void
invoke('get_album_art', { songId }) → base64 | null

// Requests  
invoke('get_requests', { status: 'pending' | 'accepted' | 'rejected' }) → Request[]
invoke('accept_request', { requestId }) → void
invoke('reject_request', { requestId, reason? }) → void
```

## New Events Needed

```typescript
listen('playhead_update', handler)    // { deck, positionMs } at 100ms interval for waveform scrubbing
listen('queue_updated', handler)      // full queue rebroadcast on any change
listen('request_received', handler)   // new listener request from web (future DBE integration)
```

## Frontend Dependencies to Add

```json
"@radix-ui/react-dialog": "^1",      // accessible modal dialogs
"@radix-ui/react-slider": "^1",      // accessible sliders for EQ/AGC controls
"@radix-ui/react-tabs": "^1",        // tabbed song editor
"@radix-ui/react-select": "^2",      // curve type dropdowns
"@radix-ui/react-dropdown-menu": "^2",
"framer-motion": "^12",              // panel animations
"@dnd-kit/core": "^6",               // queue drag-to-reorder
"@dnd-kit/sortable": "^8",
"lucide-react": "^0.574",            // icons
"clsx": "^2",
"tailwind-merge": "^3"
```

## Acceptance Criteria

1. Main window renders all panels without errors
2. Deck A plays a test MP3 — waveform visible, VU meter animating, playhead moving
3. CrossfadeSettingsDialog opens from toolbar, all fields editable, saved config reflected in `get_crossfade_config`
4. FadeCurveGraph canvas draws correct curves for each type (verify Exponential is quadratic, S-Curve is smooth sigmoid)
5. Song Information Editor opens from library double-click; Fading tab shows per-song override controls
6. Cue point set in Settings tab persists after dialog close (SQLite roundtrip)
7. Queue drag-to-reorder works; order change triggers `queue_updated` event
8. Audio Pipeline Diagram shows all 5 source nodes; clicking Deck A EQ node opens EQ dialog pre-loaded with Deck A settings
