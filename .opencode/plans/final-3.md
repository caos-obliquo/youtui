# Final Polish Plan — 3 Items

## Item 1: Esc to Exit `/` Search (Verify + Fix)

### Current state
- Playlist: `/` toggles search. `Esc` keybind was just added (`ClearSearch`). Should work now.
- Library browser: `/` toggles `InputRouting::Search`. `Esc` should route through `browser_search` keybinds → `BrowserSearchAction::Close` → `handle_toggle_search()`. 
- Not sure if the TextHandler swallows Esc before it reaches keybind system.

### Verification
1. Build and test: press `/` in library → search box shows → press `Esc` → search box closes
2. If Esc doesn't work: the issue is `TextHandler::handle_text_event_impl` returning `None` for Esc, but the key never reaches `handle_key_event`. Need to check `try_handle_text` flow.

### Fix if broken
In `app/ui.rs`, the `handle_crossterm_event` flow:
```rust
if let Some(effect) = self.try_handle_text(&event) {
    return effect.into();
}
match event {
    Event::Key(k) => return self.handle_key_event(k),
    ...
}
```

When TextHandler returns `None` for Esc, `try_handle_text` returns `None` and the key falls through to `handle_key_event`. This should work.

If it DOESN'T work, the fix: in `LibraryBrowser::handle_text_event_impl`, DON'T return `None` for Esc. Instead, call `handle_toggle_search()` directly:
```rust
KeyCode::Esc => {
    self.handle_toggle_search();
    return Some(AsyncTask::new_no_op());
}
```

### Time: 5min verification

---

## Item 2: Nerd Font Icons in Context Menus

### Current state
Context menu keys are displayed as plain keyboard characters:
```
o → d (Delete), r (Lyrics), y (Copy), E (Save), etc.
```

### Desired state
Use Nerd Font icons to make the menu more visual:
- `E` → `` (folder-add icon for "Save to existing playlist")
- Or keep `E` as the key but add an icon prefix in the description

