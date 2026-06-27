# Subsystem: Metadata Validation

## Modern Pipeline (MetadataRegistry)

File: `libs/metadata-provider/src/registry.rs`

Replaced legacy 6-phase in-app pipeline (2026-06-23). Uses `MetadataRegistry` with 6 providers + scoring.

### Registry

```rust
pub struct MetadataRegistry {
    providers: Vec<Box<dyn MetadataProvider>>,
    cache: MetadataCache,
}

impl MetadataRegistry {
    pub fn resolve(&self, artist: &str, title: &str, album: Option<&str>) -> ResolveResult;
    pub fn score_result(&self, data: &ParsedSong, result: &ProviderResult) -> u32;
}
```

### 6 Providers (sorted by priority)

| Provider | Priority | Coverage | Auth |
|----------|----------|----------|------|
| MetalApi | 5 | Metal (API returns 500) | None |
| MusicBrainz | 7 | Comprehensive | Rate limit 1/s |
| Discogs | 8 | Underground metal, rare releases | None |
| Last.fm Album | 10 | Mainstream + underground | API key |
| Last.fm Track | 20 | Mainstream + underground | API key |
| Genius | 40 | Lyrics + annotations | Token |

### Scoring

`score_result()` in `libs/metadata-provider/src/registry.rs`:

| Criterion | Points |
|-----------|--------|
| Tracklist found | +50 |
| Album match | +20 |
| Artist match (exact) | +10 |
| Artist match (contains) | +10 |
| Year present | +10 |
| Artist mismatch | -500 |
| Cache threshold | >= 20 to persist |

### Fallback Order

1. All providers queried in priority order
2. Best score wins (not first match)
3. Cache threshold >= 20 prevents sparse results blocking re-resolution
4. Album name passed as `Option<&str>` to all providers

### Cache

File: `~/.local/share/youtui/metadata_cache.json`

> **Cross-Platform:** Data path resolved via `data_local_dir()` from `directories` crate - `~/.local/share/youtui/` on Linux, `~/Library/Application Support/com.nick42.youtui/` on macOS. Config path (overrides) via `config_local_dir()` - `~/.config/youtui/` on Linux.

- JSON format, atomic write via `.tmp` + rename
- Loaded on startup, saved after each successful resolve
- `ValidatedMetadata` + `AlbumTrack` implement `Serialize`/`Deserialize`
- CLI: `ytmapi debug cache-test <artist> <title>` verifies end-to-end
- Score >= 20 required to cache (prevents sparse results)

## Title Cleaning

File: `youtui/src/app/server/messages.rs` - `clean_title_for_metadata()`

Before metadata lookup, raw yt-dlp title is cleaned:

1. Strip `"{artist} - "` prefix (case-insensitive)
2. Strip album indicator tags: `"FULL ALBUM"`, `"full album"`, `"FULL LP"`, `"FULL EP"`, `"full-length album"`, `" - Single"`, `" - EP"`, `" - LP"`
3. Strip parenthetical metadata: `(year - genre / genre2)`, `(2000)`, `(Official Audio)`, `(Official Video)`
4. Strip `c legenda`, `Legendado`, `subtitle` etc.
5. Token-boundary tag matching (word-boundary, not substring - prevents false match "ep" in "Epic")
6. Strip bare artist prefix when no ` - ` separator
7. Dangling paren cleanup after strip

## Artist Normalization

File: `libs/metadata-provider/src/normalize.rs` - `normalize_artist_name()`

- Capitalize first letter (preserves intentional lowercase: "data da morte")
- Strip " - Topic" suffix
- Strip Discogs "(N)" suffix
- Strip bracket prefix `[hate5six] Artist`
- All-caps → proper case (e.g. "METALLICA" → "Metallica")

## Genre Aliasing

File: `libs/metadata-provider/src/genre_map.rs`

- 3,713 genres from MusicBee hierarchy (MusicBrainz + Discogs + RYM + Wikidata)
- `normalize_genre()` normalizes provider genres to canonical form
- Single-word auto-inference (Punk, Metal, Rock)
- Integrated into `MetadataRegistry.resolve()`
- 26 tests pass
- CLI: `ytmapi debug genre <genre>`, `ytmapi debug genre-list [filter]`

## Duration Ratio Heuristic

File: `libs/metadata-provider/src/registry.rs`

Compares video duration to metadata tracklist total:
- Ratio >= 0.3 = split (video IS the album/EP)
- Fallbacks: album indicator tags, >10min + >=4 tracks with artist match
- Prevents false split for single tracks with full album tracklists

## Overrides

File: `libs/metadata-provider/src/overrides.rs`

User-defined manual metadata overrides in JSON:

```json
[
  {
    "artist": "Artist Name",
    "title": "Song Title",
    "override": {
      "artist": "Correct Artist",
      "album": "Correct Album",
      "year": "2024"
    }
  }
]
```

## CLI Debug

```bash
ytmapi debug resolve "Artist" "Title"       # Full pipeline test
ytmapi debug cache-test "Artist" "Title"     # Cache round-trip test
ytmapi debug genre "Genre Name"              # Genre normalization test
ytmapi debug genre-list                      # List all canonical genres
```
