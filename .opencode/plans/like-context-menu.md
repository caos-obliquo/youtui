# Like Button + Context Menu Cleanup Plan

## 1. Add `l` = ToggleLike to all context menus

### Playlist `o` mode (keymap.rs ~913)
Add after `y`=CopySongUrl:
```rust
(
    Keybind::new_unmodified(KeyCode::Char('l')),
    KeyActionTree::new_key(AppAction::Playlist(PlaylistAction::ToggleLike)),
),
```

### Library `o` mode (keymap.rs ~1097)
Same entry, with `AppAction::BrowserSongs(BrowserSongsAction::...)` — but there's no `ToggleLike` in `BrowserSongsAction`. Need to add it, OR use `PlaylistAction` since the playlist owns like status.

Actually, the like status is on `ListSong` which is shared across all contexts. The `ToggleLike` action needs to mutate the current song. In the library context, the song lives in `BrowserSongsList`. But the like/unlike API call goes through the playlist.

**Simplest approach**: Add `ToggleLike` as a `PlaylistAction` only (since the playlist has the current playing song and the `api.rate_song()` trigger). For non-playlist contexts, dispatching `PlaylistAction::ToggleLike` would need a different routing.

Actually, looking at how the current code works: `PlaylistAction::ToggleLike` handles the current song from `self.cur_selected`. But when you're in the browser library, there's no direct access to the playlist's `cur_selected`.

**Better approach**: Each context that has songs should have its own like handler. But this is complex. Instead, for now, just add `l` = ToggleLike to the playlist `o` context menu. The browser contexts will get it later when we have a proper cross-context like system.

Wait — the user said "Like should have a dedicated button on context menu too" — they want it accessible from:

**Plan**: Add `ToggleLike` as `PlaylistAction` and add `l` keybind to:
- Playlist `o` context menu ✅
- Playlist standalone `l` keybind ✅ (already planned)

For other contexts (library, songs, artist-songs, playlist-songs), add `l` = ToggleLike by dispatching through `AppCallback`:
```rust
AppCallback::ToggleLike(VideoID<'static>)
```
This way any context can trigger a like/unlike without having direct access to the playlist.

But this adds complexity. For NOW, let's just add to the playlist. Other contexts get `l` later.

## 2. Fix leftover `l` = ViewLyrics in browser_artist_songs `o` mode

Line 1262: change `Char('l')` → `Char('r')` and `ViewLyrics`.

## 3. Add standalone `l` = ToggleLike in playlist global keybinds

Add after existing keybinds in `default_playlist_keybinds`:
```rust
(
    Keybind::new_unmodified(KeyCode::Char('l')),
    KeyActionTree::new_key(AppAction::Playlist(PlaylistAction::ToggleLike)),
),
```

## Files Changed

| File | Lines | Change |
|------|-------|--------|
| `keymap.rs:913` (playlist `o` mode) | 5 | Add `l`→ToggleLike |
| `keymap.rs:1262` (artist_songs `o` mode) | 2 | Fix `l`→`r` for ViewLyrics |
| `keymap.rs` (playlist global) | 5 | Add standalone `l`→ToggleLike |

Total: **~12 lines, ~5min**

`L` = LoadFromYTM is already only in the playlist context (lines 900, 942). No changes needed.
