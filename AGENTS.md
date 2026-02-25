# AGENTS.md

Execution guide for contributors and coding agents working in this repository.

## Repository Snapshot

- App: Tauri v2 desktop broadcaster (`React + TypeScript` frontend, `Rust` backend)
- Frontend root: `src/`
- Backend root: `src-tauri/`
- Bundler/dev server: `Vite`
- Desktop runtime/build: `Tauri CLI`

## Prerequisites

1. Install Node.js `18+` and Rust `1.70+`.
2. Install dependencies from repo root:
   ```bash
   npm install
   ```
3. Optional preflight for Rust crates:
   ```bash
   cd src-tauri && cargo check
   ```

## Command Reference

### Root Commands

```bash
npm install
npm run dev
npm run build
npm run preview
npm run tauri dev
npm run tauri build
```

### Rust Backend Commands

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test
cd src-tauri && cargo fmt --all
cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings
```

### Quick File/Code Navigation

```bash
rg --files
rg "invoke\\(|#[tauri::command]|deck_|crossfade" src src-tauri
```

## Workflows

### 1) Frontend-Only Development Loop

Use this when changing React UI without needing the desktop shell.

1. Start Vite:
   ```bash
   npm run dev
   ```
2. Build-check frontend changes:
   ```bash
   npm run build
   ```
3. Preview production bundle:
   ```bash
   npm run preview
   ```

### 2) Full Desktop App Development Loop

Use this when touching Tauri commands, IPC, or Rust-backed features.

1. Run Tauri dev:
   ```bash
   npm run tauri dev
   ```
2. Verify end-to-end interaction in the desktop window (deck controls, queue, DSP, streaming panels as applicable).
3. Before commit, validate Rust compilation/tests:
   ```bash
   cd src-tauri && cargo check && cargo test
   ```

### 3) Rust Audio Engine / Command Workflow

Use this when editing `src-tauri/src/audio/*` or `src-tauri/src/commands/*`.

1. Fast compile check:
   ```bash
   cd src-tauri && cargo check
   ```
2. Run tests:
   ```bash
   cd src-tauri && cargo test
   ```
3. Enforce style and lints:
   ```bash
   cd src-tauri && cargo fmt --all
   cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings
   ```
4. If command payloads changed, update frontend bridge bindings in `src/lib/bridge.ts`.

### 4) Frontend ↔ Rust IPC Change Workflow

Use this when changing `invoke(...)` calls or Tauri `#[tauri::command]` signatures.

1. Update Rust command handlers in `src-tauri/src/commands/`.
2. Keep command registration in `src-tauri/src/lib.rs` aligned.
3. Update typed wrappers/interfaces in `src/lib/bridge.ts`.
4. Validate with full app run:
   ```bash
   npm run tauri dev
   ```

### 5) Release Build Workflow

1. Build distributable desktop app:
   ```bash
   npm run tauri build
   ```
2. Verify generated artifacts under:
   - `src-tauri/target/release/bundle/dmg/` (macOS)
   - `src-tauri/target/release/bundle/msi/` (Windows)
   - `src-tauri/target/release/bundle/` (Linux targets)

## Guardrails

1. Keep real-time audio callback paths allocation-free and non-blocking.
2. Avoid introducing blocking DB/network work on audio render paths.
3. Keep Rust command contracts and frontend bridge types synchronized.
4. Prefer targeted fixes over broad refactors in `audio/engine.rs` and command modules.
5. Do not commit secrets, server passwords, or local machine paths.
