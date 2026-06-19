# Final Implementation Plan — Build Fixes + Direct Artist Navigation

## Part 1: Fix Build Errors (~20 min, 10 errors)

### E1: `HandleRateSongOk`/`HandleRateSongErr` not in scope
**File**: `app/ui/playlist.rs`
**Fix**: Add imports at top:
```rust
use crate::app::ui::playlist::effect_handlers_playlist::{
    HandleRateSongOk, HandleRateSongErr, ...
};
```

### E2: `rate_song` method on `RwLockReadGuard`
**File**: `app/server/messages.rs:88`
**Root cause**: `DynamicYtMusic` implements `rate_song` as a method on `SimplifiedQueries` trait. Need to bring the trait into scope.
**Fix**: Add `use ytmapi_rs::simplified_queries::SimplifiedQueries;` at top of `messages.rs`.

Actually, looking at the ytmapi-rs crate, `rate_song` is defined in `simplified_queries.rs` as:
```rust
impl SimplifiedQueries for DynamicYtMusic { ... }
```
But `DynamicYtMusic` might have `rate_song` directly if it's inherent. Let me check...

The method is in `impl DynamicYtMusic` block at line 851. So it's an inherent method, not a trait method. The issue is that `RwLockReadGuard` doesn't auto-deref to the inner type for method calls... actually it DOES via `Deref`. The error says `no method named rate_song` for the guard type, which means the Deref isn't working.

The fix: use `let api = api_guard.read().await;` then call `api.rate_song(...).await`. But the error says `rate_song` is not found on `RwLockReadGuard`. This might be because `rate_song` takes `&self` and `Deref` should forward...

Let me try a different approach: use `api_guard.read().await` in a block:
```rust
let rating = self.1;
let video_id = self.0;
let guard = api_guard.read().await;
guard.rate_song(video_id, rating).await?;
```
This should work because `RwLockReadGuard` implements `Deref<Target=DynamicYtMusic>`.

### E3: `FrontendEffect` trait bound
**File**: `app/component/actionhandler.rs` — This is inside the `impl_youtui_task_handler!` macro for `HandleRateSong*`. The error means the handler's `handle()` method returns a type that doesn't implement `FrontendEffect<Playlist, ArcServer, TaskMetadata>`.
**Fix**: Change the handler impls to return the correct type. Current:
```rust
impl_youtui_task_handler!(
    HandleRateSongOk,
    (),
    Playlist,
    |_, _: ()| {
        info!("Song rated successfully");
        AsyncTask::new_no_op()
    }
);
```
The `impl_youtui_task_handler!` macro expects the closure to return a `FrontendEffect`. `AsyncTask::new_no_op()` should implement this. Unless the macro is producing mismatched types.

**Fix**: Use `ComponentEffect::<Playlist>::new_no_op()` instead of `AsyncTask::new_no_op()`:
```rust
|_, _: ()| {
    info!("Song rated successfully");
    ComponentEffect::<Playlist>::new_no_op()
}
```

### E4 + E5: Missing `like_status`
**File**: `effect_handlers_playlist.rs:581` — add `like_status: s.like_status,`
**File**: `structures.rs:556` — need to add `like_status` to `add_raw_playlist_item`. The destructuring at line 488 uses a tuple `(track_no, title, ...)` and the `PlaylistItem::Song` arm already has `..` which ignores `like_status`. Need to extract it.

### E6: `draw_table_impl` argument count
**File**: `ui/draw.rs:239` — the `draw_table` call passes 11 args but function expects 12.

Need to check the exact call site and add the missing `visual_range` parameter.

---

## Part 2: Direct Artist Navigation Architecture (~25 min)

### Architecture

**Step 1**: Add `NavTarget::ArtistChannel(ArtistChannelID)` to `app.rs`:
```rust
pub enum NavTarget {
    Artist(String),
    ArtistChannel(ArtistChannelID<'static>),
    Album { artist: String, album: String },
    SongSearch(String),
}
```

**Step 2**: Add `load_artist_by_id` to `ArtistSearchBrowser` (`artistsearch.rs`):
```rust
pub fn load_artist_by_id(&mut self, channel_id: ArtistChannelID<'static>) -> ComponentEffect<Self> {
    self.change_routing(InputRouting::Song);
    self.artist_search_panel.search_popped = false;
    self.album_songs_panel.list.clear();
    AsyncTask::new_stream(
        GetArtistSongs(channel_id.clone()),
        HandleGetArtistSongsProgressUpdate(channel_id),
        Some(Constraint::new_kill_same_type()),
    )
}
```

This method:
- Switches to Song routing (shows songs panel)
- Hides the search popup
- Clears previous song results
- Starts streaming the artist's songs directly

**Step 3**: Add `NavTarget::ArtistChannel` arm to `Browser::navigate_to()` (`browser.rs`):
```rust
NavTarget::ArtistChannel(channel_id) => {
    self.variant = BrowserVariant::Artist;
    self.artist_search_browser.input_routing = artistsearch::InputRouting::Artist;
    self.artist_search_browser.artist_search_panel.search_popped = false;
    Some(self.artist_search_browser.load_artist_by_id(channel_id).map_frontend(|b: &mut Browser| &mut b.artist_search_browser))
}
```

**Step 4**: Change `LibraryCategory::Artists` handler (`library.rs`):
```rust
LibraryCategory::Artists => match action {
    BrowserSongsAction::PlaySong => {
        if let Some(artist) = self.artist_data.get(self.artist_selected) {
            return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::ArtistChannel(artist.channel_id.clone()))));
        }
    }
    _ => warn!("Unsupported song action for artists: {:?}", action),
},
```

**Step 5**: Handle `ArtistChannelID` import — ensure `ArtistChannelID` is imported in `app.rs`:
```rust
use ytmapi_rs::common::{PlaylistID, VideoID, ArtistChannelID};
```

---

## Files Changed — Full Table

| File | Lines | Change |
|------|-------|--------|
| `app.rs` | 3 | Add `ArtistChannelID` import + `NavTarget::ArtistChannel` variant |
| `app/ui/browser.rs` | 10 | Add `navigate_to` arm for `ArtistChannel` |
| `app/ui/browser/artistsearch.rs` | 8 | Add `load_artist_by_id` method |
| `app/ui/browser/library.rs` | 3 | Change Artists PlaySong to dispatch `ArtistChannel` |
| `app/server/messages.rs` | 4 | Fix `rate_song` API call + add trait import |
| `app/ui/playlist.rs` | 2 | Add `HandleRateSongOk/Err` imports |
| `app/ui/playlist/effect_handlers_playlist.rs` | 2 | Add `like_status` to TracksAppended |
| `app/ui/playlist/effect_handlers_playlist.rs` | 4 | Fix handler return types |
| `app/structures.rs` | 5 | Add `like_status` to `add_raw_playlist_item` destructuring |
| `app/ui/draw.rs` | 2 | Fix `draw_table_impl` argument count |
| **Total** | **~45** | |

## Execution Order

| # | Step | Est. |
|---|------|------|
| 1 | Fix all 10 build errors | 20min |
| 2 | Add `NavTarget::ArtistChannel` + `navigate_to` arm | 10min |
| 3 | Add `load_artist_by_id` to ArtistSearchBrowser | 8min |
| 4 | Change Library Artists handler | 3min |
| 5 | Build + test + commit | 10min |
| | **Total** | **~50 min** |
