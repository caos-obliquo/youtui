# Enship — Navigation Hub + Local Search + Go To Artist/Album

## Architecture

### 1. Navigation Hub (`app/navigation.rs` — NEW)

```rust
pub enum NavTarget {
    Artist(String),
    Album { artist: String, album: String },
    SongSearch(String),
    Lyrics { artist: String, title: String },
}

pub struct SongNavInfo {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub video_id: VideoID<'static>,
}
```

### 2. App Callback (app.rs — 2 variants, ~20 lines)

```rust
pub enum AppCallback {
    // ... existing ...
    Navigate(NavTarget),
    Back,
}
```

Handler in `handle_callback`:
```rust
AppCallback::Navigate(target) => {
    self.window_state.context = WindowContext::Browser;
    if let Some(task) = self.window_state.browser.navigate_to(target) {
        self.task_manager.spawn_task(&self.server, task);
    }
}
AppCallback::Back => {
    self.window_state.browser.navigate_back();
}
```

### 3. Browser State Snapshot (browser.rs — ~50 lines)

Add to Browser struct:
```rust
state_stack: Vec<BrowserSnapshot>,
```

```rust
struct BrowserSnapshot {
    variant: BrowserVariant,
    library_state: LibrarySnapshot,
}

struct LibrarySnapshot { category: LibraryCategory, input_routing: InputRouting }

impl Browser {
    fn navigate_to(&mut self, target: NavTarget) -> Option<AsyncTask<...>> {
        self.push_snapshot();
        match target {
            NavTarget::Artist(name) => {
                self.variant = BrowserVariant::Artist;
                self.artist_search_browser.artist_search_panel
                    .search.replace_text(&name);
                Some(self.artist_search_browser.fetch_artists(name).map_frontend(...))
            }
            NavTarget::Album { artist, album } => {
                self.variant = BrowserVariant::Song;
                self.song_search_browser.search.replace_text(format!("{artist} {album}"));
                Some(self.song_search_browser.search_songs(format!("{artist} {album}")).map_frontend(...))
            }
            NavTarget::SongSearch(query) => { ... }
            NavTarget::Lyrics { artist, title } => {
                // open lyrics popup
            }
        }
    }
    
    fn push_snapshot(&mut self) { ... }
    fn navigate_back(&mut self) { ... }
}
```

### 4. Local Search in Library (library.rs — ~80 lines)

Add fields:
```rust
pub search_active: bool,
pub search_editor: ViTextEditor,
```

Change `InputRouting`:
```rust
pub enum InputRouting {
    Category,
    Content,
    Search,
}
```

Add methods:
- `handle_toggle_search()`
- `text_editor_mode() -> Option<String>`
- `get_filtered_playlists() -> Vec<&LibraryPlaylist>`
- `get_filtered_artists() -> Vec<&LibraryArtist>`
- `get_filtered_albums() -> Vec<&SearchResultAlbum>`
- `get_filtered_songs() -> Vec<&ListSong>`

Implement `TextHandler` (delegate to `search_editor` when in Search mode).

In draw: when `InputRouting::Search`, draw search box above content. Filter all category views.

Navigation: `/` key → `BrowserAction::Search` → `library_browser.handle_toggle_search()`

### 5. Go To Artist / Go To Album Actions

Add to these action enums:

```rust
// BrowserSongsAction (library + songsearch):
GoToArtist, GoToAlbum

// BrowserArtistSongsAction:
GoToArtist, GoToAlbum

// BrowserPlaylistSongsAction:
GoToArtist, GoToAlbum

// PlaylistAction:
GoToArtist, GoToAlbum
```

Each handler extracts artist/album from current song and dispatches:
```rust
AppCallback::Navigate(NavTarget::Artist(artist_name))
```

### 6. Keybinds (keymap.rs — ~30 lines)

```rust
// browser_library context:
"/" -> BrowserAction::Search  // toggle local search
"g a" -> BrowserSongsAction::GoToArtist
"g b" -> BrowserSongsAction::GoToAlbum

// playlist context:
"g a" -> PlaylistAction::GoToArtist
"g b" -> PlaylistAction::GoToAlbum

// browser_songs context (song search):
"g a" -> BrowserSongsAction::GoToArtist
"g b" -> BrowserSongsAction::GoToAlbum

// browser_artist_songs context:
"g a" -> BrowserArtistSongsAction::GoToArtist
"g b" -> BrowserArtistSongsAction::GoToAlbum

// browser_playlist_songs context:
"g a" -> BrowserPlaylistSongsAction::GoToArtist
"g b" -> BrowserPlaylistSongsAction::GoToAlbum
```

## Files Touched (ordered by dependency)

| # | File | Change | Lines |
|---|------|--------|-------|
| 1 | `app/navigation.rs` (NEW) | `NavTarget`, `SongNavInfo` | 15 |
| 2 | `app/ui/browser/library.rs` | `search_active`, `search_editor`, `InputRouting::Search`, `TextHandler`, filtered getters, `handle_toggle_search`, `text_editor_mode` | 80 |
| 3 | `app/ui/browser/draw.rs` | Search box rendering + filtered display | 25 |
| 4 | `app/ui/browser.rs` | `BrowserSnapshot`, `state_stack`, `navigate_to()`, `navigate_back()`, `text_editor_mode` + `TextHandler` + `handle_toggle_search` for LibraryPlaylist | 60 |
| 5 | `app/ui/action.rs` | No changes needed (BrowserAction::Search already exists) | 0 |
| 6 | `app.rs` | `AppCallback::Navigate`, `Back`, handler | 20 |
| 7 | `app/ui/playlist.rs` | `GoToArtist`, `GoToAlbum` → `NavTarget` | 20 |
| 8 | `app/ui/browser/songsearch.rs` | `GoToArtist`, `GoToAlbum` → `NavTarget` | 20 |
| 9 | `app/ui/browser/artistsearch/songs_panel.rs` | +2 actions + handler | 20 |
| 10 | `app/ui/browser/playlistsearch/songs_panel.rs` | +2 actions + handler | 20 |
| 11 | `config/keymap.rs` | Keybinds in 6 contexts | 30 |
| 12 | `app/mod.rs` (check) | Add `pub mod navigation` | 1 |
| | **Total** | | **~310** |

## Build & Test

```bash
cargo build --release -p youtui --bin youtui
cargo test --release -p youtui --bin youtui
git add -A && git commit -m "feat: navigation hub, local search, go to artist/album"
```
