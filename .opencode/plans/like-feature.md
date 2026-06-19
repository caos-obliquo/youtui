# Like/Unlike Songs — Perfect Plan

## Design Decisions

### Like scope
- ✅ Playlist tracks (normal entries)
- ✅ Search results (song search, artist songs, playlist songs)
- ✅ Library tracks (liked songs from YTM)
- ❌ Album-splitted tracks (share `Arc<InMemSong>`, complex ownership)
- ✅ Error message when user tries to like a splitted track

### How to detect splitted tracks
A song is "album-splitted" when:
- `song.track_no.is_some()` AND
- `song.album_tracks` is `Some` on the playlist (indicates album split mode)

When user presses like key on a splitted track, show `last_error` toast: "Cannot like splitted album tracks yet"

### Implementation

**Step 1**: Add `like_status` to `ListSong` — need `LikeStatus` import + `Serialize`/`Deserialize`

**Step 2**: Propagate `like_status` from all API sources that have it (PlaylistSong, AlbumSong). Use `LikeStatus::Indifferent` as default for sources without it (SearchResultSong, yt-dlp).

**Step 3**: RateSong backend task + ToggleLike action + like keybind

**Step 4**: Heart icon in footer + song info display

**Step 5**: Error guard for splitted tracks

---

## Files Changed — Full List

### P1: Count Prefix Fix (logs exit)

| File | Line | Change |
|------|------|--------|
| `app/ui.rs` | ~674 | Add `is_count_prefix_active()` check before digit accumulation |

```rust
fn is_count_prefix_active(&self) -> bool {
    matches!(self.context, WindowContext::Playlist | WindowContext::Browser)
}
```

Then gate: `if c.is_ascii_digit() && self.is_count_prefix_active() {`

---

### P2: Like/Unlike Feature

#### 2a: `app/structures.rs`
Add `like_status` to `ListSong`. Import `LikeStatus`:
```rust
use ytmapi_rs::common::LikeStatus;
```
Add field:
```rust
pub like_status: LikeStatus,
```
Default value in all constructions: `LikeStatus::Indifferent`.

#### 2b: Propagation sites (7 files)

| File | Line | From | Has like_status? |
|------|------|------|-----------------|
| `structures.rs:423` | `add_album_song` | `AlbumSong` | YES |
| `structures.rs:458` | `add_raw_search_result_song` | `SearchResultSong` | NO → `Indifferent` |
| `structures.rs:551` | `add_raw_playlist_item` | `PlaylistItem::Song` | YES |
| `playlist.rs:787` | `add_yt_video` | yt-dlp raw | NO → `Indifferent` |
| `library.rs:209` | SongsLoaded | `TableListSong` | Check... |
| `library.rs:261` | PlaylistTracksLoaded | `PlaylistSong` | YES |
| `effect_handlers_playlist.rs:507` | TracksFetched | `PlaylistSong` | YES |
| `effect_handlers_playlist.rs:555` | TracksAppended | `PlaylistSong` | YES |

For `AlbumSong` at `structures.rs:423`, extract `like_status` from `song.like_status` (it has the field).
For `PlaylistItem::Song` at `structures.rs:551`, extract from `like_status` in the destructuring (add it to the `..` ignore list → extract it).

#### 2c: `app/server/messages.rs` — RateSong task

```rust
pub struct RateSong(pub VideoID<'static>, pub LikeStatus);

impl BackendTask<ArcServer> for RateSong {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(self, backend: &ArcServer) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            backend.api.rate_song(self.0, self.1).await
        }
    }
}
```

Add `HandleRateSongOk` / `HandleRateSongErr` task handlers.

#### 2d: `playlist.rs` — ToggleLike action + handler + effect

**Add to `PlaylistAction`**:
```rust
ToggleLike,
```

**Add `describe`**:
```rust
PlaylistAction::ToggleLike => "Like / Unlike",
```

