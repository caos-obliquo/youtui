# Future TODOs

## Bugs (Pre-existing)
- 54 integration tests fail — YT API format mismatches, not our bug
- Artist album pagination returns only first page — needs `ParseFromContinuable` impl

## Deprecation & Maintenance
- 27 BasicSearch deprecation warnings from upstream (#353) — needs migration
- `cargo update` — 219 transitive updates available (minor/patch bumps, safe)
- Upstream removed `AudioQuality` from structures.rs — if they finalize removal, adapt

## Performance (Minor)
- `compute_artists_string` duplicated in footer + table every draw
