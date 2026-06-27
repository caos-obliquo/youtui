# Subsystem: Album Splitting

When a YouTube video contains a full album (tracklist), youtui can split it into
individual tracks with metadata, seeking offsets, and gapless playback.

## Flow

```
1. add_yt_video or push_song_list → title cleaned, ValidateMetadata spawned
2. ValidateMetadata → identifies album with tracklist via MetadataProvider pipeline
3. Duration ratio gate: video_dur / metadata_total >= 0.3 (or tag/10min/4-track fallback)
4. insert_album_tracks → creates per-track ListSong entries
5. Arc sharing + original entry removal
```

## Track Creation

File: `youtui/src/app/ui/playlist.rs` - `insert_album_tracks`

Each track becomes a `ListSong` with:

```rust
ListSong {
    track_no: Some(i + 1),                         // Track number
    start_offset: Some(accumulated_duration),       // Offset in full audio
    actual_duration: Some(track_duration),          // Track length
    duration_string: format!("{}:{}", mins, secs),  // Display string
    year: from_validation_result.or(from_ytdlp),    // Year fallback
    download_status: None,                          // Linked later
    album_art: shared_album_art.clone(),            // Arc-cloned
    genres: parent.genres.clone(),                  // Propagated from parent
    styles: parent.styles.clone(),                  // Propagated from parent
    like_status: parent.like_status,                // Propagated from parent
}
```

Track durations accumulate: `offset_1 = 0`, `offset_2 = track_1_duration`,
`offset_3 = track_1 + track_2`, etc. Last track fills remaining time
(`parent_duration - accum`).

## Duration Ratio Gate (Split Decision)

File: `app/ui/playlist/effect_handlers_playlist.rs` - `MetadataEffect::Validated`

Before splitting, checks if the metadata tracklist represents the SAME video:

1. **Duration ratio**: if video duration / metadata total >= 0.3, split.
   Threshold handles deluxe editions (3x bonus tracks).
2. **Fallback gates** (when durations unavailable or ratio < 0.3):
   - Title has album indicator tags (word-boundary match)
   - Video > 10 minutes AND metadata has >= 4 tracks with artist match
3. No gate match → treat as single song, no split.

This prevents false splits when a single song's metadata contains the album
tracklist (e.g., "Metallica - Master of Puppets" returns the full album tracklist
from MusicBrainz but should NOT be split).

## Arc Sharing

File: `youtui/src/app/ui/playlist.rs` - `handle_song_downloaded`

When the original album entry finishes downloading, its `Arc<InMemSong>` is
cheaply cloned to all track entries:

```rust
for track in &mut album_tracks {
    if track.download_status == DownloadStatus::None {
        track.download_status = Some(original_arc.clone());
    }
}
```

## Cascade Guard

File: `youtui/src/app/ui/playlist/effect_handlers_playlist.rs`, `youtui/src/app/ui/playlist.rs`

Prevents re-triggering:

- Effect handler checks `target.album_tracks.is_none()`
- `insert_album_tracks` checks `existing_tracks >= tracks.len()`

## Original Entry Removal

After all tracks are ready (Arc shared + all `Downloaded`), the original
full-album entry is removed from the queue. Two paths:

- **Path A (validation first):** `handle_song_downloaded` → remove original
- **Path B (download first):** `insert_album_tracks` → share Arc → remove
  original

## Decode With Offsets

File: `youtui/src/app/server/messages.rs` - `DecodeSong`

When both offset AND actual_duration are `Some`, ffmpeg extracts with:

```
ffmpeg -ss {offset} -t {actual_duration} -i {full_audio} {output_file}
```

Each track gets its own decoded file of exact length for seamless seeking.

## Progress Display

File: `youtui/src/app/ui/playlist.rs` - `handle_set_song_play_progress`

```rust
if song.track_no.is_some() {
    // Track from split album -- d is already track-relative
    progress = d;
} else if let Some(offset) = song.start_offset {
    // Non-album entry with offset
    progress = d.saturating_sub(offset);
}
progress = progress.min(song.actual_duration.unwrap_or(progress));
```

## Title Cleaning (4 stages)

File: `youtui/src/app/ui/playlist.rs` - `add_yt_video`

