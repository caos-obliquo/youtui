# Subsystem: Queue

## Data Model

File: `youtui/src/app/ui/playlist.rs` - `Playlist` struct (main, ~3104 lines)

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
| Play song | `play_song(id)` | Enter |
| Next track | `next_song()` | `l` |
| Previous track | `previous_song()` | `h` |
| Add to end | `add_song_to_playlist(song)` | - |
| Remove from queue | `remove_from_playlist(id)` | `d` |
| Move up | `shift_up(id)` | `K` |
| Move down | `shift_down(id)` | `J` |
| Clear queue | (context menu) | `o.c` |
| Toggle shuffle | `toggle_shuffle()` | - |
| Cycle repeat | `cycle_repeat()` | - |

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

File: `app/queue_persistence.rs`

Queue state saved to disk on exit, loaded on startup:

```rust
pub fn save(queue: &[ListSong], current_index: Option<usize>) -> Result<()>;
pub fn load() -> Result<(Vec<ListSong>, Option<usize>)>;
```

**File:** `~/.cache/youtui/queue.json`

**Format:**
```json
{
  "queue": [
    {
      "video_id": "abc123",
      "title": "Song Title",
      "artists": ["Artist Name"],
      "album": "Album Name",
      "duration_string": "3:45"
    }
  ],
  "current_index": 0
}
```

Compact serialization: album art, thumbnails, and download status are NOT persisted (re-fetched on reload). Only essential metadata saved.

## Gapless Auto-Advance

```
Track ends (within 1s of actual_duration):
  → QueueDecodedSong(next_track) scheduled
  → Next track pre-decoded via DecodeSong
  → Seamless playback transition
```

Handled in progress update loop (`handle_set_song_play_progress`, ~10Hz check).

## Buffer

- **Ahead**: 2 songs pre-buffered (decode started before current ends)
- **Behind**: 1 song saved (for immediate previous-track seek)

### Year Enrichment

Queue songs get year (and genre/style) metadata from a batch enrichment pipeline that triggers when songs are added via `push_song_list`.

**Trigger:** `push_song_list` (playlist.rs:2021) builds `enrich_data` from all songs whose year is `None`. Dispatches `EnrichQueueYears` backend task.

**Resolution:** `EnrichQueueYears` handler calls `resolve_fast()` — a fast-path resolver that queries only ListenBrainz (priority 6) and Last.fm (Album 10, Track 20), avoiding slow/rate-limited providers like MusicBrainz (1 req/s). Each result is cached to LRU + SQLite (even `None` results to prevent re-fetch).

**Completion:** `HandleQueueEnrichYearsOk` applies enrichment results to queue songs via index map. Each result sets `song.year = Some(Rc::new(year))` when year found.

**Per-song enrichment:** `EnrichSongYear` also fires on `play_song_id` / `autoplay_song_id` for the currently playing song when year is `None`. Rate-limited to 1/2s. Includes stale-guard: only applies if song_id + artist match.

**Cache persistence:** LRU (200 entries) → SQLite fallback via `lookup_cache()`. Background flush every 60s + on quit. On restart, `lookup_cache()` checks SQLite before HTTP fetch.
