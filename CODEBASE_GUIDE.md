# Youtui Codebase Overview

Developer guide to understanding and extending youtui.

## Project Structure

```
youtui/                          # Main TUI application
├── youtui/src/
│   ├── main.rs                 # Entry point, cookie loading, CLI args
│   ├── app.rs                  # Application loop, task routing
│   ├── config/                 # Config deserialization, keymap
│   │   ├── mod.rs              # Config struct + loading
│   │   └── keymap.rs           # Keybind defaults + parsing (~2142 lines)
│   └── app/
│       ├── ui.rs               # Main window, event routing (~1741 lines)
│       ├── ui/playlist.rs      # Playback queue (~3104 lines)
│       ├── ui/playlist/
│       │   ├── effect_handlers.rs         # Effect re-exports
│       │   ├── effect_handlers_playlist.rs # Metadata/lyrics/album effects (~1302 lines)
│       │   ├── lyrics_popup.rs            # Lyrics + annotations (~1210 lines)
│       │   ├── tests.rs                   # 17 playlist unit tests
│       │   ├── song_info_popup.rs
│       │   ├── album_art_popup.rs
│       │   ├── notes_popup.rs
│       │   ├── config_editor_popup.rs
│       │   └── playlist_editor_popup.rs
│       ├── ui/browser/        # 5 browser tabs
│       │   ├── mod.rs         # Browser routing, tab dispatch (~1012 lines)
│       │   ├── draw.rs        # Browser rendering (~517 lines)
│       │   ├── songsearch.rs  # Songs tab
│       │   ├── albumsearch.rs # Albums tab (~731 lines)
│       │   ├── artistsearch.rs
│       │   ├── playlistsearch.rs
│       │   └── library.rs     # Library tab (~2005 lines)
│       ├── server/            # Backend tasks
│       │   ├── mod.rs
│       │   ├── messages.rs    # ALL BackendTask impls (~1598 lines)
│       │   ├── api.rs         # HTTP client + token refresh
│       │   ├── player.rs      # Audio decode + ffmpeg extraction (~365 lines)
│       │   ├── song_downloader.rs
│       │   └── song_thumbnail_downloader.rs
│       ├── structures.rs      # ListSong, DownloadStatus, BrowserSongsList (~872 lines)
│       └── scrobbler.rs       # ScrobbleState + submit_scrobble (~69 lines)
├── ytmapi-rs/                 # YouTube Music API client (~12.9k LOC)
├── async-callback-manager/    # Task/effect framework (~1.8k LOC)
├── json-crawler/              # JSON traversal utilities (~1.1k LOC)
├── libs/
│   ├── audio-player/          # rodio-based audio playback (~797 LOC, 0 tests)
│   ├── vi-text-editor/        # Vim-mode ratatui widget (~2.5k LOC, 65 tests)
│   ├── genius-rs/             # Genius API client + CLI (~1.3k LOC, 18 tests)
│   ├── lrclib-rs/             # LRCLIB.net lyrics API (~477 LOC, 4 tests)
│   ├── metadata-provider/     # 6 metadata providers (~2.1k LOC, 47 tests)
│   ├── rym-genre-data/        # Pre-scraped RYM genre data (~367 LOC, 10 tests)
│   └── ytmapi-cli/            # CLI debug tool for ytmapi-rs (~1.7k LOC, 7 tests)
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
| `youtui/src/app/server/messages.rs` | ~1598 | **Core file**. All BackendTask impls: ValidateMetadata (6-provider fallback), GetLyrics, DecodeSong, FetchAlbumArt. Also `fetch_album_tracks`, `norm_for_lfm`, ValidatedMetadata struct. |
| `youtui/src/app/ui/playlist.rs` | ~3104 | Playback logic: `insert_album_tracks`, `handle_song_downloaded`, `play_song_id`, `handle_set_song_play_progress`, `handle_song_download_progress_update`. |
| `youtui/src/app/server/player.rs` | ~365 | `try_decode` - ffmpeg extraction at decode time, creates DecodedInMemSong. |
| `youtui/src/app/structures.rs` | ~872 | ListSong, BrowserSongsList with `push_song_list`, `insert_after`, `remove_at`. |
| `youtui/src/app/ui/playlist/effect_handlers_playlist.rs` | ~1302 | FrontendEffect impls for MetadataEffect, AlbumSectionsEffect, FetchAlbumArtEffect. |
| `youtui/src/app/scrobbler.rs` | 69 | ScrobbleState + submit_scrobble to Last.fm. |

## Key Patterns

### Tasks + Effects
Backend tasks are defined in `youtui/src/app/server/messages.rs` with `impl BackendTask<ArcServer>`. They run on the server tokio threadpool. Results come back as `FrontendEffect` variants, dispatched in `youtui/src/app/ui/playlist/effect_handlers_playlist.rs`:

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
  -> yt-dlp --dump-json -> get artist + title + year
  -> clean_title (strip "Artist - " + "FULL ALBUM" + parenthetical)
  -> ValidateMetadata(artist, clean_title, id, api_key)

ValidateMetadata::into_future (messages.rs:730):
  1. Metal Archives (MA_COOKIE env)
  2. Last.fm album.search(norm_for_lfm(title))
  3. -> album.getInfo -> tracklist with durations
  4. (falls through only if album search fails)
  5. YTM album enrichment (post-registry fallback)
  6. Last.fm track.getInfo(artist, title) -> exact track match
  7. Discogs API search (no auth)
  8. MusicBrainz recording search (1 req/s)
  9. Genius (lowest priority)

fetch_album_tracks (messages.rs:926):
  1. Metal Archives (MA_COOKIE)
  2. Last.fm album.getInfo (requires API key)
  3. Discogs API search + master tracklist
  4. Last.fm album.search -> re-fetch album.getInfo

MetadataEffect::Validated handler:
  -> Updates song.album, song.year, song.artists on original entry
  -> If album_tracks found: calls insert_album_tracks
  -> Spawns per-track ValidateMetadata
  -> Spawns FetchAlbumArt(artist, album, api_key)
```

