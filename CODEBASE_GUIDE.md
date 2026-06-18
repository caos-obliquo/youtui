# Youtui Codebase Overview

Developer guide to understanding and extending youtui.

## Project Structure

```
youtui/
├── youtui/                # Main TUI application
│   └── src/
│       ├── main.rs        # Entry point, cookie loading, CLI args
│       ├── app.rs         # Application loop, task routing
│       ├── config/        # Config deserialization, keymap
│       ├── app/
│       │   ├── ui/        # Ratatui TUI components
│       │   │   ├── playlist.rs          # Main playback queue (~2200 lines)
│       │   │   ├── playlist/
│       │   │   │   ├── effect_handlers.rs        # Playlist effect dispatch
│       │   │   │   ├── effect_handlers_playlist.rs # Metadata/lyrics/album effects
│       │   │   │   ├── lyrics_popup.rs
│       │   │   │   ├── tests.rs                  # 17 playlist unit tests
│       │   │   │   └── ... (browser, footer, etc.)
│       │   │   └── footer.rs, draw_media_controls.rs
│       │   ├── server/    # Backend tasks
│       │   │   ├── messages.rs           # ALL backend tasks (~1420 lines)
│       │   │   ├── player.rs             # Audio decode + ffmpeg extraction
│       │   │   ├── song_downloader.rs    # yt-dlp / native download
│       │   │   ├── song_thumbnail_downloader.rs
│       │   │   └── metallum.rs           # Metal Archives client (commented out, TODO)
│       │   ├── structures.rs # ListSong, DownloadStatus, BrowserSongsList
│       │   └── scrobbler.rs # ScrobbleState + submit_scrobble
│       └── youtube_downloader/ # yt-dlp command builder
├── ytmapi-rs/             # YouTube Music API wrapper
├── async-callback-manager/ # Task/effect framework
└── json-crawler/          # JSON traversal utilities
```

## Core Data Types

### ListSong (`structures.rs:75`)

```rust
pub struct ListSong {
    pub video_id: VideoID<'static>,
    pub track_no: Option<usize>,          // Some(1-9) for album track entries
    pub title: String,
    pub artists: MaybeRc<Vec<ListSongArtist>>,
    pub album: Option<MaybeRc<ListSongAlbum>>,
    pub duration_string: String,           // "M:SS" display format
    pub actual_duration: Option<Duration>, // Per-track duration for gapless/scrobble
    pub start_offset: Option<Duration>,    // Position in full album for seeking
    pub year: Option<Rc<String>>,
    pub download_status: DownloadStatus,
    pub album_art: AlbumArtState,
    pub id: ListSongID,
    // ... plays, explicit, thumbnails
}
```

### Important Modules

| File | Lines | Purpose |
|---|---|---|
| `messages.rs` | ~1420 | **Core file**. All BackendTask impls: ValidateMetadata (6-layer fallback), GetLyrics, DecodeSong, FetchAlbumArt. Also `fetch_album_tracks`, `norm_for_lfm`, ValidatedMetadata struct. |
| `playlist.rs` | ~2200 | Playback logic: `insert_album_tracks`, `handle_song_downloaded`, `play_song_id`, `handle_set_song_play_progress`, `handle_song_download_progress_update`. |
| `player.rs` | ~190 | `try_decode` — ffmpeg extraction at decode time, creates DecodedInMemSong. |
| `structures.rs` | ~610 | ListSong, BrowserSongsList with `push_song_list`, `insert_after`, `remove_at`. |
| `effect_handlers_playlist.rs` | ~390 | FrontendEffect impls for MetadataEffect, AlbumSectionsEffect, FetchAlbumArtEffect. |
| `scrobbler.rs` | 70 | ScrobbleState + submit_scrobble to Last.fm. |

## Key Patterns

### Tasks + Effects
Backend tasks are defined in `messages.rs` with `impl BackendTask<ArcServer>`. They run on the server tokio threadpool. Results come back as `FrontendEffect` variants, dispatched in `effect_handlers_playlist.rs`:

