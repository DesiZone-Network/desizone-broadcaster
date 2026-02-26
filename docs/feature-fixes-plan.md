# Feature Fixes + Enhancements Plan

This plan translates the requested fixes/features into an implementation sequence with file-level touch points and acceptance checks.

## Scope Summary

1. Drag-drop songs from Queue/Library into DECK A, DECK B, AUX1, AUX2.
2. Queue reorder by drag-drop.
3. Fix CUE set + CUE go.
4. Fix SYNC behavior (sync active deck to the opposite deck BPM).
5. Populate LOGS tab with activity + errors.
6. Add EQ slider reset controls.
7. Expand Requests tab (all requests + active pending section + add-to-queue action).
8. Add album art base URL setting and render album art in edit + deck headers.
9. Show current listeners in top bar and persist listener snapshot into history entries.

---

## Phase 1 — Discovery + Contract Audit

### Frontend files to audit
- `src/components/deck/DeckPanel.tsx`
- `src/components/library/LibraryPanel.tsx`
- `src/components/queue/QueuePanel.tsx`
- `src/components/requests/RequestsPanel.tsx`
- `src/components/layout/TopBar.tsx`
- `src/components/analytics/EventLogPanel.tsx`
- `src/components/dsp/ChannelDspDialog.tsx`
- `src/pages/SettingsPage.tsx`
- `src/lib/songDrag.ts`
- `src/lib/bridge.ts`

### Backend files to audit
- `src-tauri/src/commands/queue_commands.rs`
- `src-tauri/src/commands/cue_commands.rs`
- `src-tauri/src/commands/audio_commands.rs`
- `src-tauri/src/commands/dsp_commands.rs`
- `src-tauri/src/commands/analytics_commands.rs`
- `src-tauri/src/analytics/event_logger.rs`
- `src-tauri/src/analytics/listener_stats.rs`
- `src-tauri/src/audio/engine.rs`
- `src-tauri/src/audio/deck.rs`
- `src-tauri/src/db/sam.rs`
- `src-tauri/src/lib.rs`

### Output of this phase
- Mapping document/table of: UI event → bridge API → tauri command → runtime/db effect.
- Gap list for each requested feature.

---

## Phase 2 — Drag & Drop Into Deck A/B + AUX1/AUX2

### Implementation
1. Standardize drag payload type in `src/lib/songDrag.ts` for both queue/library origins.
2. Make all target strips droppable in `DeckPanel` (A/B + AUX1/AUX2).
3. Route dropped songs through the same load path used by existing deck/aux load actions.
4. Add visual drop affordance + invalid-drop guard.

### Acceptance checks
- Drag from Library to each target works.
- Drag from Queue to each target works.
- Invalid payload or unsupported target does not crash and logs warning.

---

## Phase 3 — Queue Reorder via Drag & Drop

### Implementation
1. Add item drag handles/reorder interactions in `QueuePanel`.
2. Persist order changes via queue command(s) in bridge/backend.
3. Ensure deterministic order key and stable updates.

### Acceptance checks
- Reordered queue remains in same order after refresh/reload.
- Reordering emits activity log entries.

---

## Phase 4 — CUE and SYNC Reliability

### CUE fixes
1. Validate command wiring for cue set/go from UI controls.
2. Ensure cue point state is written/read from correct deck.
3. On CUE GO: seek to saved cue; preserve expected play/pause behavior.
4. Add explicit user feedback when no cue exists.

### SYNC fixes
1. Identify current/active deck semantics.
2. Apply BPM sync target as the opposite deck BPM.
3. Add guards for missing/invalid BPM.

### Acceptance checks
- CUE set stores current position.
- CUE go reliably returns to saved position.
- Pressing SYNC on deck A follows deck B BPM and vice versa.

---

## Phase 5 — Logs Tab Activity + Errors

### Implementation
1. Define structured log model (`timestamp`, `level`, `source`, `action`, `details`).
2. Feed logs from:
   - Queue operations
   - Deck load/play/pause/cue/sync actions
   - Request promotion to queue
   - Settings updates
   - Invoke/backend errors
3. Render list in logs panel with level styling and optional filtering.

### Acceptance checks
- Logs tab is non-empty during normal usage.
- Runtime errors are visible in Logs tab with actionable text.

---

## Phase 6 — Deck EQ Reset Controls

### Implementation
1. Add reset button for each EQ slider in `ChannelDspDialog` (or deck EQ component in use).
2. Reset target value to neutral baseline (e.g., `0 dB`).
3. Ensure control updates both UI state and DSP backend consistently.

### Acceptance checks
- Resetting one slider affects only that slider.
- Audio continues without artifacts from reset action.

---

## Phase 7 — Requests Tab Overhaul

### Implementation
1. Query/show all rows from `requestlist`.
2. Add top section for active pending requests.
3. Add action to promote pending request into queue.
4. Update status metadata after promotion.

### Acceptance checks
- All requests visible.
- Pending section correctly reflects active/pending rows.
- Add-to-queue works and is logged.

---

## Phase 8 — Album Art Base URL + Rendering

### Implementation
1. Add settings field `albumArtBaseUrl` with default:
   - `https://dzblobstor1.blob.core.windows.net/albumart/pictures/`
2. Compose art URL from base URL + `songlist.picture` filename.
3. Show album art preview in song edit flow.
4. Show album art in DECK A/B header before title.
5. Add fallback image/placeholder for missing picture.

### Acceptance checks
- Settings persists base URL.
- Songs with picture filenames render art on edit and deck headers.
- Missing/bad image URLs gracefully fallback.

---

## Phase 9 — Listener Count in Top Bar + History Snapshot

### Implementation
1. Surface current listeners in `TopBar` from existing analytics stream (Shoutcast/Icecast source).
2. Extend history write path so each new history entry stores listener count snapshot from that moment.
3. Ensure non-blocking behavior in playback/audio paths.

### Acceptance checks
- Top bar reflects current listener count.
- Newly inserted history rows contain listener count values.

---

## Phase 10 — Verification, Hardening, and Rollout

### Frontend checks
- `npm run build`

### Backend checks
- `cd src-tauri && cargo check`
- `cd src-tauri && cargo test`
- `cd src-tauri && cargo fmt --all`
- `cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings`

### Manual QA matrix
- Drag/drop: library+queue → A/B/AUX1/AUX2
- Queue reorder persist
- CUE set/go
- SYNC both directions
- Logs activity + error visibility
- EQ slider reset
- Requests pending + add to queue
- Album art display in editor and deck headers
- Listener count top bar + history snapshot

---

## Recommended Delivery Strategy

1. **PR 1:** Drag/drop + queue reorder.
2. **PR 2:** CUE/SYNC fixes + logs tab.
3. **PR 3:** Requests + album art + listeners/history snapshot.

This keeps risk localized and simplifies QA/rollback.
