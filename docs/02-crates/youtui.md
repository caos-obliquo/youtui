# Crate: youtui

**~35k LOC, 71 files** - main application crate.

## Module Tree

```
youtui/src/
├── main.rs                 - Entry point, CLI parsing, Youtui::run()
├── app.rs                  - Youtui struct, PlayDebouncer, AppCallback enum, event loop
├── appevent.rs             - EventHandler, Crossterm event stream
├── core.rs                 - Temp file management, file cleanup
├── drawutils.rs            - Rect math, alignment helpers
├── keyaction.rs            - KeyActionTree, DisplayableKeyAction
├── keybind.rs              - Keybind parsing (from config string)
├── tests.rs                - Integration tests
├── widgets.rs              - Widget re-exports
│
├── cli/                    - CLI query builder
│   └── querybuilder.rs
│
├── api/
│   ├── mod.rs              - YTM API wrapper (refresh token, search)
│   └── error.rs
│
├── config/
│   ├── mod.rs              - Config, ConfigIR, Config::new()
│   └── keymap.rs           - Keymap struct, default keybinds, parsing
│
├── widgets/
│   ├── mod.rs
│   ├── scrolling_list.rs   - Scrollable list state widget
│   ├── scrolling_table.rs  - Scrollable table with column widths
│   └── tab_grid.rs         - Tab-based grid layout
│
├── youtube_downloader/
│   ├── mod.rs
│   ├── yt_dlp.rs           - yt-dlp command wrapper
│   └── native.rs           - rusty_ytdl (unused, broken)
│
└── app/
    ├── mod.rs              - App re-exports
    ├── structures.rs       - ListSong, ListSongArtist, AlbumArtState
    ├── queue_persistence.rs- Save/load queue to disk
    ├── media_controls.rs   - MPRIS media controls (souvlaki)
    ├── scrobbler.rs        - Libre.fm/Last.fm scrobbling
    │
    ├── component/
    │   ├── mod.rs
    │   └── actionhandler.rs- ActionHandler, KeyRouter, YoutuiEffect
    │
    ├── view/
    │   ├── mod.rs          - View trait, TableView, Filter, Sort types
    │   └── draw.rs         - draw_panel, draw_table_impl
    │
    ├── server/
    │   ├── mod.rs          - Server struct
    │   ├── messages.rs     - ALL BackendTask impls (~1598 lines)
    │   ├── api.rs          - HTTP client + token refresh
    │   ├── api_error_handler.rs
    │   ├── player.rs       - Audio decode + ffmpeg extraction
    │   ├── song_downloader.rs - Download semaphore + validation
    │   └── song_thumbnail_downloader.rs - Album art fetch
    │
    └── ui/
├── mod.rs              - YoutuiWindow (cached_album_protocol, invalidate_protocol_cache), HelpMenu, context routing
        ├── action.rs       - AppAction enum, ALL action variants
        ├── draw.rs         - Main draw function: popups, help, footer
        ├── draw_media_controls.rs - Media progress bar
        ├── header.rs       - Top bar: mode, title, controls
        ├── footer.rs       - Bottom bar: keybinding hints, album art protocol cache
        ├── logger.rs       - Logs view (tui-logger)
        │
        ├── browser/
        │   ├── mod.rs      - Browser struct, tab dispatch
        │   ├── draw.rs     - Browser rendering
        │   ├── shared_components.rs - SearchBlock, SortManager, FilterManager
        │   ├── songsearch.rs - Song search tab
        │   ├── artistsearch.rs - Artist search tab
        │   ├── artistsearch/
        │   │   ├── search_panel.rs
        │   │   └── songs_panel.rs
        │   ├── playlistsearch.rs  - Playlist search tab + songs
        │   ├── playlistsearch/
        │   │   ├── search_panel.rs
        │   │   └── songs_panel.rs
        │   └── library.rs - Library browser (4th tab)
        │
        ├── components/
        │   └── mod.rs      - Component macros
        │
        └── playlist/
            ├── mod.rs      - Playlist struct (~3104 lines)
            ├── effect_handlers.rs - Effect handler re-exports
            ├── effect_handlers_playlist.rs - Playlist-specific effects
            ├── lyrics_popup.rs
            ├── song_info_popup.rs
            ├── album_art_popup.rs
            ├── config_editor_popup.rs
            ├── playlist_save_popup.rs
            ├── playlist_update_popup.rs
            └── tests.rs    - Playlist unit tests
```

## AppAction Enum

The central action type. All 40+ variants:

```
Quit, VolUp, VolDown, NextSong, PrevSong,
SeekForward, SeekBack, ToggleHelp, ViewLogs,
PlayPause, NoOp, ToggleBrowser, TogglePlaylist,
EditConfig, OpenUrl,
Browser(BrowserAction),       Filter(FilterAction),
Sort(SortAction),             Help(HelpAction),
BrowserArtists(...),          BrowserPlaylists(...),
BrowserSearch(...),           BrowserArtistSongs(...),
BrowserPlaylistSongs(...),    BrowserSongs(...),
BrowserLibrary(...),          Log(LoggerAction),
Playlist(PlaylistAction),     PlaylistSavePopup(...),
ConfigEditor(...),            LyricsPopup(...),
SongInfo(...),                TextEntry(...),
List(ListAction)
```

## AppCallback Enum

Callbacks from UI components to the main event loop:

```rust
Quit, ChangeContext(WindowContext),
AddSongsToPlaylist(Vec<ListSong>),
AddSongsToPlaylistAndPlay(Vec<ListSong>),
ViewLyrics { artist, title },
ViewSongInfo { song },
ViewAlbumCover { thumbnail },
UpdateSongInfo { id, song },
ClosePopup,
LoadPlaylistFromPopup, AppendPlaylistFromPopup,
CreatePlaylistFromPopup { title, description, video_ids },
Navigate(NavTarget),
SeekBack, SeekForward, SeekTo(Duration),
ReloadConfig,
Back
```
