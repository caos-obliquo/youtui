# Leftovers — Plan

## P1: Keybind Standardization Bugs

### K1: `y` conflict in browser_artist_songs + browser_songs

**Problem**: Standalone `y` = `ViewLyrics`, but `y` in `o` context menu = `CopySongUrl`. Same key does different things depending on whether you're in a mode.

**Fix**: Change standalone `y` → `CopySongUrl`, add standalone `l` → `ViewLyrics` in:
- `keymap.rs:1207-1211` (browser_artist_songs)
- `keymap.rs:1439-1441` (browser_songs)

**Lines**: ~8

### K2: Missing standalone `l` for ViewLyrics in browser contexts

**Problem**: `browser_songs` and `browser_artist_songs` have `l` in `o` mode but no standalone `l` global keybind.

**Fix**: Add standalone `l` = `ViewLyrics` to `default_browser_songs_keybinds` and `default_browser_artist_songs_keybinds`.

**Lines**: ~8

### K3: Verify library `o` mode Space/P work

**Problem**: Added `Space` and `P` to library `o` mode, but need to check that the `LibraryBrowser` `ActionHandler<BrowserSongsAction>` handles `AddSongToPlaylist` and `AddSongsToPlaylist` for PLAYISTS/ARTISTS/ALBUMS categories (not just LikedSongs).

**Check**: `library.rs` — LikedSongs handler has `AddSongToPlaylist`/`AddSongsToPlaylist` (lines 608-618). But Playlist/Artist/Album categories don't. They fall through to `_ => warn!`. Should add handlers for these categories or at least log debug message.

## P2: Remaining Features

### F1: Highlight/yank in lyrics/annotations

**Vim-style visual mode in lyrics popup**:
- `V` toggles visual mode on focused pane
- `j`/`k` extend selection
- `y` copies selected lines to wl-clipboard
- Selection highlighted with different color

**Files**: `lyrics_popup.rs` (+~40 lines)

### F2: Visual mode highlight in playlist

**Problem**: `get_highlighted_row()` not wired to draw_table_impl. Selection range (`visual_start` to `cur_selected`) not highlighted.

**Fix**:
- `playlist.rs:531` — `get_highlighted_row()` return `visual_start` when `visual_mode` is active
- `view/draw.rs` — wire `secondary_highlight_row`
- Add `y` to yank visual selection to clipboard

**Lines**: ~20

### F3: Universal `/` search in ALL views

**Currently only**: Playlist + Library

**Missing from**: Artist songs panel, Playlist songs panel, Song search results

**Implementation**: Add `InputRouting::Search` + `TextHandler` to `AlbumSongsPanel` and `PlaylistSongsPanel`. When in search mode, local-filter the displayed song list by title/artist.

**Lines**: ~60

### F4: Library Artist/Albums Enter context actions

**Problem**: Currently only `PlaySong` works for Artists (→ artist page) and Albums (→ search). Should also handle `PlaySongs`, `AddSongToPlaylist`, etc.

**Fix**: Add match arms for `PlaySongs`, `AddSongToPlaylist`, `ViewLyrics`, `CopySongUrl` in the `Artists` and `Albums` category handlers.

**Lines**: ~20

## P3: Investigate

### I1: Lyrics malformed (B3)

**Check**: Add raw Musixmatch response logging in `messages.rs`. Test known songs. Check if Genius HTML structure changed.

### I2: Annotations display (B2)

**Check**: User says "(a: 10)" but only 3 visible. The scrolling logic at `lyrics_popup.rs:240-244` is correct. If explanations are long, only ~3 annotations fit on screen. j/k scrolls 1 line at a time. Fix may be: scroll by annotation block instead of by line.

---

## Execution Order

| Order | Item | Est. |
|-------|------|------|
| 1 | K1+K2: Fix `y`/`l` keybind consistency | 10min |
| 2 | K3: Verify library context menu handlers | 5min |
| 3 | F4: Library Artist/Album context menu actions | 10min |
| 4 | Build + test + commit | 5min |
| 5 | F1: Highlight/yank in lyrics | 30min |
| 6 | F2: Visual mode highlight in playlist | 20min |
| 7 | F3: Universal `/` search in all views | 30min |
| 8 | I1+I2: Investigate lyrics + annotations | 1-2h |
| | **Total** | **~3.5h** |

## Strategic Note

Items 1-4 are ~30min quick wins that finish the keybind cohesion work.
Items 5-7 are ~1.5h of new features.
Item 8 is open-ended investigation.

**Recommendation**: Knock out 1-4 now, then tackle 5-7 in order of impact. Defer 8 to when users report specific examples.
