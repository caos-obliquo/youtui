# metadata-provider

**47 tests, 0 warnings.**

Metadata resolution crate: queries 6 external APIs to resolve artist/album/year/
tracklist/genre for YouTube Music songs. Used by the album splitting pipeline.

## Providers (in priority order)

| Provider | Priority | Token Needed | Coverage |
|----------|----------|--------------|----------|
| MA_COOKIE (Metal Archives) | 5 | `MA_COOKIE` env | Metal/rock bands, full tracklists |
| Discogs | 8 | `discogs_token` in config | Broad music catalog, Master API |
| Last.fm AlbumSearch | 10 | `api_key` in config | album.getInfo, tracklists |
| YTM Album Enrichment | 15 | (uses YTM client) | Post-registry fallback from YTM |
| Last.fm TrackSearch | 20 | `api_key` in config | track.getInfo, album/year/track_no |
| Genius | 40 | `genius_token` in config | Song metadata (tracklist) |
| MusicBrainz | 50 | None | Widest coverage, last resort |

## Scoring

`MetadataRegistry::score_result()` weights each provider result:

- **+50** tracklist present
- **+20** album name matches (with `&` vs `and` normalization)
- **+10** artist exact match
- **+10** year matches or present

Minimum score for caching: >= 20. Prevents stale sparse results from blocking
re-resolution.

## Cache

- File: `~/.local/share/youtui/metadata_cache.json`
- LRU capacity: 200 entries
- Persistence: atomic write (`.json.tmp` + rename)
- No TTL (manual cache clear if stale data)

## Genre Aliasing

File: `src/genre_map.rs`

- 3,713 genres from MusicBee hierarchy (MusicBrainz + Discogs + RYM + Wikidata)
- `normalize_genre()` normalizes provider genres to canonical forms
- 26 tests in genre_map module
- Auto-inference: first word of multi-word canonicals maps to parent genre
  (e.g., "indie rock" auto-maps to "Indie")

## What Was Tried and Abandoned

- **metal-api.dev**: Public MA REST API, approved. Returns 500 errors. Provider
  code written but API broken. Only MA_COOKIE direct HTTP access works.
- **Per-track validation**: Spawning separate lookups for each split track.
  Overwrote correct artist/album with unrelated results. Removed.
- **Tag-only split gate**: Required YouTube title tags like `[Full Album]`.
  Missed official label uploads. Replaced with duration ratio heuristic.

## Build & Test

```bash
cargo test --release -p metadata-provider    # 47 pass
```

Located at `libs/metadata-provider/` in workspace root. Part of the 11-crate workspace.
