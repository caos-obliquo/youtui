# Youtui Codebase Overview

A guide to understanding the youtui codebase for future development sessions.

## Project Structure

```
youtui/
├── youtui/                 # Main TUI application (workspace member)
├── ytmapi-rs/              # YouTube Music API wrapper (workspace member)
├── json-crawler/           # JSON traversal utilities (workspace member)
├── async-callback-manager/ # Task management framework (workspace member)
├── libs/ytmapi-cli/        # YTM API debug CLI (workspace member)
├── libs/metadata-cache-sqlite/ # SQLite metadata cache (workspace member)
├── libs/genre-db-sqlite/   # SQLite genre hierarchy (workspace member)
├── libs/metadata-provider/ # Metadata providers (crate dir, not a workspace member)
├── libs/audio-player/      # Rodio audio player (crate dir)
├── libs/genius-rs/         # Genius API client (crate dir)
├── libs/lrclib-rs/         # LRCLIB client (crate dir)
├── libs/rym-genre-data/    # RYM genre data (crate dir)
├── libs/vi-text-editor/    # Vim text editor (crate dir)
└── justfile               # Task runner (just test, just clippy, etc.)
```

7 workspace members, 13 crate directories total.

## Key Concepts

### Architecture Pattern

Youtui uses an **async callback architecture** with three main layers:

1. **Frontend (UI State)** - Ratatui-based TUI components
2. **Backend (Server)** - YouTube Music API interactions
3. **Task Manager** - Async callback manager connecting frontend to backend

### Core Files

| File | Purpose |
|------|---------|
| `youtui/src/app.rs` | Application entry point, main loop |
| `youtui/src/app/ui/` | All TUI components (playlist, browser, etc.) |
| `youtui/src/app/server/` | API calls, song downloading, playback |
| `youtui/src/config/` | Configuration, keybinds |
| `ytmapi-rs/src/` | YouTube Music API parsing |

### Key Data Structures

#### ListSong
Song representation used throughout the app. Stored in `BrowserSongsList`:
- `video_id`: YouTube video identifier
- `title`, `artists`, `album`: Metadata
- `download_status`: Current download state
- `album_art`: Thumbnail state

#### Playlist
Main playback queue at `youtui/src/app/ui/playlist.rs`:
- Manages song list, playback state, volume
- Handles shuffle, search within playlist
- Key entry point for playback control

### Text Input System

Text inputs use `rat_text::text_input::TextInputState` via the `TextHandler` trait:

1. **SearchBlock** - Search queries with suggestions
2. **FilterManager** - Column filtering in tables

Key methods:
- `handle_text_event_impl()` - Process crossterm events
- `delete_word()` - Delete previous word (Ctrl+W)
- `fetch_search_suggestions()` - API call for suggestions

### Keybinding System

Located in `youtui/src/config/keymap.rs`:

- Keybinds are configured per-context (global, playlist, browser, text_entry, etc.)
- Actions are defined in enums (`AppAction`, `TextEntryAction`, etc.)
- `KeyActionTree` allows nesting modes (e.g., Enter opens a mode)

### Event Flow

```
1. KeyEvent → crossterm
2. handle_key_stack() → matches to Action
3. apply_action() → generates Effect
4. Effect → TaskManager → Backend
5. Backend response → Frontend mutation
6. UI redraws
```

### Persistence

Queue saving in `youtui/src/app/queue_persistence.rs`:
- Saves only `video_id` for minimal file size
- On load, creates placeholder `ListSong::create_placeholder()`
- Metadata refreshes when songs are fetched via API

## Adding New Features

### Adding a Keybind

1. Define action in appropriate enum (e.g., `TextEntryAction`)
2. Add keybind in `default_*_keybinds()` functions
3. Implement `apply_action()` for the component

### Adding Text Input Handling

1. Add method to `TextHandler` trait if needed
2. Implement in `SearchBlock` or `FilterManager`
3. Route via `handle_text_entry_action()` in browser components

### Saving New Data

1. Add struct with `#[derive(Serialize, Deserialize)]`
2. Use `serde_skip` for non-serializable fields
3. Follow existing patterns in `queue_persistence.rs`

## Testing

```bash
just test          # Run all tests
cargo build       # Build debug
cargo build --release  # Build release
cargo clippy      # Lint checks
```

Note: Some test infrastructure issues exist in the current codebase.

## Useful Commands

```bash
# Run the app
cargo run --release

# Check formatting
cargo fmt -- --check

# Generate docs
cargo doc --package ytmapi-rs --open
```

## Dependencies

Key external dependencies:
- `rat_text`: Text input widget
- `ratatui`: Terminal UI framework
- `ytmapi_rs`: YouTube Music API
- `rodio`: Audio playback

Audio download path is external binaries: `yt-dlp` (streams) plus `ffmpeg`
(track splitting). `youtui-vendored-rusty_ytdl` remains in youtui/Cargo.toml
as the legacy native-download fallback (`youtube_downloader/native.rs`).