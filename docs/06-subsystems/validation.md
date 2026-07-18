# Metadata Validation & Enrichment

## Overview

The metadata validation pipeline resolves song metadata (year, genres, styles, album, track list) from multiple providers, scores results, merges by weighted consensus, and caches persistently.

Flow: Song queued -> Metadata providers queried -> Results scored -> Genre merge (cumulative weights) + Year merge (provider-weighted vote) -> Cache result (LRU + SQLite) -> Album split (if album tracks found) -> Album art fetch (YTM -> MB CAA -> Last.fm)

## Providers & Priorities

Registered in `MetadataRegistry::new()` (`libs/metadata-provider/src/lib.rs`). Sorted by priority on init.

| Provider | Priority | Genre Wt | Style Wt | Limits |
|----------|----------|----------|----------|--------|
| Genius | 40 | -- | -- | Genius API token, lyrics+annotations |
| TrackSearch (Last.fm) | 20 | 1 | 0 | None |
| AlbumSearch (Last.fm) | 10 | 1 | 0 | 1 req/s configurable |
| Discogs | 8 | 1 | 0 | Rate limited, needs token |
| LibreFM | 8 | 1 | 0 | None |
| MusicBrainz | 7 | 3 | 0 | 1 req/s (limiter), OAuth2 |
| ListenBrainz | 6 | 2 | 1 | User token required |
| MetalAPI | 5 | 1 | 0 | Backend 500 errors (unavailable) |

LibreFM and ListenBrainz are conditionally added (only when token/key configured).

## Scoring System

Every provider result gets a score via `score_result()` in `libs/metadata-provider/src/lib.rs`:

| Criterion | Points |
|-----------|--------|
| Artist exact match | +50 |
| Artist partial (contains) | +10 |
| Album tracks + artist match | +100 |
| Album tracks (no artist match) | +80 |
| Album name present | +10 |
| Year present | +5 |
| Album name == title | +15 |
| Album name contains title | +7 |
| Album normalized (and/&) | +10 |
| Per track (up to 10) | +1 each |
| Wrong artist penalty | -500 |

Result with highest score wins. Results with score <= 0 are discarded. JSON cache persistence gated at score >= 20 (but SQLite writes always happen).

## Genre Merge (weighted_merge_genres)

File: `libs/metadata-provider/src/merge.rs`.

Cumulative weight accumulation using `HashMap<String, (String, u8)>`:

- MusicBrainz (priority 7): genre weight = 3, style weight = 0
- ListenBrainz (priority 6): genre weight = 2, style weight = 1
- All others: genre weight = 1, style weight = 0

All weights accumulate via `and_modify`/`saturating_add`. After accumulation, genres sorted by weight desc, then alphabetically. **Capped at 30** each for genres and styles. Parent genres expanded via `GenreDb::expand_parent_genres()`. Final dedup by `name_lower`.

## Year Merge (merge_year)

File: `libs/metadata-provider/src/merge.rs`.

Provider-weighted voting with `HashMap<String, u8>`:

- MusicBrainz (priority 7): 3 votes
- ListenBrainz (priority 6): 2 votes
- All others: 1 vote

Year with highest cumulative weight wins. Tie-break: best-scoring provider's year (highest `score_result` value). Years must be 4-digit (validated). Returns `None` if no provider returned a year.

## Genre Database (genre-db-sqlite)

Crate at `libs/genre-db-sqlite/`. Singleton `GenreDb` via `OnceLock`:

- 10,500+ genres seeded from MusicBee (3,729), Discogs, RYM (6,163)
- Parent hierarchy (e.g. "thrash metal" -> "metal")
- `normalize_genre(name)`: case-insensitive, fuzzy match via trigram fallback (RYM)
- `expand_parent_genres(genres)`: returns input + parent genres for each
- `is_known_genre(name)`: checks exact in GenreDb, then RYM `find_genre()`
- `get_ancestors(name)`: walks parent chain to root

Integration: `genre_map::normalize_genres()` normalizes all provider genres before merge. `expand_parent_genres()` called after merge.

## MusicBrainz OAuth2 + Genre Fetch

- Device flow OAuth2 with user code displayed in CLI
- Token storage: `bearer_token` + `refresh_token` in config
- Auto-refresh: `Arc<Mutex<(Option<String>, Option<String>)>>` with 401 retry
- `fetch_release_group_genres(mbid)`: extracts genre/style tags from MB release groups
- Rate limiter: 1 req/s

## ListenBrainz Genre Validation

- Tags from LB classified as genres (not styles) when matching `is_known_genre()`
- Tags with `genre_mbid` from LB always promoted to genres
- Non-matching tags classified as styles