### Album Splitting Code Flow

```
add_yt_video -> ValidateMetadata -> MetadataEffect::Validated
  -> insert_album_tracks (playlist.rs:575)
    -> Creates N track entries with:
      track_no: Some(1..N)
      start_offset: accumulated seconds
      actual_duration: per-track duration
      year: validated year OR original's yt-dlp year
    -> If original already downloaded: share Arc + remove original + play track 1

  -> Download completes -> handle_song_downloaded (playlist.rs:893)
    -> If album_tracks set: share Arc with tracks
    -> If all tracks ready: play track 1 + remove original

  -> User plays track N -> play_song_id
    -> DecodeSong(pointer, offset, actual_duration)
    -> try_decode (player.rs:112)
      -> If offset + actual_duration: ffmpeg -ss offset -t duration
      -> Decodes extracted section -> DecodedInMemSong
    -> Progress: handle_set_song_play_progress
      -> If track entry: use d directly (ffmpeg already extracted)
      -> Cap at actual_duration
      -> If near end: QueueDecodedSong next track
```

## Adding New Features

### Add a backend task
1. Define struct in `youtui/src/app/server/messages.rs` with `impl BackendTask<ArcServer>`
2. Define handler + effect in `youtui/src/app/ui/playlist/effect_handlers_playlist.rs`
3. Wire `impl_youtui_task_handler!` macro
4. Spawn from playlist via `AsyncTask::new_future_try(...)`

### Add a keybind
1. Add variant to `AppAction`/`PlaylistAction` in `youtui/src/config/keymap.rs`
2. Add key mapping in `default_playlist_keybinds()`
3. Implement `apply_action()` match arm in `youtui/src/app/ui/playlist.rs`

### Add a test
Tests live in `youtui/src/app/ui/playlist/tests.rs`. Use `Playlist::new()` to create an empty list, `push_song_list()` to add songs, then call the handler directly and assert state.

## Testing

```bash
cargo test --release -p youtui --bin youtui               # 136 tests
cargo test --release -p youtui --bin youtui -- playlist::tests::  # Playlist only
cargo clippy                                               # Lint
```

136 tests pass, 4 ignored, 0 warnings. Full workspace: 388+ tests across 11 crates.