```rust
// Define task struct (messages.rs)
pub struct ValidateMetadata(pub String, pub String, pub ListSongID, pub String);
impl BackendTask<ArcServer> for ValidateMetadata { ... }

// Define handler struct (effect_handlers_playlist.rs)
pub struct HandleMetadataValidated(pub ListSongID);
impl_youtui_task_handler!(
    HandleMetadataValidated, ValidatedMetadata, Playlist,
    |this, metadata| MetadataEffect::Validated(metadata, this.0)
);

// Define effect + frontend handler
pub enum MetadataEffect {
    Validated(ValidatedMetadata, ListSongID),
    ValidationError,
}
impl FrontendEffect<Playlist, ...> for MetadataEffect {
    fn apply(self, target: &mut Playlist) -> ... { ... }
}

// Spawn task (playlist.rs)
let task = AsyncTask::new_future_try(
    ValidateMetadata(artist, title, id, api_key),
    HandleMetadataValidated(id),
    HandleMetadataValidationError,
    None,
);
```

### Metadata Validation Flow

```
add_yt_video
  → yt-dlp --dump-json → get artist + title + year
  → clean_title (strip "Artist - " + "FULL ALBUM" + parenthetical)
  → ValidateMetadata(artist, clean_title, id, api_key)

ValidateMetadata::into_future (messages.rs:730):
  1. Last.fm album.search(norm_for_lfm(title))
  2. → album.getInfo → tracklist with durations
  3. (falls through only if album search fails)
  4. Last.fm track.getInfo(artist, title) → exact track match
  5. Last.fm track.search() → fuzzy track → re-fetch for album context
  6. Discogs API search (no auth)
  7. MusicBrainz recording search (1 req/s)

fetch_album_tracks (messages.rs:926):
  1. Last.fm album.getInfo (requires API key)
  2. Discogs API search + master tracklist (no auth)
  3. Last.fm album.search → re-fetch album.getInfo

MetadataEffect::Validated handler:
  → Updates song.album, song.year, song.artists on original entry
  → If album_tracks found: calls insert_album_tracks
  → Spawns per-track ValidateMetadata
  → Spawns FetchAlbumArt(artist, album, api_key)
```

### Album Splitting Code Flow

```
add_yt_video → ValidateMetadata → MetadataEffect::Validated
  → insert_album_tracks (playlist.rs:575)
    → Creates N track entries with:
      track_no: Some(1..N)
      start_offset: accumulated seconds
      actual_duration: per-track duration
      year: validated year OR original's yt-dlp year
    → If original already downloaded: share Arc + remove original + play track 1

  → Download completes → handle_song_downloaded (playlist.rs:893)
    → If album_tracks set: share Arc with tracks
    → If all tracks ready: play track 1 + remove original

  → User plays track N → play_song_id
    → DecodeSong(pointer, offset, actual_duration)
    → try_decode (player.rs:112)
      → If offset + actual_duration: ffmpeg -ss offset -t duration
      → Decodes extracted section → DecodedInMemSong
    → Progress: handle_set_song_play_progress
      → If track entry: use d directly (ffmpeg already extracted)
      → Cap at actual_duration
      → If near end: QueueDecodedSong next track
```

## Adding New Features

### Add a backend task
1. Define struct in `messages.rs` with `impl BackendTask<ArcServer>`
2. Define handler + effect in `effect_handlers_playlist.rs`
3. Wire `impl_youtui_task_handler!` macro
4. Spawn from playlist via `AsyncTask::new_future_try(...)`

### Add a keybind
1. Add variant to `AppAction`/`PlaylistAction` in `keymap.rs`
2. Add key mapping in `default_playlist_keybinds()`
3. Implement `apply_action()` match arm in `playlist.rs`

### Add a test
Tests live in `app/ui/playlist/tests.rs`. Use `Playlist::new()` to create an empty list, `push_song_list()` to add songs, then call the handler directly and assert state.

## Testing

```bash
cargo test --release -p youtui --bin youtui               # All tests
cargo test --release -p youtui --bin youtui -- playlist::tests::  # Playlist only
cargo clippy                                               # Lint
```

95 tests pass, 2 pre-existing config failures (deserialization of the default keymap into the IR format).
