# Feature Fixes Plan

## Completed

- [x] Implement event log filtering for level, category, time range, and text search in `get_event_log`.
- [x] Expand event log filtering to include metadata-aware search and deck-specific queries.
- [x] Implement report generation scaffolding and wire report summaries/sections to real local analytics data.
- [x] Enrich top-song report labels with SAM metadata (artist/title) when MySQL is connected.
- [x] Implement CSV export to produce a real file in the system temp directory.
- [x] Add tests for report CSV export helpers.
- [x] Add SQLite fixture-based integration-style test coverage for `get_event_log` filtering.

## Next Candidates

- [ ] Add percentile/rolling aggregates for listener trend reports.
- [ ] Add export formats beyond CSV (JSON/PDF).
