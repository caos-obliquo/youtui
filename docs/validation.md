# Metadata Validation

Youtui resolves metadata through a multi-provider pipeline. Results are scored, merged, and cached.

## Metadata Providers

The `MetadataRegistry` queries 8 providers in priority order. Lower priority = checked first.

| Priority | Provider | Requires | Status |
|---|---|---|---|
| 2 | TrackSearchProvider (Last.fm) | `api_key` | Active |
| 3 | AlbumSearchProvider (Last.fm) | `api_key` | Active |
| 4 | DiscogsProvider | `discogs_token` | Active |
| 5 | MetalApiProvider | `MA_COOKIE` env var | **DEAD** - API returns 500 |
| 6 | ListenBrainzProvider | `listenbrainz_token` | Active |
| 7 | MusicBrainzProvider | nothing (OAuth2 optional) | Active |
| 8 | LibreFMProvider | `librefm_key` | Reserved (future use) |
| 9 | GeniusProvider | `genius_token` | Active |

First provider to return a result wins per field. Tracklist and genre data are merged across all providers.

## Scoring Formula

Each provider result gets a confidence score:

| Signal | Points | Condition |
|---|---|---|
| artist_match | +50 | Artist name matches query |
| tracklist | +100 | With artist match |
| tracklist | +80 | Without artist match |
| album | +10 | Album name present |
| year | +5 | Release year present |
| album_title_match | +15 | Album title matches query |
| track_count | +1 per track | Max +10 |
| genre | +2 per genre | Max +10 |
| wrong_artist | -500 | Artist clearly mismatched |

Results below a threshold are discarded. Highest score wins.

## Genre Merge Pipeline

Genres from all providers are merged in three stages:

### 1. Weighted Merge

Each provider contributes genres with a weight:

| Source | Weight | Notes |
|---|---|---|
| MusicBrainz | 3 | Authoritative genre tags |
| ListenBrainz genre | 2 | Community-voted |
| ListenBrainz style | 1 | Or 2 if count >= 10 |
| All other providers | 1 | Last.fm, Discogs, etc. |

Dedup by lowercase. First insertion wins for display order.

### 2. Cap

- Max 20 genres
- Max 20 styles

### 3. RYM Parent Expansion

After weighted merge, RYM parent expansion runs:

- Every genre is looked up in the RYM genre hierarchy
- Parent genres are added if missing
- Example: `death metal` → also adds `metal`, `extreme metal`

This runs AFTER the cap, so parents may push the list back over 20. The cap is not re-applied.

**Performance note**: `genre_map` contains 3000+ canonical genres. Parent expansion is a linear scan per genre. See limitation F6.

## Cache

Two-tier caching:

| Tier | Size | Behavior |
|---|---|---|
| LRU | 200 entries | In-memory hot cache |
| SQLite | Unlimited | Write-through on resolve() hit |

Location: `~/.local/share/youtui/metadata_cache.sqlite`

On first open, the old `metadata_cache.json` is migrated to `.bak` and imported into SQLite.

Clear by deleting the `.sqlite` file.

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
