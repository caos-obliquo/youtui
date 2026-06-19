# Re-evaluation: Keybinding Standard + Library Playlist Browser

## Keybinding Audit — Every Context

### Legend
`y` = yank/copy in vim. `l` = right. `h` = left. `r` = replace.
`i` = insert. `I` = insert at line start. `o` = open below line.
`p` = paste. `P` = paste above.

### Current vs Proposed

| Key | Vim meaning | Current binding | Problem | Proposed |
|-----|------------|----------------|---------|----------|
| `y` | yank | ViewLyrics / CopySongUrl | Conflict: yank vs copy URL | **CopySongUrl** (yank=copy, fits) |
| `l` | right | Browser right / List.down(?) | Vim muscle memory: `l`=right, not lyrics | **right/nav ONLY**, never lyrics |
| `r` | replace | ReloadCategory (browser only) | FREE in all other contexts | **ViewLyrics** (user suggestion) |
| `i` | insert | unbound globally | Reserved for future "info" | **view info** (maybe rename `I`→`i`) |
| `I` | insert at line start | ViewSongInfo (playlist) | OK, caps = different action | Keep as ViewSongInfo |
| `R` | replace mode | ToggleRomaji (playlist) | OK, caps variant | Keep |
| `h` | left | Browser left | OK | Keep |
| `L` | (no vim meaning) | LoadFromYTM | OK | Keep |
| `E` | (no vim meaning) | SaveToExistingPlaylist | OK | Keep |
| `g` | go to line | GoToArtist/GoToAlbum mode | OK, new mode prefix | Keep |
| `space` | (no vim meaning) | AddSongToPlaylist | OK | Keep |
| `p` | paste | PlaySongs | Conflict: paste vs play all | **Keep** (context-dependent, `p` in `o` mode vs standalone) |
| `P` | paste above | AddSongsToPlaylist | OK, caps variant | Keep |

### Standard Key Table (all song contexts)

| Key | Action | Rationale |
|-----|--------|-----------|
| `y` | CopySongUrl | yank = copy, natural fit |
| `r` | ViewLyrics | r = "reveal" / "read", FREE in most contexts |
| `o` | Context menu | Consistent everywhere |
| `g a/b` | GoToArtist/Album | g = "go to" prefix |
| `space` | AddSongToPlaylist | Intuitive |
| `p` | PlaySongs | p in context menu |
| `P` | AddSongsToPlaylist | Caps variant |
| `Enter` | PlaySong | Standard |
| `/` | ToggleSearch | Universal |
| `d` | Delete | Delete selected |
| `h`/`l` | Browser nav | Vim standard |

### Files to change

| File | Change |
|------|--------|
| `keymap.rs` — playlist context | `y`(ViewLyrics) → `y`(CopySongUrl), add `r`(ViewLyrics) |
| `keymap.rs` — browser_artist_songs | `y`(ViewLyrics) → `y`(CopySongUrl), add `r`(ViewLyrics), remove `l` from `o` mode |
| `keymap.rs` — browser_playlist_songs | `y`(ViewLyrics) → already done ✅, add `r`(ViewLyrics), remove `l` from `o` mode |
| `keymap.rs` — browser_songs | `y`(ViewLyrics) → `y`(CopySongUrl), add `r`(ViewLyrics), remove `l` from `o` mode |
| `keymap.rs` — browser_library | Remove `l` from `o` mode, keep `y`=CopySongUrl (already correct) |
| `keymap.rs` — playlist context menu | Remove `l`, add `r` |
| `action.rs` | Verify `r` doesn't conflict with any existing |
| All song handler files | Add `ViewLyrics` handler for `r` where missing |

---

## Feature: Library Playlist Enter → Show Songs In-Browser

### Current behavior
```
Library → Playlists → Enter → switches to Playlist view (WindowContext::Playlist)
```

### Desired behavior
```
Library → Playlists → Enter → shows playlist songs in right panel of Library browser
                              (like Artist browser shows songs in right panel)
```

### Implementation plan

**Step 1**: Add playlist tracks state to `LibraryBrowser`