1. **Artist prefix strip**: If title starts with artist name followed by `-` or
   `--`, remove the prefix. Single-char artist name guard prevents corruption.
2. **Noise tag strip**: Remove `(Official Audio)`, `(Official Video)`,
   `c legenda`, `Legendado`, `subtitle`, `sub.` inside parens/brackets
3. **Album suffix strip**: Remove album type tags `(full album)`, `(EP)`, `(LP)`,
   `(demo)`, `(single)`, `(album)`, `(full EP)`, `(full LP)`, `(full demo)`,
   `(full single)`, `(singles)` etc. Uses word-boundary token matching
   (not substring) to prevent false matches like "ep" in "Epic".
4. **Year strip**: Remove `(YYYY)` or `[YYYY]` at end of title before metadata
   lookup

Residual trailing whitespace/punctuation `- ,;:/` cleaned after each stage.

## Album Name in Metadata Provider Lookup

File: `libs/metadata-provider/src/lib.rs` - `resolve(artist, title, album)`

`ValidateMetadata` now passes the original album name (from the song's
`ListSong.album` field) to `MetadataRegistry::resolve()`. All 6 providers
receive `album: Option<&str>` for better search accuracy:

- **Discogs, MusicBrainz, Last.fm Album**: use album name as search param
- **MetaApi, Genius, Last.fm Track**: use album name for result filtering

This fixes false matches when the cleaned title alone is too generic for
provider search.

## Metadata Pipeline (Provider Scoring)

File: `libs/metadata-provider/src/lib.rs` - `resolve()`

All providers tried in priority order. Each result scored:

- **+50** = tracklist present (strong signal)
- **+20** = album name matches cleaned title (with `&`⇔`and` normalization)
- **+10** = artist exact match
- **+10** = year matches or present

Best score wins. Minimum threshold for caching: >= 20.

Provider order and priorities:

| Provider | Priority | Notes |
|----------|----------|-------|
| MA_COOKIE (try_direct_ma) | 5 | Direct Metal Archives HTTP (cookie-based) |
| Discogs | 8 | Master API + structured search |
| Last.fm AlbumSearch | 10 | album.getInfo API |
| YTM Album Enrichment | 15 | Post-registry fallback via backend YTM client |
| Last.fm TrackSearch | 20 | track.getInfo API |
| Genius | 40 | Genius API (tracklist detection) |
| MusicBrainz | 50 | MB API (last resort, widest coverage) |

## Original Album Preservation

File: `app/ui/playlist/effect_handlers_playlist.rs`

Before metadata overrides the album name, the original YouTube video title
(cleaned) is saved. `insert_album_tracks()` accepts
`original_album: &Option<String>`. Split tracks use
`original_album.or(metadata_album)`. This keeps user-recognizable album names
over metadata provider names.

## YTM Album Enrichment

File: `youtui/src/app/server/messages.rs` - `ValidateMetadata::into_future()`

After `MetadataRegistry::resolve()` returns, an additional YTM API call enriches
the result with artist name, album name, year, thumbnails, and genres from
YouTube Music's GetAlbum endpoint. This is best-effort: if YTM enrichment fails,
the resolved metadata is still returned (log warning, keep original).

## What Was Tried and Abandoned

- **Per-track validation**: Spawning async lookups for each split track.
  Overwrote correct artist/album with wrong provider results. Removed.
  Year still propagates via `insert_album_tracks()` fallback chain.
- **`url_added` flag**: Previously prevented album splitting for URL-added songs.
  Removed; all sources split equally now.
- **Tag-only gate**: Original impl only checked YouTube title tags
  (`[Full Album]`). Missed official label uploads without tags. Replaced with
  duration ratio heuristic.
- **Exclusive substring tag matching**: Tags like "ep" matched "Epic". Replaced
  with word-boundary token matching.

## Force Split (`o.f`)

File: `youtui/src/app/ui/playlist.rs` - `ForceSplitAlbum` handler

Manual re-split:

1. Finds selected song's parent album entry by matching `original_album` field
2. If no parent entry found, runs metadata pipeline from scratch (uses song's
   artist/title)
3. Removes all existing split tracks (matches by `original_album`)
4. Re-runs `ValidateMetadata` through all providers
5. Re-splits into tracks via `insert_album_tracks()`

Works when parent entry exists OR when original was already deleted from queue.
