# DesiZone Broadcaster — Roadmap

A full SAM Broadcaster Pro replacement as a Tauri v2 desktop application with Rust audio engine. See individual phase docs for full architecture details, component lists, and acceptance criteria.

---

## Phase Overview

| Phase | Name | Focus | Est. Days | Status |
|-------|------|-------|-----------|--------|
| **1** | [Audio Engine](phase1-audio-engine.md) | Rust audio engine, crossfade, DSP pipeline, Icecast streaming | ~30 | 🔄 In Progress |
| **2** | [Operator UI](phase2-operator-ui.md) | React dark console UI, deck panels, waveform, crossfade/DSP dialogs | ~25 | ⏳ Pending |
| **3** | [Automation & Scheduling](phase3-automation-scheduling.md) | AutoDJ, rotation rules, show scheduler, GAP killer, request policy | ~22 | ⏳ Pending |
| **4** | [Streaming & Encoders](phase4-streaming-encoders.md) | Multiple encoders, file recording, live listen, listener stats | ~20 | ⏳ Pending |
| **5** | [Scripting & Advanced Audio](phase5-scripting-advanced-audio.md) | Lua scripting engine, mic/voice FX, de-esser, reverb, voice tracking | ~18 | ⏳ Pending |
| **6** | [DBE Gateway Integration](phase6-dbe-gateway-integration.md) | Desktop↔gateway bridge, AutoPilot, remote DJ, live talk | ~13 | ⏳ Pending |
| **7** | [Analytics & Operations](phase7-analytics-operations.md) | Play history, listener graphs, event log, system health, reports | ~17 | ⏳ Pending |

**Total estimated effort: ~145 developer-days**

---

## Phase Dependencies

```
Phase 1 (Audio Engine)
    └─► Phase 2 (UI — needs engine commands to wire up)
    └─► Phase 3 (Automation — needs deck control + queue commands)
    └─► Phase 4 (Encoders — needs master output ring buffer)
    └─► Phase 5 (Scripting — needs all commands available for Lua API)
         └─► Phase 6 (Gateway — needs Phase 5 for live talk)
              └─► Phase 7 (Analytics — needs all phases feeding event logger)
```

Phases 3 and 4 can be worked on in parallel once Phase 1 is complete.  
Phase 5 can begin in parallel with Phase 4.

---

## SAM Broadcaster Parity Matrix

| SAM Feature | Phase | Status |
|-------------|-------|--------|
| Dual deck playback (Deck A / Deck B) | 1 | 🔄 |
| Crossfade: Linear, Exponential, S-Curve, Log, Constant Power | 1 | 🔄 |
| Auto-detect crossfade (dB trigger) | 1 | 🔄 |
| Fixed crossfade point | 1 | 🔄 |
| Per-song fade overrides | 1 | 🔄 |
| 3-band EQ per channel | 1 | 🔄 |
| Gated AGC with pre-emphasis | 1 | 🔄 |
| 5-band multiband compressor | 1 | 🔄 |
| Dual-band processor (LF/HF) | 1 | 🔄 |
| Clipper | 1 | 🔄 |
| Direct Icecast/Shoutcast streaming | 1 | 🔄 |
| Cue points (Start/End/Intro/Outro/Fade/XFade) | 1 | 🔄 |
| VU meters (real dBFS, not simulated) | 1 | 🔄 |
| SAM MySQL schema compatibility | 1 | 🔄 |
| Deck control UI (load, play, pause, seek) | 2 | ⏳ |
| Waveform display | 2 | ⏳ |
| Crossfade settings dialog (full SAM parity) | 2 | ⏳ |
| Fade curve preview graph | 2 | ⏳ |
| Audio Pipeline diagram | 2 | ⏳ |
| EQ/AGC/DSP settings panel per channel | 2 | ⏳ |
| Song Information Editor (8 tabs) | 2 | ⏳ |
| Media library browser | 2 | ⏳ |
| Queue panel (drag-and-drop) | 2 | ⏳ |
| Requests panel | 2 | ⏳ |
| AutoDJ mode | 3 | ⏳ |
| Rotation rules engine | 3 | ⏳ |
| Show scheduler (SAM-compatible) | 3 | ⏳ |
| GAP killer (silence trimming) | 3 | ⏳ |
| Request policy engine | 3 | ⏳ |
| Multiple encoders (Icecast/Shoutcast + file) | 4 | ⏳ |
| Stream-to-file recording + cue sheets | 4 | ⏳ |
| Live listen (local monitoring output) | 4 | ⏳ |
| Listener count graph | 4 | ⏳ |
| Scripting engine (modern Lua, SAM PAL replacement) | 5 | ⏳ |
| Microphone input + Voice FX pipeline | 5 | ⏳ |
| De-esser | 5 | ⏳ |
| Reverb | 5 | ⏳ |
| Voice tracking | 5 | ⏳ |
| DBE gateway integration (AutoPilot) | 6 | ⏳ |
| Remote DJ via web dashboard | 6 | ⏳ |
| Live talk to remote callers | 6 | ⏳ |
| Play history analytics | 7 | ⏳ |
| Event log viewer | 7 | ⏳ |
| System health monitor | 7 | ⏳ |
| Broadcast reports (CSV/PDF) | 7 | ⏳ |

---

## Key Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Desktop framework | Tauri v2 (Rust + webview) | Low-latency local audio, ASIO/WASAPI, cross-platform |
| Audio I/O | CPAL 0.15 | Supports ASIO, WASAPI, CoreAudio, ALSA |
| Audio decode | Symphonia 0.5 | Pure Rust, MPL v2, no GPL taint, MP3/AAC/FLAC/OGG |
| DSP filters | biquad crate | Pure Rust, no GPL, standard biquad IIR |
| Ring buffers | ringbuf 0.4 | Lock-free SPSC, safe for real-time audio thread |
| Database (local) | sqlx + SQLite | Async, no server, cue points + config |
| Database (SAM) | sqlx + MySQL | Direct SAM schema read/write compatibility |
| Icecast streaming | Rust reqwest HTTP PUT | No Liquidsoap dependency on desktop |
| Scripting engine | mlua (Lua 5.4) | Lightweight, sandboxed, modern PAL replacement |
| MP3 encoding | lame-sys (LGPL) | Industry standard, dynamically linked = commercial OK |
| Mixxx code | ❌ Not used | GPL v2 — reimplemented all DSP from math/scratch |
| SAM PAL scripting | ❌ Deprecated | Replaced with Lua + mlua for modern extensibility |

---

## Project Management

- **ADO Project:** Minhaj Prayer Project
- **Assign work items to:** Minhaj Services (services@minhaj.work)
- **Use gemini-cli for:** tests, documentation, large-context code generation
- **Related project:** `../desizone-broadcast-engine` (NestJS + Next.js gateway — Phase 6 integration)

---

## Getting Started

```bash
# Dev mode
npm run tauri dev

# Rust compile check (fast, no window)
cd src-tauri && cargo check

# Run Rust tests
cd src-tauri && cargo test

# Build release
npm run tauri build
```

> **First `cargo build` takes 10–15 minutes** (Tauri + CPAL + Symphonia + sqlx + DSP crates from scratch). Subsequent builds are incremental.