### Approach
The context menu keys are keyboard triggers — they MUST be keyboard characters (you can't type Nerd Font icons). So the keybinding stays as `E`.

**Option A**: Add icon to the action `describe()` text used in help/status display.
- Each `Action::describe()` returns a string like `"Save to existing playlist"`
- Could prefix with Nerd Font icon: `" Save to existing playlist"`

**Option B**: Change the key to a Nerd Font character for non-keyboard triggers (not possible — keys must be physical keyboard keys).

**Option C**: Leave as-is for now — the plain letters are clean and functional.

### Files & Lines
- `app/ui/action.rs` — `describe()` methods for each action (~10 files to update)
- One line per action description (~20 actions across all contexts)

### Recommendation: Option A
Add Nerd Font icons to `describe()` texts:
```rust
PlaylistAction::SaveToExistingPlaylist => " Save to existing playlist",
PlaylistAction::DeleteSelected => " Delete selected",
PlaylistAction::ViewLyrics => "󰋼 View lyrics",
PlaylistAction::CopySongUrl => "󰖟 Copy URL",
PlaylistAction::GoToArtist => "󰓇 Go to Artist",
PlaylistAction::GoToAlbum => "󰀥 Go to Album",
PlaylistAction::ToggleLike => "󰋑 Like / Unlike",
```

### Time: ~20 min for all action enums

---

## Item 3: Album Art Quality in Footer (720p)

### Current state
- `upgrade_thumbnail_url()` exists in `draw_media_controls.rs` but only applied to external media controls (MPRIS)
- Footer at `footer.rs` renders album art from `AlbumArtState::Downloaded` (high-res from `FetchAlbumArt` Last.fm) OR raw YTM thumbnails (low-res 60×60)
- `FetchAlbumArt` is only triggered for album-split songs, NOT for regular playlist entries

### Root cause
The footer has TWO code paths for album art:
1. `AlbumArtState::Downloaded` — high-res from Last.fm's `FetchAlbumArt` 
2. YTM thumbnails from `s.thumbnails` — low-res, NOT upgraded

Path 2 is the fallback when `FetchAlbumArt` hasn't been called (which is most songs).

### Fix

**Step 1**: Apply `upgrade_thumbnail_url()` in `footer.rs` when falling back to YTM thumbnails.

In `footer.rs`, the album art rendering at line 180-209:
```rust
match album_art {
    Some(AlbumArtState::Downloaded(album_art)) => { /* high-res */ }
    _ => { /* fallback — currently just shows blank space " " */ }
}
```

The footer currently doesn't even use the YTM thumbnails for the album art display! It only uses `AlbumArtState::Downloaded`. When that's not available, it shows blank space.

Wait — let me re-check. The `album_art` variable is `cur_active_song.map(|s| &s.album_art)`. If `AlbumArtState::Downloaded` is not set, the fallback shows `" "` (blank) or `""` (error).

So the album art is BLANK for most songs! The user only sees album art for songs where `FetchAlbumArt` was triggered (album-split songs or metadata-validated ones).

**Fix**: Ensure `FetchAlbumArt` is triggered for ALL songs when they're displayed, not just album-split ones.

**Step 2**: In `playlist.rs`, when a song starts playing AND has album metadata AND `AlbumArtState` is not `Downloaded`, spawn `FetchAlbumArt`:

```rust
// In the play-song handler or a tick handler:
if matches!(song.album_art, AlbumArtState::None) && song.album.is_some() {
    let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
    let album_name = song.album.as_ref().map(|a| a.name.clone()).unwrap_or_default();
    if !album_name.is_empty() {
        let effect = AsyncTask::new_future_try(
            FetchAlbumArt(artist, album_name, api_key),
            HandleFetchAlbumArtOk(/* video_id or song_id */),
            HandleFetchAlbumArtErr,
            None,
        );
        // spawn effect...
    }
}
```

But this requires knowing the `api_key` in the playlist context. Looking at how it's done elsewhere: `FetchAlbumArt` is used in `effect_handlers_playlist.rs` after metadata validation.

**Step 3**: Add a simpler approach: instead of spawning `FetchAlbumArt` (which queries Last.fm), just upgrade the YTM thumbnail URL to 600x600 in all places where thumbnails are displayed.

The `footer.rs` doesn't currently display YTM thumbnails at all — it only shows `Downloaded` art. I could add a fallback that uses the upgraded thumbnail.

But actually, the album art in the footer uses `ratatui_image` which renders images from files or URLs. If I apply `upgrade_thumbnail_url()` to the URL and pass it to `ratatui_image`, it should work.

**Simplest fix**: In `footer.rs`, when `AlbumArtState::Downloaded` is NOT set, fall back to the upgraded YTM thumbnail URL:

```rust
// In footer.rs, after the album_art match:
if album_art.is_none() || matches!(album_art, Some(AlbumArtState::None)) {
    if let Some(song) = cur_active_song {
        if let Some(thumb) = song.thumbnails.iter().max_by_key(|t| t.height * t.width) {
            let url = upgrade_thumbnail_url(&thumb.url);
            // Use url for display via ratatui_image or external viewer
        }
    }
}
```

But `ratatui_image` might not support displaying from URLs directly. It works with `DynamicImage` which comes from `FetchAlbumArt`'s disk cache.

**Realistic approach**:
1. Apply `upgrade_thumbnail_url()` in `footer.rs` where YTM thumbnails could be used
2. Ensure `FetchAlbumArt` is triggered in the song "now playing" handler

### Files & Lines

| File | Lines | Change |
|------|-------|--------|
| `app/ui/footer.rs` | 5 | Apply `upgrade_thumbnail_url` in fallback path |
| `app/ui/playlist.rs` | 15 | Spawn `FetchAlbumArt` when playing song has album metadata |
| `app/ui/draw_media_controls.rs` | 5 | Already done ✅ |
| **Total** | **~25** | |

### Risk
- `ratatui_image` may not support URL-based images (needs file path from `SongThumbnail`)
- Adding `FetchAlbumArt` for all songs adds Last.fm API calls (rate limited)

### Time: ~30 min

---

## Execution Order

| # | Item | Est. |
|---|------|------|
| 1 | Verify/ fix Esc to exit `/` search | 5min |
| 2 | Nerd Font icons in action descriptions | 20min |
| 3 | Album art quality fix | 30min |
| | **Total** | **~55min** |

## Summary

All three items are independent. Items 1 and 2 are quick wins. Item 3 has the most impact (album art is currently blank for most songs) but requires the most careful implementation due to `ratatui_image` constraints.
