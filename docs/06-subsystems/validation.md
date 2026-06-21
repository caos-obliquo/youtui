# Subsystem: Metadata Validation

## Pipeline

File: `app/server/messages.rs:730` — `ValidateMetadata::into_future`

Progressive search with 6 phases. Stops at first success.

```
Phase 1: Last.fm album.search → album.getInfo       ← 17 tracks
Phase 2: Last.fm track.getInfo(artist, title)       ← exact match
Phase 3: Last.fm track.search(norm_for_lfm(title))  ← fuzzy → re-fetch for album
Phase 4: Discogs API (no auth, underground metal)   ← fetch_album_tracks
Phase 5: Last.fm album.search fallback              ← fetch_album_tracks
Phase 6: MusicBrainz recording search               ← 1 req/s rate limit
```

## norm_for_lfm

File: `app/server/providers/util.rs`

Normalizes messy YouTube titles for database queries. Strips in order:

1. `"FULL ALBUM"`, `"Full Album"`, `"full album"`, `"FULL LP"`, `"FULL EP"`, `"full-length album"`
2. `" - Single"`, `" - EP"`, `" - LP"`, `" - full album"`
3. Parenthesized blocks: ` (year - genre / genre2)`, ` (2000)`
4. Bracketed blocks: ` [genre]`, ` [HD]`
5. Replaces ` & ` → ` and ` for Last.fm compatibility

## add_yt_video Title Cleaning

File: `app/ui/playlist.rs:735`

Before `ValidateMetadata` is spawned, raw yt-dlp title is cleaned:

1. Strip `"{artist} - "` prefix (case-insensitive)
2. Strip `"FULL ALBUM"` suffix (case-insensitive)
3. Strip `"  ("` suffix (parenthetical metadata)

## fetch_album_tracks

File: `app/server/messages.rs:926`

Three-phase fallback for getting full tracklists when a song is identified as part of an album:

- **Phase 1**: Last.fm `album.getInfo` (requires API key)
- **Phase 2**: Discogs API (no auth, works for underground extreme metal)
- **Phase 3**: Last.fm `album.search` → re-fetch `album.getInfo`

### Discogs API

```rust
GET https://api.discogs.com/masters/{id}
GET https://api.discogs.com/release/{id}
```

No authentication required. Parses `tracklist` array with `title` and `duration` fields.
Returns `Vec<TrackInfo { title, duration }>`.

### MusicBrainz

```rust
GET https://musicbrainz.org/ws/2/recording?query={query}&fmt=json
```

Rate limited: 1 request per second. Uses `norm_for_lfm` for query construction.

## Providers

All metadata providers implement the `MetadataProvider` trait:

```rust
trait MetadataProvider {
    fn search_album(&self, artist: &str, album: &str) -> Result<AlbumInfo>;
    fn search_track(&self, artist: &str, track: &str) -> Result<TrackInfo>;
}
```

File: `app/server/providers/mod.rs`

| Provider | Auth | Covers |
|----------|------|--------|
| Last.fm | API key | Mainstream + underground |
| Discogs | None | Underground metal, rare releases |
| Genius | Bearer token | Song metadata (not lyrics) |
| MusicBrainz | 1 req/s | Comprehensive, rate-limited |
| Overrides | Manual file | User-defined corrections |

## Overrides

File: `app/server/providers/overrides.rs`

User can define manual metadata overrides in a file. Format:

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
