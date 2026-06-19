# UX Polish — Context Menu Cohesion + Bugfixes

## Audit: Context Menu Cohesion Issues

### Issue 1: `o` and `Enter` modes are DUPLICATED identically in 3 contexts

| Context | `o` mode keys | `Enter` mode keys | Problem |
|---------|--------------|-------------------|---------|
| browser_songs | Space p Enter P y | Space p Enter P y | IDENTICAL |
| browser_artist_songs | Space p a Enter P A y | Space p a Enter P A y | IDENTICAL |
| browser_playlist_songs | Space p Enter P y | Space p Enter P y | IDENTICAL |

**Fix**: Remove one duplicate. `o` (context menu) is the standard. Remove the `Enter` mode entirely from browser_songs/artist_songs/playlist_songs. Instead, `Enter` should immediately PlaySong (no mode popup) like it does in library and playlist.

### Issue 2: Inconsistent `l` key usage

| Context | `l` as standalone | `l` in context menu |
|---------|------------------|-------------------|
| Playlist | `l` = no bind (conflicts with list navigation `l`/`h`) | `o` → `l` = ViewLyrics |
| browser_playlist_songs | `y` = ViewLyrics | no lyrics entry in `o` mode |
| browser_songs | `y` = ViewLyrics | no lyrics entry in `o` mode |
| browser_artist_songs | `y` = ViewLyrics | `o` → `y` = CopySongUrl |
| browser_library | `y` = CopySongUrl | `o` → `y` = CopySongUrl |

**Fix**: Standardize `y` = CopySongUrl everywhere (global). Standardize `l` = ViewLyrics everywhere (global + context menu). Add lyrics entry to `o` mode in song/playlist-songs panels.

### Issue 3: Playlist `o` mode has different subkeys from browser `o` mode

Playlist `o`: Enter(d), d(elete), l(yrics), y(URL), E(xisting playlist), L(oad YTM)
Browser `o`: Space(add), p(lay all), a(lbum), Enter(play), P(add all), A(dd album), y(URL)

These are different actions because they operate on different data. This is OK — they're different contexts with different needs. But the naming should be consistent where possible:
- `Space` should always = AddSongToPlaylist
- `p` should always = PlayAll
- `P` should always = AddAllToPlaylist
- `y` should always = CopySongUrl
- `l` should always = ViewLyrics
- `g a/b` should always = GoToArtist/Album

### Issue 4: Global keybinds that should be in context menus (and vice versa)

**Missing from playlist context menu** (`o`):
- `I` (ViewSongInfo) — should be in context menu
- `u` (UndoDelete) — only makes sense in visual mode context
- `z` (ToggleRepeat) — playback control, fine as global
- `Z` (ToggleRadio) — fine as global
- `d` (DeleteSelected) — already in context menu as `d`

**Missing from library context menu** (`o`):
- AddSongToPlaylist — not in context menu (needs `Space`)
- AddSongsToPlaylist — not in context menu (needs `P`)
- ViewLyrics — not in context menu

### Issue 5: `L` and `E` conflict in playlist

- Standalone `L` = LoadFromYTM (line 886)
- Standalone `E` = SaveToExistingPlaylist (line 952) 
- `o` → `L` = LoadFromYTM (line 924)
- `o` → `E` = SaveToExistingPlaylist (line 920)

These work but are inconsistent: `L` is both standalone AND in context menu. `E` is both standalone AND in context menu. The issue is that users may press lowercase `e` expecting it to work but it's `E` (shift+e).

**Fix**: Add lowercase `e` bind for SaveToExistingPlaylist. Or standardize on uppercase for "dangerous" actions.

---

## Save to Existing Playlist Bug

**Root cause**: `PlaylistAction::SaveToExistingPlaylist` collects ALL songs from the full playlist (line 235-237), not just the current song. When the playlist has many songs (e.g., 5000), the resulting `AddSongsToPlaylist` call takes a long time and may fail if there are duplicate video IDs.

**Already fixed**: Dedup + `Unhandled` mode added in previous session. Need to rebuild.

**UX improvement**: `SaveToExistingPlaylist` should only save the CURRENT song (like how `AddSongToPlaylist` works in browser contexts), not the entire queue. Add a separate "Save Queue to Playlist" action for the full-queue use case.

---

## Remaining Implementation Items

