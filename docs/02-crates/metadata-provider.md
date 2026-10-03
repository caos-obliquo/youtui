# metadata-provider

**48 tests, 0 warnings.**

Metadata resolution crate: queries 8 external providers to resolve artist/album/year/
tracklist/genre for YouTube Music songs. Used by the album splitting pipeline.

## Providers (in priority order)

| Provider | Priority | Token Needed | Coverage |
|----------|----------|--------------|----------|
| MetalApi (metal-api.dev primary, localhost:5000 proxy fallback) | 5 | None (metal-api.dev currently returns 500; direct MA access needs MA_COOKIE) | Metal bands |
| ListenBrainz | 6 | `listenbrainz_token` in config for full data | Year + genres + styles in one call |
| MusicBrainz | 7 | None | Widest coverage, rate limit 1/s via shared Semaphore in util::musicbrainz_limiter |
| Discogs | 8 | `discogs_token` in config | Broad music catalog, Master API |
| Libre.fm | 8 | `api_key` in config | Libre.fm album data |
| Last.fm AlbumSearch | 10 | `api_key` in config | album.getInfo, tracklists |
| Last.fm TrackSearch | 20 | `api_key` in config | track.getInfo, album/year/track_no |
| Genius | 40 | `genius_token` in config | Song metadata |

## Scoring

All providers are queried and the best-scoring result wins (`MetadataRegistry::resolve` in `libs/metadata-provider/src/lib.rs`, `score_result` at lib.rs:121):

- artist exact match +50 (partial contains +10)
- tracklist present +100 if artist matches, +80 otherwise
- album name present +10
- year present +5
- album == title +15 (partial contains +7), `&` vs `and` normalized equality +10
- +1 per track up to +10
- +4 per genre up to +20
- artist mismatch with no album match -500

Minimum score for caching: >= 20. Prevents stale sparse results from blocking
re-resolution.

## Cache

Two-layer cache (`libs/metadata-provider/src/lib.rs`):

- LRU-200 in memory (`LruCache::new(200)`)
- PLUS SQLite disk cache (`SqliteCache`, file `metadata_cache.db` under the data dir)
- Lookup order: LRU, then SQLite (populates LRU on hit), then HTTP fetch
- Background flush of LRU to SQLite every 60s (`start_background_flush`, lib.rs:493) and on quit (`flush_cache_to_sqlite`)
- JSON file (`metadata_cache.json`) written only as fallback when SQLite is absent (lib.rs:286)

## Genre Aliasing

File: `src/genre_map.rs`

- Genre count asserted as `all.len() > 3000` in `test_all_genres_loaded` (genre_map.rs:277); RYM fallback covers 5,977+ genres
- `normalize_genre()` normalizes provider genres to canonical forms
- 12 `#[test]` functions in genre_map module
- Auto-inference: first word of multi-word canonicals maps to parent genre
  (e.g., "indie rock" auto-maps to "Indie")

## What Was Tried and Abandoned

- **metal-api.dev**: Public MA REST API (primary) plus optional localhost:5000 Rust proxy fallback (metal_api.rs:33-34). metal-api.dev currently returns 500. Only MA_COOKIE direct HTTP access works.
- **Per-track validation**: Spawning separate lookups for each split track.
  Overwrote correct artist/album with unrelated results. Removed.
- **Tag-only split gate**: Required YouTube title tags like `[Full Album]`.
  Missed official label uploads. Replaced with duration ratio heuristic.

## Build & Test

```bash
cargo test --release -p metadata-provider    # 48 pass
```

Located at `libs/metadata-provider/` in workspace root. Part of the 7-member workspace (13 crate directories on disk).
