# Subsystem: Queue

## Data Model

File: `youtui/src/app/ui/playlist.rs` - `Playlist` struct (main)

```rust
pub struct Playlist {
    pub list: Vec<ListSong>,           // Current queue
    pub cur_selected: usize,           // Currently highlighted position
    pub current_song: Option<Arc<ListSong>>,  // Currently playing
    pub current_index: Option<usize>,  // Index of playing song in queue
    pub album_tracks: Option<Vec<ListSong>>,  // Split album tracks
    pub pending_count: usize,          // Count prefix accumulator
    pub scrobbling_config: ScrobblingConfig,
}
```

## Queue Operations

| Operation | Method | Key |
|-----------|--------|-----|
| Play song | `play_song_id(id)` | Enter |
| Next track | `next_song()` | `>` (global) |
| Previous track | `previous_song()` | `<` (global) |
| Add to end | `push_song_list(songs)` | - |
| Remove from queue | `delete_selected()` | `d` then `d` (Delete mode; `dg`/`dG` to top/bottom) |
| Move track up/down (browser) | `MoveTrackUp`/`MoveTrackDown` | `K` / `J` in library view |
| Delete all | `delete_all()` | `o.D` |
| Toggle shuffle | `toggle_shuffle()` | `o.s` |
| Cycle repeat | repeat action | `o.z` |

## Shuffle

File: `youtui/src/app/ui/playlist.rs`

Uses `rand::thread_rng()` to generate a shuffled index order. The original queue order is preserved - shuffle is a view transformation.

```rust
pub fn toggle_shuffle(&mut self) {
    self.shuffled = !self.shuffled;
    if self.shuffled {
        self.shuffle_order = self.generate_shuffle_order();
    } else {
        self.shuffle_order = None;
    }
}
```

## Repeat Modes

```rust
pub enum RepeatMode { Off, All, One }
```

Cycled by repeat action: `Off → All → One → Off`.

- **Off**: queue ends when last track finishes
- **All**: queue loops back to first track after last
- **One**: current track repeats indefinitely

## Persistence

File: `youtui/src/app/queue_persistence.rs`

Queue state saved to disk on exit, loaded on startup via `auto_save` / `auto_load` (`__autosave` queue name):

```rust
pub fn save_queue(playlist: &Playlist, name: &str) -> Result<()>;
pub fn load_queue(playlist: &mut Playlist, name: &str) -> Result<()>;
pub fn auto_save(playlist: &Playlist) -> Result<()>;
pub fn auto_load(playlist: &mut Playlist) -> Result<()>;
```

**File:** `get_data_dir()/youtui/queues/{name}.json` (e.g. `~/.local/share/youtui/youtui/queues/__autosave.json` on Linux; `get_data_dir()` in `youtui/src/main.rs` uses `ProjectDirs::from("com", "nick42", "youtui")`, overridable via `YOUTUI_DATA_DIR`).

**Format (`CompactSavedQueue`):**
```json
{
  "songs": [
    {
      "video_id": "abc123",
      "title": "Song Title",
      "artists": ["Artist Name"],
      "album": "Album Name",
      "duration_string": "3:45",
      "thumbnail_url": "...",
      "like_status": "INDIFFERENT"
    }
  ],
  "current_index": 0
}
```

Compact serialization: album art, thumbnails, and download status are NOT persisted (re-fetched on reload). Only essential metadata saved.

## Gapless Auto-Advance

```
Track within GAPLESS_PLAYBACK_THRESHOLD (1s) of actual_duration end:
  → DecodeSong(next_track, offset, actual_duration) scheduled
  → mapped to QueueDecodedSong(next_song.id)
  → handle_queued decodes; seamless transition on track end
```

Handled in the progress update path (`youtui/src/app/ui/playlist.rs`, gapless block). Only one next track is pre-decoded, only when it is already `DownloadStatus::Downloaded`, never in Repeat One mode, and `QueueState::Queued` guards against double-scheduling. There is no 2-ahead/1-behind buffer.

### Year Enrichment

Queue songs get year (and genre/style) metadata from a batch enrichment pipeline that triggers when songs are added via `push_song_list`.

**Trigger:** `push_song_list` (`playlist.rs`) builds `enrich_data` from all queue songs whose year is `None` and dispatches the `EnrichQueueYears` backend task.

**Resolution:** `EnrichQueueYears` handler calls `resolve_fast()` - a fast-path resolver that queries only ListenBrainz (priority 6) and Last.fm (Album 10, Track 20), avoiding slow/rate-limited providers like MusicBrainz (1 req/s). Each result is cached to LRU + SQLite (even `None` results to prevent re-fetch).

**Completion:** `HandleQueueEnrichYearsOk` applies enrichment results to queue songs via index map. Each result sets `song.year = Some(Rc::new(year))` when year found.

**Per-song enrichment:** `EnrichSongYear` also fires on `play_song_id` / `autoplay_song_id` for the currently playing song when year is `None`. Rate-limited to 1/2s. Includes stale-guard: only applies if song_id + artist match.

**Cache persistence:** LRU (200 entries) → SQLite fallback via `lookup_cache()`. Background flush every 60s + on quit. On restart, `lookup_cache()` checks SQLite before HTTP fetch.
