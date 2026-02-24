# ADR 001: No Liquidsoap — Direct Icecast Streaming from Rust

## Status
Accepted

## Context
The previous DBE project used Liquidsoap as the audio engine for server-side streaming. DesiZone Broadcaster is a local desktop application that needs direct soundcard access and low-latency monitoring.

## Decision
The Rust backend (via CPAL) handles all audio I/O, DSP processing, and Icecast streaming directly. Liquidsoap is not a dependency.

## Consequences
- No external audio engine process to manage
- Icecast streaming implemented via HTTP PUT in Rust (reqwest + MP3 encoding via lame/shine)
- Full control over audio pipeline, crossfading, and DSP in one process
