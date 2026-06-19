# Final Enship Plan

## Progress (today)
| Item | Status |
|------|--------|
| NavTarget enum + AppCallback | ✅ Done |
| Browser::navigate_to + navigate_back + snapshots | ✅ Done |
| Library InputRouting::Search + TextHandler | ✅ Done |
| Browser TextHandler/BrowserSearchAction for LibraryPlaylist | ✅ Done |
| DominantKeyRouter for Library Search mode | ✅ Done |
| GoToArtist/GoToAlbum in BrowserSongsAction | ✅ Done |
| BrowserAction::Back | ✅ Done |
| B1: `[`/`]` in lyrics (AppCallback + handler + lyrics_popup) | ✅ Done |
| F4: Library search box draw (draw.rs) | ⚡ Partial — need `right_chunk`→`content_chunk` replace |
| GoToArtist/GoToAlbum in BrowserArtistSongsAction | ⚡ Enum + describe done, handler methods needed |
| F5: Rest of GoToArtist/GoToAlbum contexts | ❌ Not started |
| F1: Keybinds | ❌ Not started |
| F2+F3: Library Artist/Album Enter | ❌ Not started |
| B4: Album art quality | ❌ Not started |
| B3: Lyrics malformed investigation | ❌ Not started |

---

## P0: Must Ship

### P0a: `right_chunk`→`content_chunk` (F4 completion)

**What**: `draw.rs:draw_library_browser` — all 4 category match arms reference `right_chunk` but need to reference `content_chunk` (added for search box split).

**Fix**: Replace 8 occurrences in liked_songs/playlists/artists/albums blocks:
- `block.inner(right_chunk)` → `block.inner(content_chunk)`
- `f.render_widget(block, right_chunk)` → `f.render_widget(block, content_chunk)`

**File**: `draw.rs:322-444`  
**Lines**: 4 (replaceAll)

---

### P0b: GoToArtist/GoToAlbum in PlaylistAction + handler

**What**: Add `GoToArtist`, `GoToAlbum` to `PlaylistAction` enum + handler.

**Enum** (`playlist.rs:120`):
```rust
GoToArtist,
GoToAlbum,
```

**describe** (`playlist.rs:157`):
```rust
PlaylistAction::GoToArtist => "Go to Artist",
PlaylistAction::GoToAlbum => "Go to Album",
```

**handler** — in the playlist's action handler for `PlaylistAction`, add:
```rust
PlaylistAction::GoToArtist => return self.go_to_artist().into(),
PlaylistAction::GoToAlbum => return self.go_to_album().into(),
```

**methods** (`playlist.rs`):
```rust
pub fn go_to_artist(&mut self) -> impl Into<YoutuiEffect<Self>> + use<> {
    let Some(song) = self.list.get_song(self.cur_selected) else {
        return (AsyncTask::new_no_op(), None);
    };
    let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
    (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::Artist(artist))))
}
pub fn go_to_album(&mut self) -> impl Into<YoutuiEffect<Self>> + use<> {
    // same pattern, check song.album.is_some()
}
```

**File**: `playlist.rs`  
**Lines**: ~25

---

### P0c: GoToArtist/GoToAlbum in BrowserArtistSongsAction + handler

**What**: Same as P0b but for `artistsearch/songs_panel.rs`.

Enum + describe ✅ already done. Need handler methods + action dispatch.

**File**: `artistsearch/songs_panel.rs`  
**Lines**: ~25

---

### P0d: GoToArtist/GoToAlbum in BrowserPlaylistSongsAction + handler

**What**: Same pattern for `playlistsearch/songs_panel.rs`.

**File**: `playlistsearch/songs_panel.rs`  
**Lines**: ~25

---

### P0e: Keybinds (F1)

**File**: `config/keymap.rs`

| Context | Key | Action |
|---------|-----|--------|
| `browser_library` | `/` | `BrowserAction::Search` |
| `browser_library` | `g a` | `BrowserSongsAction::GoToArtist` |
| `browser_library` | `g b` | `BrowserSongsAction::GoToAlbum` |
| `browser_songs` | `g a` | `BrowserSongsAction::GoToArtist` |
| `browser_songs` | `g b` | `BrowserSongsAction::GoToAlbum` |
| `browser` (global) | `Backspace` | `BrowserAction::Back` |
| `playlist` | `g a` | `PlaylistAction::GoToArtist` |
| `playlist` | `g b` | `PlaylistAction::GoToAlbum` |
| `browser_artist_songs` | `g a` | `BrowserArtistSongsAction::GoToArtist` |
| `browser_artist_songs` | `g b` | `BrowserArtistSongsAction::GoToAlbum` |
| `browser_playlist_songs` | `g a` | `BrowserPlaylistSongsAction::GoToArtist` |
| `browser_playlist_songs` | `g b` | `BrowserPlaylistSongsAction::GoToAlbum` |