| # | Item | File | Lines | Status |
|---|------|------|-------|--------|
| R1 | `upgrade_thumbnail_url` fn | `draw_media_controls.rs` | ~5 | Not implemented |
| R2 | Remove duplicate `Enter` modes in browser_songs/artist_songs/playlist_songs | `keymap.rs` | ~40 | Not started |
| R3 | Add `l` (Lyrics) to context menus where missing | `keymap.rs` + action handlers | ~15 | Not started |
| R4 | Add `Space`/`P` (AddToPlaylist) + `l` (Lyrics) to library context menu | `keymap.rs` + `library.rs` | ~10 | Not started |
| R5 | Add `I` (SongInfo) + `l` (Lyrics) to playlist context menu | `keymap.rs` | ~5 | Not started |
| R6 | Fix SaveToExistingPlaylist — use current song only | `playlist.rs` | ~3 | Not started |
| R7 | Build + test + commit | - | - | Not started |

---

## Execution Plan

### Phase 1: Add missing `upgrade_thumbnail_url` (R1)
```rust
fn upgrade_thumbnail_url(url: &str) -> String {
    let re = regex::Regex::new(r"=w\d+-h\d+|=\w+s\d+").unwrap();
    re.replace(url, "=w600-h600").to_string()
}
```
Insert at end of `draw_media_controls.rs`.

### Phase 2: Fix context menu cohesion (R2-R5)

**Consolidated keybind table** after changes:

#### Playlist (`o` context menu)
| Key | Action | Status |
|-----|--------|--------|
| Enter | PlaySong | ✅ |
| d | DeleteSelected | ✅ |
| l | ViewLyrics | ✅ |
| y | CopySongUrl | ✅ |
| I | ViewSongInfo | ✅ NEW |
| E/e | SaveToExistingPlaylist (current song) | ✅ FIX |
| L | LoadFromYTM | ✅ |

#### Library (`o` context menu)
| Key | Action | Status |
|-----|--------|--------|
| Enter | PlaySong | ✅ |
| p | PlaySongs | ✅ |
| y | CopySongUrl | ✅ |
| Space | AddSongToPlaylist | ✅ NEW |
| P | AddSongsToPlaylist | ✅ NEW |
| l | ViewLyrics | ✅ NEW |

#### Browser song/artist-songs/playlist-songs (`o` context menu)
Remove duplicate `Enter` mode. Keep `o` as the standard context menu entry point.

| Key | Action | Status |
|-----|--------|--------|
| Space | AddSongToPlaylist | ✅ |
| p | PlaySongs | ✅ |
| Enter | PlaySong | ✅ |
| P | AddSongsToPlaylist | ✅ |
| y | CopySongUrl | ✅ |
| l | ViewLyrics | ✅ NEW |
| a | PlayAlbum (artist-songs only) | ✅ EXISTING |
| A | AddAlbumToPlaylist (artist-songs only) | ✅ EXISTING |

### Phase 3: Fix SaveToExistingPlaylist (R6)

Change `SaveToExistingPlaylist` handler in `playlist.rs:234`:
```rust
// Before: collects ALL songs
let video_ids: Vec<VideoID<'static>> = self.list.get_list_iter()
    .map(|song| song.video_id.clone())
    .collect();

// After: collects only current song
let actual_index = self.visual_to_actual_index(self.cur_selected);
let video_ids: Vec<VideoID<'static>> = if let Some(song) = self.get_song_from_idx(actual_index) {
    vec![song.video_id.clone()]
} else {
    Vec::new()
};
```

Also add new action `SaveQueueToPlaylist` for saving the full queue (currently what `SaveToExistingPlaylist` does).

### Phase 4: Build + Test + Commit

```bash
cargo build --release -p youtui --bin youtui
cargo test --release -p youtui --bin youtui
git add -A && git commit -m "feat: ux polish — context menus, album art, cohesion"
```

---

## Summary

| Phase | Item | Time |
|-------|------|------|
| 1 | `upgrade_thumbnail_url` + add `regex` dep check | 5min |
| 2 | Remove duplicate Enter modes (3 contexts) | 15min |
| 2 | Add missing context menu entries (l, Space, I, P) | 15min |
| 3 | Fix SaveToExistingPlaylist to use current song only | 5min |
| 4 | Build + test + commit | 10min |
| **Total** | | **~50min** |