**Add handler in action handler** (around line 280):
```rust
PlaylistAction::ToggleLike => {
    // Check if splitted track
    if self.album_tracks.is_some() {
        if let Some(song) = self.list.get_list_iter().nth(self.cur_selected) {
            if song.track_no.is_some() {
                self.last_error = Some("Cannot like splitted album tracks yet".to_string());
                return (AsyncTask::new_no_op(), None);
            }
        }
    }
    let actual_index = self.visual_to_actual_index(self.cur_selected);
    if let Some(song) = self.get_song_from_idx(actual_index) {
        let new_status = match song.like_status {
            LikeStatus::Liked => LikeStatus::Indifferent,
            _ => LikeStatus::Liked,
        };
        // Update local status immediately (optimistic update)
        song.like_status = new_status.clone();
        let video_id = song.video_id.clone();
        let effect = AsyncTask::new_future_try(
            RateSong(video_id, new_status),
            HandleRateSongOk,
            HandleRateSongErr,
            None,
        ).map_frontend(|this: &mut Self| this);
        return (effect, None);
    }
    (AsyncTask::new_no_op(), None)
}
```

**Add effect handler**:
```rust
#[derive(Debug, PartialEq)]
struct HandleRateSongOk;
#[derive(Debug, PartialEq)]
struct HandleRateSongErr;
impl_youtui_task_handler!(HandleRateSongOk, (), Playlist, |_, _: ()| {
    info!("Song rated successfully");
    AsyncTask::new_no_op()
});
impl_youtui_task_handler!(HandleRateSongErr, anyhow::Error, Playlist, |_, err: anyhow::Error| {
    error!("Failed to rate song: {}", err);
    AsyncTask::new_no_op()
});
```

#### 2e: `footer.rs` — Heart icon display

After line 73 (`format!("{} {} - ", ...)`), add the heart icon:
```rust
let like_icon = match song.like_status {
    LikeStatus::Liked => " 󰋑 ".to_string(),
    _ => " ♡ ".to_string(),
};
let mut s = format!("{}{}{} {} - ", w.playlist.play_status.list_icon(), like_icon, song.title);
```

Also add `use ytmapi_rs::common::LikeStatus;` to footer.rs.

#### 2f: `config/keymap.rs` — Keybind

Add to `default_playlist_keybinds`:
```rust
(
    Keybind::new_unmodified(crossterm::event::KeyCode::Char('l')),
    KeyActionTree::new_key_with_visibility(
        AppAction::Playlist(PlaylistAction::ToggleLike),
        KeyActionVisibility::Global,
    ),
),
```

Wait — `l` already exists in the playlist context? Let me check... Looking at the current playlist keybinds, `l` is NOT used standalone (it was used in the `o` context menu but I changed it to `r`). So `l` is free.

Actually wait — in the browser context, `l` = `BrowserAction::Right`. But in the playlist context, `l` is not used. Different contexts can use the same key differently.

---

## Files Summary Table

| File | Lines | Change |
|------|-------|--------|
| `app/ui.rs` | 5 | Count prefix context gate |
| `app/structures.rs` | 3 | Add `like_status` to `ListSong` + import |
| `app/structures.rs:423` | 1 | Propagate `like_status` from AlbumSong |
| `app/structures.rs:458` | 1 | Add `like_status: Indifferent` |
| `app/structures.rs:551` | 3 | Propagate `like_status` from PlaylistItem + destructure |
| `app/ui/playlist.rs:787` | 1 | Add `like_status: Indifferent` |
| `app/ui/browser/library.rs:209` | 1 | Add `like_status: Indifferent` (TableListSong) |
| `app/ui/browser/library.rs:261` | 3 | Propagate `like_status` + destructure |
| `app/ui/playlist/effect_handlers_playlist.rs:507,555` | 4 | Propagate `like_status` twice |
| `app/server/messages.rs` | 15 | `RateSong` task + handlers |
| `app/ui/playlist.rs` (actions) | 25 | `ToggleLike` action + handler + guard |
| `app/ui/footer.rs` | 5 | Heart icon display |
| `config/keymap.rs` | 5 | `l` = ToggleLike keybind |
| **Total** | **~75** | |

## Execution Order

| # | Step | Est. |
|---|------|------|
| 1 | P1: Count prefix gate (fixes logs exit) | 5min |
| 2 | P2a: `like_status` field on `ListSong` | 3min |
| 3 | P2b: Propagate to all 7 construction sites | 10min |
| 4 | P2c: `RateSong` backend task | 8min |
| 5 | P2d: `ToggleLike` action + handler + splitted guard | 12min |
| 6 | P2e: Heart icon in footer | 5min |
| 7 | P2f: `l` keybind | 3min |
| 8 | Build + test + commit | 10min |
| | **Total** | **~56min** |