```rust
// library.rs
pub struct LibraryBrowser {
    // ... existing fields ...
    pub playlist_tracks: Vec<ListSong>,          // NEW
    pub playlist_tracks_loaded: bool,            // NEW
    pub playlist_tracks_selected: usize,         // NEW
    pub show_playlist_tracks: bool,              // NEW — when true, shows tracks instead of playlist list
}
```

**Step 2**: Add `PlaylistSongsLoaded` effect variant

```rust
// library.rs
pub enum LibraryEffect {
    SongsLoaded(Vec<ListSong>),
    PlaylistsLoaded(Vec<LibraryPlaylist>),
    PlaylistTracksLoaded(Vec<ListSong>),   // NEW
    ArtistsLoaded(Vec<LibraryArtist>),
    AlbumsLoaded(Vec<SearchResultAlbum>),
    LoadError(String),
}
```

**Step 3**: Task handler + effect

```rust
// library.rs
impl_youtui_task_handler!(HandleLibraryPlaylistTracksOk, Vec<PlaylistSong>, LibraryBrowser, |_, raw: Vec<PlaylistSong>| {
    let songs: Vec<ListSong> = raw.into_iter().map(|ps| ListSong {
        // convert PlaylistSong → ListSong
        video_id: ps.video_id,
        title: ps.title,
        artists: MaybeRc::Owned(ps.artists.into_iter().map(|a| ListSongArtist { name: a.name, id: None }).collect()),
        track_no: None,
        album: ps.album.map(|a| ListSongAlbum { name: a.name, id: ... }),
        year: ps.year.map(|y| y.to_string()),
        // ... other fields with defaults
    }).collect();
    LibraryEffect::PlaylistTracksLoaded(songs)
});
```

**Step 4**: `PlaySong` action in Playlists category → fetch tracks instead of loading into playlist

```rust
// library.rs ActionHandler<BrowserSongsAction>
LibraryCategory::Playlists => match action {
    BrowserSongsAction::PlaySong => {
        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
            // Fetch tracks and show in right panel
            return (self.fetch_playlist_tracks(pl.playlist_id.clone()), None);
        }
    }
    // ...
}
```

```rust
fn fetch_playlist_tracks(&mut self, id: PlaylistID<'static>) -> AsyncTask<...> {
    self.show_playlist_tracks = true;
    AsyncTask::new_future_try(
        GetPlaylistTracks(id),
        HandleLibraryPlaylistTracksOk,
        HandleLibraryPlaylistTracksErr,
        None,
    ).map_frontend(|this: &mut Self| this)
}
```

**Step 5**: Draw — when `show_playlist_tracks`, display `playlist_tracks` in right panel

```rust
// draw.rs:draw_library_browser — Playlists category
if browser.show_playlist_tracks {
    // Show tracks list instead of playlist name list
    // Similar to how LikedSongs are drawn
    let items: Vec<ListItem> = browser.playlist_tracks.iter().enumerate().map(...).collect();
    // ...
} else {
    // Show playlist names (current behavior)
    let items: Vec<ListItem> = browser.playlist_data.iter().enumerate().map(...).collect();
}
```

**Step 6**: `Esc` or `h` to go back to playlist list

```rust
// When show_playlist_tracks is true and user presses Esc or h
KeyCode::Esc | KeyCode::Char('h') => {
    self.show_playlist_tracks = false;
    self.playlist_tracks.clear();
}
```

**Step 7**: Wire actions on tracks (Enter = play, y = copy URL, etc.)

Same as LikedSongs actions but against `playlist_tracks[playlist_tracks_selected]`.

---

## Execution Order

| # | Item | Est. | Files |
|---|------|------|-------|
| 1 | Rebind: `r`=lyrics, `y`=copy in all contexts | 15min | `keymap.rs` × 5 contexts |
| 2 | Remove `l` from all context menus (`o` modes) | 5min | `keymap.rs` × 4 contexts |
| 3 | Add playlist tracks state + fetch + effect | 30min | `library.rs` |
| 4 | Draw playlist tracks in right panel | 20min | `draw.rs` |
| 5 | Esc/h to go back + actions on tracks | 15min | `library.rs` |
| 6 | Build + test + commit | 10min |
| | **Total** | **~1.5h** |
