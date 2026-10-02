# Metadata Validation

Youtui resolves metadata through a multi-provider pipeline. Results are scored, merged, and cached.

## Metadata Providers

The `MetadataRegistry` queries 8 providers. Lower priority number = checked first, but all providers are queried and the best score wins (see Scoring Formula).

| Priority | Provider | Requires | Status |
|---|---|---|---|
| 5 | MetalApiProvider | `MA_COOKIE` env var | **DEAD** - API returns 500 |
| 6 | ListenBrainzProvider | `listenbrainz_token` | Active |
| 7 | MusicBrainzProvider | nothing (OAuth2 optional) | Active |
| 8 | DiscogsProvider | `discogs_token` | Active |
| 8 | LibreFMProvider | `librefm_key` | Reserved (future use) |
| 10 | AlbumSearchProvider (Last.fm) | `api_key` | Active |
| 20 | TrackSearchProvider (Last.fm) | `api_key` | Active |
| 40 | GeniusProvider | `genius_token` | Active |

All providers are tried and scored; the highest score wins. Year, album, artist, and genre/style data are merged across all results (see Genre Merge Pipeline). There is no early stop after a winner is found.

## Scoring Formula

Each provider result gets a confidence score:

| Signal | Points | Condition |
|---|---|---|
| artist_match | +50 | Artist name exact match (+10 for substring either way) |
| tracklist | +100 | With artist match |
| tracklist | +80 | Without artist match |
| album | +10 | Album name present |
| year | +5 | Release year present |
| album_title_match | +15 | Album title equals query (+7 contains, +10 `&`/`and` normalized equal) |
| track_count | +1 per track | Max +10 |
| genre | +4 per genre | Max +20 |
| wrong_artist | -500 | Artist mismatched AND album does not match title |

Results with score <= 0 are discarded. Highest score wins.

## Genre Merge Pipeline

Genres from all providers are merged in three stages:

### 1. Weighted Merge

Each provider contributes genres and styles with a weight derived from its registry priority (`priority_weight` / `weighted_merge_genres` in `metadata-provider/src/merge.rs`):

| Source | Genre weight | Style weight | Notes |
|---|---|---|---|
| MusicBrainz (priority 7) | 3 | 0 | Authoritative genre tags |
| ListenBrainz (priority 6) | 2 | 1 | Community-voted |
| All other providers | 1 | 0 | Last.fm, Discogs, etc. |

Weights accumulate per tag. Dedup by lowercase. Sorted by weight descending, then alphabetically ascending.

### 2. Cap

- Max 30 genres
- Max 30 styles

### 3. RYM Parent Expansion

After weighted merge, RYM parent expansion runs:

- Every genre is looked up in the RYM genre hierarchy
- Parent genres are added if missing
- Example: `death metal` → also adds `metal`, `extreme metal`

This runs on the capped lists, then the cap is re-applied (`take(30)` after `expand_parent_genres` in `merge.rs`), so the final lists stay within 30.

**Performance note**: `genre_map` contains 3000+ canonical genres. Parent expansion is a linear scan per genre. See limitation F6.

## Cache

Two-tier caching:

| Tier | Size | Behavior |
|---|---|---|
| LRU | 200 entries | In-memory hot cache |
| SQLite | Unlimited | Write-through on resolve() hit |

Location: `~/.local/share/youtui/metadata_cache.db` (via `get_data_dir()`, overridable with `YOUTUI_DATA_DIR`)

Clear by deleting the `.db` file.

## Known Limitations

| ID | Limitation | Impact | File |
|---|---|---|---|
| F6 | `genre_map` iteration contains 3000+ canonical genres, linear scan per parent expansion | Genre merge O(n) per expansion | `metadata-provider/src/genre_map.rs` |
| F7 | LibreFM `librefm_key` config field unused (reserved for future Libre.fm scrobbling) | No functional impact | `config.rs` |
| F8 | MetalApi provider is dead code (API returns 500, provider still registered) | Wasted priority-5 slot in registry | `metadata-provider/src/metal_api.rs` |
| F9 | Rate limiter has no logging for wait times or throttle events | Silent delays, hard to debug | `metadata-provider/src/lib.rs` |
| F10 | No early-stop optimization: all 8 providers queried even after score winner found | Wasted API calls, slower resolution | `metadata-provider/src/lib.rs` |
| F11 | LRU + SQLite caches not invalidated when config changes (token update requires restart) | Stale cache after token rotation | `metadata-cache-sqlite/src/lib.rs` |
| F12 | MusicBrainz OAuth bearer token not auto-refreshed | Manual re-auth when token expires | `metadata-provider/src/musicbrainz.rs` |

See also [08-known-issues.md](08-known-issues.md) for runtime issues and workarounds.