**Lines**: ~50

---

### P0f: Library Artist/Album Enter (F2+F3)

**File**: `library.rs` ActionHandler

For `LibraryCategory::Artists` + `BrowserSongsAction::PlaySong`:
- Dispatch `AppCallback::Navigate(NavTarget::Artist(artist_name))`

For `LibraryCategory::Albums` + `BrowserSongsAction::PlaySong`:
- Dispatch `AppCallback::Navigate(NavTarget::SongSearch("{artist} {album}"))`

**Lines**: ~20

---

### P0g: Album Art Quality (B4)

**Step 1**: In `draw_media_controls.rs:45`, upgrade YTM thumbnail URL:
```rust
fn upgrade_yt_thumb(url: &str) -> String {
    // Replace =w60-h60 or =s60 with =w600-h600
    let re = regex::Regex::new(r"=w\d+-h\d+|=\w+s\d+").unwrap();
    re.replace(url, "=w600-h600").to_string()
}
```
Apply when falling back to `s.thumbnails`.

**Step 2**: Spawn `FetchAlbumArt` for all songs that have album metadata after they are added to playlist. In `effect_handlers_playlist.rs`, after any song is created with `album.is_some()`, spawn:
```rust
AsyncTask::new_future_try(
    FetchAlbumArt(artist, album_name, api_key),
    HandleFetchAlbumArtOk(...),
    HandleFetchAlbumArtErr,
    None,
)
```

**Files**: `draw_media_controls.rs`, `effect_handlers_playlist.rs` or `playlist.rs` (play-song hook)  
**Lines**: ~25

---

## P1: Future TODO — Highlight/Yank in Lyrics + Vi Fixes

### V1: Vim Visual Mode in Lyrics/Annotations

**What**: Allow selecting text in lyrics/annotations panel with `V` (visual line) and copying with `y`.

**Current state**: `LyricsPopup` handles keys directly in `handle_key()` — no text selection, no clipboard.

**Implementation**:
1. Add `visual_mode: bool`, `visual_start: usize`, `visual_end: usize` to `LyricsPopup`
2. `V` toggles visual mode on the focused pane (lyrics or annotations)
3. `j`/`k` extend selection when visual mode active
4. `y` copies selected lines to clipboard via `wl-copy` or similar
5. Visual selection is highlighted with different color/style
6. `Esc` exits visual mode

**Fields** (lyrics_popup.rs:48):
```rust
pub visual_mode: bool,
pub visual_start: usize,
pub visual_end: usize,
```

**Key handling** (lyrics_popup.rs:103):
```rust
KeyCode::Char('V') => {
    if !self.visual_mode {
        self.visual_mode = true;
        self.visual_start = self.scroll_offset;
        self.visual_end = self.scroll_offset;
    }
}
KeyCode::Char('y') => {
    if self.visual_mode {
        // copy selected lines to clipboard
        let lines = ...; // extract lines from visual_start..visual_end
        let _ = std::process::Command::new("wl-copy").arg(&lines).spawn();
        self.visual_mode = false;
    }
}
KeyCode::Esc => {
    self.visual_mode = false;
}
```

**Draw** — when `visual_mode`, render selected lines with different style (e.g., `Style::default().bg(ROW_HIGHLIGHT_COLOUR)`).

**File**: `lyrics_popup.rs`  
**Lines**: ~40

---

### V2: Visual Mode Bug Fixes in Playlist

**Known bugs** (from user report):
1. `V` visual mode selection highlight doesn't show visually (`get_highlighted_row()` not wired)
2. Selection range (`visual_start` to `cur_selected`) not drawn in table
3. `y` to yank/copy not implemented for visual selection

**Fix**:
1. `playlist.rs:531` — `get_highlighted_row()` must return `visual_start` when `visual_mode` is active
2. `view/draw.rs` — `draw_table_impl` supports `secondary_highlight_row` but it's unused. Wire it.
3. Add `y` to copy selected lines to clipboard in visual mode

**Lines**: ~20

---

### V3: Lyrics Malformed Investigation (B3)

1. Add raw API response logging to `GetLyrics` pipeline in `messages.rs`
2. Check Musixmatch response format
3. Check Genius HTML structure
4. Add response snippet to `tracing::info!()`

**Est**: 1-2h

---

## Execution Order

```
Phase 1: Draw fix     [F4 complete]     → 5 min
Phase 2: GoTo*        [P0b+P0c+P0d]     → 30 min
Phase 3: Keybinds     [P0e]             → 20 min
Phase 4: Lib Enter    [P0f]             → 15 min
Phase 5: Album art    [P0g]             → 30 min
Phase 6: Build+test+commit              → 10 min
==============================================
Total:                                   ~1h 50min
```

## Future TODO
- V1: Highlight/yank in lyrics/annotations
- V2: Visual mode bugs in playlist (highlight + yank)
- V3: Lyrics malformed investigation