## MB Cover Art Archive

Fallback pipeline: YTM thumbnail -> MB CAA -> Last.fm.

File: `youtui/src/app/server/messages.rs` - `FetchAlbumArt`.

- Fetches from `https://coverartarchive.org/release-group/{mbid}/front`
- No auth required for CAA
- `release_mbid`: `Option<String>` field on `ListSong` / `ValidatedMetadata.musicbrainz_release_group_id`
- MBID captured from MusicBrainz provider during validation (`meta.musicbrainz_release_group_id`)
- On success: saves via `SongThumbnailDownloader.download_song_thumbnail_from_bytes()` with thumb ID `caa:{mbid}`
- On failure: falls through to Last.fm `album.getInfo`

## Cache Architecture

Two-layer cache:

1. **LRU (in-memory)**: `LruCache<String, ValidatedMetadata>` with 200 entries (`NonZeroUsize::new(200)`). Fast lookups, thread-safe via `Mutex`.
2. **SQLite (disk)**: `metadata-cache-sqlite` crate, `~/.local/share/youtui/metadata_cache.db`. Persists across restarts. `PRAGMA synchronous = NORMAL`.

Lookup chain: LRU hit -> return -> SQLite hit -> populate LRU -> return -> HTTP fetch -> cache both layers.

Write behavior:
- **SQLite**: immediate write on every `resolve()` and `resolve_fast()` success (no score gate)
- **JSON cache** (`save_cache()`): only when score >= 20 (prevents sparse entries from blocking re-resolution)
- **Background flush**: thread spawned via `start_background_flush()` copies LRU -> SQLite every 60s
- **On quit**: `flush_cache_to_sqlite()` called in `AppStatus::Exiting` handler

## Year Enrichment Pipeline

Year enrichment provides metadata for songs that lack it from YTM.

**Library paths:**

- `HandleLibrarySongsOk` -> `EnrichFromMetadataCache(EnrichTarget::LikedSongs)` -> `resolve_fast()` -> `SongsEnriched(LikedSongs)` -> update song list
- `HandleLibraryPlaylistTracksOk` -> `PopulateSetIds` -> `EnrichFromMetadataCache(EnrichTarget::PlaylistTracks)` -> `resolve_fast()` -> `SongsEnriched(PlaylistTracks)` -> update playlist tracks

**Queue path:**

- `push_song_list` -> `EnrichQueueYears` -> `resolve_fast()` -> `HandleQueueEnrichYearsOk` -> update songs by index map

**resolve_fast()**: Filters providers to fast ones only: ListenBrainz (priority 6) + Last.fm Album (10) + Last.fm Track (20). No rate-limit bottleneck. Skips MB (7), Discogs (8), Genius (40), MetalApi (5), LibreFM (8). Always caches result (even None) to prevent re-fetch.

**EnrichTarget enum**: `LikedSongs`, `PlaylistTracks` -- used for race-safe routing at completion.

File: `youtui/src/app/server/messages.rs` lines 359-365:
```rust
pub enum EnrichTarget {
    LikedSongs,
    PlaylistTracks,
}
```

## Album Art Pipeline

1. **YTM thumbnail** from `song.thumbnails` (largest URL) via `SongThumbnailDownloader.download_song_thumbnail()`
2. **MB CAA** (if `release_mbid` present) via `https://coverartarchive.org/release-group/{mbid}/front`
3. **Last.fm** via `FetchAlbumArt` (requires matching album name, uses `album.getInfo`)
4. None -- no art available

## CLI Tools

| Command | Purpose |
|---------|---------|
| `youtui test-musicbrainz --artist "X" --title "Y"` | Test MB recording lookup + genre fetch |
| `youtui test-caa --release-group-id "MBID"` | Test CAA album art download |
| `youtui test-listenbrainz --artist "X" --title "Y"` | Test LB tag lookup |
| `youtui test-validate-metadata --artist "X" --title "Y" --album "Z"` | Test full multi-provider resolution |
| `youtui enrich-cache [--artist "X" --title "Y"]` | Batch enrich cache from stdin or args |
| `youtui test-scrobble --artist "X" --title "Y" --album "Z" --duration N` | Test Last.fm scrobble API |
| `youtui scrobble-cache --show/--clear/--retry` | Manage failed scrobble retry queue |

## Test Coverage

- `metadata-provider`: 110 tests
- `genre-db-sqlite`: 27 tests
- `metadata-cache-sqlite`: 20 tests (roundtrip mbid, migration, put_batch)
