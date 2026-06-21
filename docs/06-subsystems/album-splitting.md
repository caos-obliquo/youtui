# Subsystem: Album Splitting

When a YouTube video contains a full album (tracklist), youtui can split it into individual tracks with metadata, seeking offsets, and gapless playback.

## Flow

```
1. add_yt_video → title cleaned, ValidateMetadata spawned
2. ValidateMetadata → identifies album with tracklist
3. insert_album_tracks → creates per-track ListSong entries
4. Arc sharing + optional original removal
```

## Track Creation

File: `app/ui/playlist.rs:575` — `insert_album_tracks`

Each track becomes a `ListSong` with:

```rust
ListSong {
    track_no: Some(i + 1),                         // Track number
    start_offset: Some(accumulated_duration),       // Offset in full audio
    actual_duration: Some(track_duration),          // Track length
    duration_string: format!("{}:{}", mins, secs),  // Display string
    year: from_validation_result.or(from_ytdlp),    // Year fallback
    download_status: None,                          // Linked later
    album_art: shared_album_art.clone(),           // Arc-cloned
}
```

Track durations accumulate: `offset_1 = 0`, `offset_2 = track_1_duration`, `offset_3 = track_1 + track_2`, etc.

## Arc Sharing

File: `app/ui/playlist.rs:893` — `handle_song_downloaded`

When the original album entry finishes downloading, its `Arc<InMemSong>` is cheaply cloned to all track entries:

```rust
for track in &mut album_tracks {
    if track.download_status == DownloadStatus::None {
        track.download_status = Some(original_arc.clone());
    }
}
```

## Cascade Guard

File: `app/ui/playlist/effect_handlers_playlist.rs:276`, `app/ui/playlist.rs:606`

Prevents re-triggering:

```rust
// In effect handler — only run if not already inserted
if target.album_tracks.is_none() { ... }

// In insert_album_tracks — prevent double insert
if existing_tracks >= tracks.len() { return; }
```

## Original Entry Removal

After all tracks are ready (Arc shared + all `Downloaded`), the original full-album entry is removed from the queue. Two paths:

**Path A (validation first):** `handle_song_downloaded` → remove original → play track 1.

**Path B (download first):** `insert_album_tracks` → share Arc → remove original → play track 1.

## Decode With Offsets

File: `app/server/player.rs:112` — `DecodeSong`

When both offset AND actual_duration are `Some`, ffmpeg extracts with:

```
ffmpeg -ss {offset} -t {actual_duration} -i {full_audio} {output_file}
```

Each track gets its own decoded file of exact length for seamless seeking.

## Progress Display

File: `app/ui/playlist.rs:2000` — `handle_set_song_play_progress`

```rust
if song.track_no.is_some() {
    // Track from split album — d is already track-relative
    progress = d;
} else if let Some(offset) = song.start_offset {
    // Non-album entry with offset
    progress = d.saturating_sub(offset);
}
progress = progress.min(song.actual_duration.unwrap_or(progress));
```
