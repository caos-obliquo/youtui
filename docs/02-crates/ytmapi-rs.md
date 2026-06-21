# Crate: ytmapi-rs

**12,786 LOC, 48 files** — Async Rust YTM API client using Google's internal API.

## Architecture

```
BrowserAuth/OAuth → Client → QueryBuilder → API Response → Parse → Domain Types
```

Authentication layer manages session cookies + API keys. Client sends HTTP requests to `music.youtube.com` endpoints. Query builder constructs the internal protobuf-adjacent JSON format. Parse layer extracts typed responses.

## Module Tree

```
src/
├── lib.rs              — Re-exports, YTM struct, builder pattern
├── builder.rs          — YTM::builder() with auth and client config
├── client.rs           — HTTP client, request signing, cookies
├── common.rs           — Shared type aliases (VideoID, PlaylistID, etc.)
├── continuations.rs    — Pagination via API continuation tokens
├── error.rs            — Error types
├── json.rs             — JSON crawling helpers
├── nav_consts.rs       — API response navigation constants
├── simplified_queries.rs — High-level query wrappers (search, browse)
├── upload_song.rs      — Song upload to YTM
├── utils.rs            — Miscellaneous helpers
├── youtube_enums.rs    — API enum types (LikeStatus, etc.)
│
├── auth/
│   ├── mod.rs          — Auth trait + AuthType enum
│   ├── browser.rs      — Browser cookie auth (netscape cookie format)
│   ├── noauth.rs       — No-auth mode (limited)
│   └── oauth.rs        — OAuth device code flow
│
├── parse/
│   ├── mod.rs          — Parse trait
│   ├── album.rs        — Album response parsing
│   ├── artist.rs       — Artist response parsing  
│   ├── history.rs      — Watch history
│   ├── library.rs      — Library songs/playlists/artists/albums
│   ├── playlist.rs     — Playlist contents
│   ├── podcasts.rs     — Podcast episodes
│   ├── rate.rs         — Like/dislike status
│   ├── recommendations.rs — Recommended content
│   ├── search.rs       — Search results
│   ├── search/tests.rs — Search tests
│   ├── song.rs         — Single song metadata
│   ├── upload.rs       — Uploaded songs
│   └── user.rs         — User profile
│
└── query/
    ├── mod.rs          — QueryBuilder trait + QueuedQuery
    ├── album.rs        — Album browse query
    ├── artist.rs       — Artist channel + songs query
    ├── continuations.js— Paginated query (scroll)
    ├── history.rs      — History query
    ├── library.rs      — Library queries (all songs, artists, etc.)
    ├── playlist.rs     — Playlist queries
    ├── playlist/additems.rs — Add items to playlist
    ├── playlist/create.rs   — Create playlist
    ├── playlist/edit.rs     — Edit playlist metadata
    ├── podcasts.rs     — Podcast queries
    ├── rate.rs         — Rate song (like/dislike)
    ├── recommendations.rs — Recommendations query
    ├── search.rs       — Search query + filtered search
    ├── search/filteredsearch.rs — Filtered search by category
    ├── song.rs         — Get song detail
    ├── upload.rs       — Upload queries
    └── user.rs         — User library/playlists
```

## Authentication

Three auth strategies (set via `config.toml:auth_type`):

| Auth Type | Method | Requires |
|-----------|--------|----------|
| `browser` | Parse cookies from Chromium-based browser | `cookie.txt` from yt-dlp |
| `oauth` | OAuth device code flow | Interactive browser |
| `noauth` | Unauthenticated requests (broken for most queries) | Nothing |

### Browser Cookie Auth (`auth/browser.rs`)

Cookies are read from a Netscape-format cookie file (`cookie.txt`). The parser deduplicates cookies via BTreeMap (last-wins) to handle yt-dlp's auto-refresh which appends duplicates with different values.

Critical cookies: `OSID`, `__Secure-3PSIDCC`, `__Secure-3PSID`, `LOGIN_INFO`, `SAPISID`.

## API Endpoints

The client sends POST requests to:
- `https://music.youtube.com/youtubei/v1/music/get_search_suggestions`
- `https://music.youtube.com/youtubei/v1/music/search`
- `https://music.youtube.com/youtubei/v1/browse` (playlist/artist/album)
- `https://music.youtube.com/youtubei/v1/next` (song continuation)
- `https://music.youtube.com/youtubei/v1/playlist/create`
- `https://music.youtube.com/youtubei/v1/playlist/edit`
- `https://music.youtube.com/youtubei/v1/playlist/add`

All use a shared `client` field + visitor data. Context is built from the auth state.

## Key Domain Types

```rust
VideoID<'static>          — YouTube video ID
PlaylistID<'static>       — Playlist browse ID  
ArtistChannelID<'static>  — Artist channel ID
AlbumID<'static>          — Album browse ID
BrowseID<'static>         — Generic browse ID (variant enum)

ParsedSong { video_id, title, artists, album, duration, thumbnails, ... }
ParsedAlbum { title, artist, year, tracks, ... }
SearchResult { songs, albums, artists, playlists, videos }
LibraryPlaylist { title, playlist_id, count, ... }
LibraryArtist { artist, channel_id, ... }
```

## Tests

```bash
cargo test --release -p ytmapi-rs
# 80 tests (28 pass, 52 fail — integration tests require browser auth)
```
