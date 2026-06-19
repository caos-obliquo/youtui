# Fix Plan: Logs Exit + Like/Unlike Songs

## P1: Logs Exit Bug — Count Prefix Context Gate

### Root cause
`handle_key_event` intercepts ALL digits (`0`-`9`) as count prefix, including when in the Logs view. The log keybind `5` = `LoggerAction::ViewBrowser` never fires because `5` is consumed by the count accumulator.

### Fix
Add a context check before accumulating digits. Only accumulate when in a scrollable list context:

```rust
// In handle_key_event, before the digit accumulation:
fn is_count_prefix_active(&self) -> bool {
    matches!(self.context, WindowContext::Playlist | WindowContext::Browser)
}
```

Then gate the digit check:
```rust
if c.is_ascii_digit() && self.is_count_prefix_active() {
    // accumulate as count prefix
} else if c.is_ascii_digit() {
    // Don't accumulate — pass through to keybind system normally
    self.key_stack.push(key_event);
}
```

### File: `app/ui.rs:674-678`
### Lines: ~5

---

## P2: Like/Unlike Songs Feature

### What needs to change

**2a: Add `like_status` to `ListSong`** — `app/structures.rs:83`

```rust
pub struct ListSong {
    // ... existing fields ...
    pub like_status: LikeStatus,
}
```

Requires `use ytmapi_rs::common::LikeStatus;` import.

**2b: Propagate `like_status` in all conversions**

Every place where `ListSong` is constructed from API results:
- `library.rs` — `SongsLoaded` handler (line 190-215)
- `playlist.rs` — `add_yt_video` (line 737)
- `effect_handlers_playlist.rs` — `TracksFetched` / `TracksAppended`
- `messages.rs` — all places where `PlaylistSong` → `ListSong`
- `songsearch.rs` — search results
- `artistsearch/songs_panel.rs` — artist songs
- `playlistsearch/songs_panel.rs` — playlist songs

Each needs: `like_status: s.like_status.clone()` added.

**2c: Add `RateSong` backend task** — `app/server/messages.rs`

New backend task:
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

**2d: Add `ToggleLike` action** — `app/ui/playlist.rs`

New `PlaylistAction::ToggleLike` variant. Handler toggles current song's `like_status` and dispatches `RateSong`:

```rust
PlaylistAction::ToggleLike => {
    let actual_index = self.visual_to_actual_index(self.cur_selected);
    if let Some(song) = self.get_song_from_idx(actual_index) {
        let new_status = match song.like_status {
            LikeStatus::Liked => LikeStatus::Indifferent,
            _ => LikeStatus::Liked,
        };
        song.like_status = new_status.clone();
        let task = AsyncTask::new_future_try(
            RateSong(song.video_id.clone(), new_status),
            HandleRateSongOk,
            HandleRateSongErr,
            None,
        );
        return (task, None);
    }
}
```

**2e: Show heart in footer** — `app/ui/footer.rs`

In the footer draw code, after the song title/artist, add:
```rust
if let LikeStatus::Liked = song.like_status {
    // show filled heart "♥"
} else {
    // show outline heart "♡"
}
```

**2f: Add keybind** — `config/keymap.rs`

Add `L` or `l` = `PlaylistAction::ToggleLike` to playlist context (both standalone and in `o` context menu).

### Files summary

| File | Lines | Change |
|------|-------|--------|
| `structures.rs` | 3 | Add `like_status: LikeStatus` to `ListSong` |
| `library.rs` | 1 | Propagate `like_status` in SongsLoaded |
| `playlist.rs` | 20 | Add `ToggleLike` action + handler + rate callback |
| `effect_handlers_playlist.rs` | 4 | Propagate `like_status` in track conversions |
| `songsearch.rs` | 1 | Propagate `like_status` |
| `artistsearch/songs_panel.rs` | 1 | Propagate `like_status` |
| `playlistsearch/songs_panel.rs` | 1 | Propagate `like_status` |
| `messages.rs` | 10 | Add `RateSong` backend task |
| `footer.rs` | 10 | Show heart icon based on like_status |
| `keymap.rs` | 5 | Add `L` = ToggleLike keybind |
| `ui.rs` | 5 | Fix count prefix context gate |
| **Total** | **~60** | |

---

## Execution Order

| # | Item | Est. |
|---|------|------|
| 1 | P1: Fix count prefix context gate (logs exit) | 5min |
| 2 | P2a: Add `like_status` to `ListSong` struct | 5min |
| 3 | P2b: Propagate `like_status` in all conversions (7 files) | 10min |
| 4 | P2c: Add `RateSong` backend task | 5min |
| 5 | P2d: Add `ToggleLike` action + handler | 10min |
| 6 | P2e: Show heart icon in footer | 10min |
| 7 | P2f: Add `L` keybind | 5min |
| 8 | Build + test + commit | 10min |
| | **Total** | **~1h** |
