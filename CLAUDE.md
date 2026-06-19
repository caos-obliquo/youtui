# Youtui — Project Knowledge

## GOLDEN RULE
One feature at a time. Implement → test → commit → next. Never batch changes.
If things break, rollback and re-apply one-by-one.

## Build
- Workspace root: `/home/caos/builds/youtui/`
- Rust nightly (1.97.0)
- Binary: `cargo build --release` → `target/release/youtui`
- Dependencies: yt-dlp, ffmpeg, alsa-lib
- Tests: `cargo test --release -p youtui --bin youtui` (126 pass, 2 pre-existing config failures)

## Architecture

3-layer async callback:
```
Frontend (UI) → TaskManager → Backend (Server)
```

- **Frontend**: Ratatui TUI components in `app/ui/`. State mutations happen via `FrontendEffect` handlers.
- **Backend**: Server at `app/server/`. Handles downloads, API calls, audio decoding.
- **Tasks**: `ValidateMetadata`, `GetLyrics`, `DecodeSong`, etc. defined in `messages.rs` as `BackendTask` impls.
- **Effects**: Frontend reactions to backend results. `impl FrontendEffect<Playlist, ArcServer, TaskMetadata>`.

## Metadata Validation Pipeline

`messages.rs:730` — `ValidateMetadata::into_future`. Progressive search order:

```
1.  album.search(norm_for_lfm(clean_title))          # ← takes priority
    → album.getInfo → 17 tracks from Last.fm ✅
2.  track.getInfo(artist, clean_title)                # exact track match
3.  track.search(norm_for_lfm(clean_title))           # fuzzy track → re-fetch for album
4.  Discogs API (no auth, underground metal)          # `fetch_album_tracks` Phase 2
5.  Last.fm album.search fallback                     # `fetch_album_tracks` Phase 3
6.  MusicBrainz recording search                      # 1 req/s rate limit
```

### `norm_for_lfm` (`messages.rs:1074`)
Normalizes messy titles for database queries. Strips in order:
- `"FULL ALBUM"`, `"Full Album"`, `"full album"`, `"FULL LP"`, `"FULL EP"`, `"full-length album"`
- `" - Single"`, `" - EP"`, `" - LP"`, `" - full album"`
- Parenthesized blocks: ` (year - genre / genre2)`, ` (2000)`
- Bracketed blocks: ` [genre]`, ` [HD]`
- Replaces ` & ` → ` and ` for Last.fm compatibility

### `add_yt_video` title cleaning (`playlist.rs:735`)
Before `ValidateMetadata` is spawned, the raw yt-dlp title is cleaned:
1. Strip `"{artist} - "` prefix (case-insensitive)
2. Strip `"FULL ALBUM"` suffix (case-insensitive)
3. Strip `"  ("` suffix (parenthetical metadata)

### `fetch_album_tracks` (`messages.rs:926`)
Three-phase fallback for getting full tracklists:
- **Phase 1**: Last.fm `album.getInfo` (requires API key)
- **Phase 2**: Discogs API (no auth, works for underground extreme metal)
- **Phase 3**: Last.fm `album.search` → re-fetch `album.getInfo`

## Album Splitting

### Track creation
`insert_album_tracks` (`playlist.rs:575`): each track becomes a `ListSong` with:
- `track_no: Some(i+1)`, `start_offset: Some(accum)`, `actual_duration`
- `year` from validation result, falls back to original entry's yt-dlp year
- `duration_string`: `"M:SS"` format
- `download_status: None` initially → shared via `Arc::clone` from original when download completes

### Arc sharing
`handle_song_downloaded` (`playlist.rs:893`): when original album entry finishes downloading, its `Arc<InMemSong>` is cheaply cloned to all track entries with `DownloadStatus::None`.

### DecodeSong (`messages.rs` → `player.rs:112`)
Three fields: `(Arc<InMemSong>, Option<Duration> offset, Option<Duration> actual_duration)`.
- When both offset AND actual_duration are `Some`: ffmpeg extracts `-ss {offset} -t {actual_duration}` from the full audio → each track gets its own file of exact length.
- When only offset is `Some`: ffmpeg extracts from offset to end (no `-t` boundary).
- When offset is 0 and actual_duration is `None`: uses full audio without extraction.

### Progress (`handle_set_song_play_progress`, `playlist.rs:2000`)
- Track entries (`track_no.is_some()`): `d` is directly used as track-relative progress (ffmpeg already extracted the correct section).
- Non-album entries with offset: `d.saturating_sub(offset)`
- Progress is capped at `actual_duration` so display never exceeds track boundary.
- `>` key crash guard: `draw_media_controls.rs` checks `duration == 0` before division.

### Gapless auto-advance
- Gapless threshold: 1s before track end.
- When track-relative progress ≈ `actual_duration - 1s`, `QueueDecodedSong` queues next track.
- Next track is pre-decoded via `DecodeSong(next_arc, next_offset, next_actual_dur)`, played seamlessly.

### Scrobble fixes (`playlist.rs:818,2009`)
- Track entries scrobble individually: `self.album_tracks.is_none() || song.track_no.is_some()` creates `ScrobbleState` for each track.
- Persistent scrobble: `handle_set_song_play_progress` checks `should_scrobble()` on every progress update (~10Hz), not just at song change. Works in any context (lyrics, browser, playlist).

### Original entry removal
- After all tracks are ready (Arc shared + all `Downloaded`), the original full-album entry is removed from the playlist list. `Arc<InMemSong>` stays alive via track clones.
- Path A (validation first): `handle_song_downloaded` → remove original + play track 1.
- Path B (download first): `insert_album_tracks` → share Arc → remove original → play track 1.

### Cascade guard (`effect_handlers_playlist.rs:276`, `playlist.rs:606`)
- `target.album_tracks.is_none()` prevents per-track validation results from re-triggering `insert_album_tracks`.
- `existing_tracks >= tracks.len()` in `insert_album_tracks` itself prevents double-insertion.

### Album art from Last.fm
- `FetchAlbumArt(artist, album, api_key)` backend task queries `album.getInfo` → extracts largest image URL → downloads via `song_thumbnail_downloader`.
- `FetchAlbumArtEffect::Fetched` handler stores art on all playlist entries with matching `artist:album` string.
- Wired in `MetadataEffect::Validated` handler after album tracks are inserted.

## yt-dlp Integration

### Audio download pipeline
- Writes to temp file via `tempfile::Builder::new().suffix(".m4a")` (NOT `-o -` — stdout pipe produces corrupted data on yt-dlp 2026+ due to skipped FixupM4a post-processing).
- `--force-overwrites` prevents yt-dlp's resume feature from treating pre-existing 0-byte temp files as "complete" (writes nothing).
- `--extractor-args youtube:player_client=web_creator` only when cookie_path is configured (requires auth). Default extractor used otherwise.
- `--cookies-from-browser chromium` passed when cookie path configured.
- 5-minute timeout on `proc.wait()` prevents hung processes.
- Post-download validation checks for valid audio container header (ftyp/EBML/RIFF/OggS). Rejects garbage/empty with clear error.
- Logs detected container format (MP4 isom, M4A, WebM, WAV, Ogg).
- `add_yt_video` metadata fetch (`--dump-json`) passes `--cookies-from-browser chromium`.
- Cookie path flows: `main.rs → app.rs → YoutuiWindow::new → Playlist`.

## Lyrics Pipeline
Order: `Musixmatch` → `Genius scrape` (quality gate: reject < 50 chars or < 3 lines) → `Bandcamp URL construction` → `lyr CLI` → `error`

## Known Issues
- Native downloader (`rusty_ytdl::stream()`): ignores custom filter for some videos, downloads video-only MPEG-4. Workaround: `:` command uses yt-dlp (works).
- Metallum CLI integration blocked by Cloudflare (cf_clearance cookie + TLS fingerprint mismatch).
- 53 ytmapi-rs integration tests: 28 pass, 52 fail (missing browser auth + API format drift).

## Key Files

| File | Lines | Purpose |
|---|---|---|
| `app/server/messages.rs` | ~1420 | All backend tasks: ValidateMetadata, GetLyrics, DecodeSong, FetchAlbumArt |
| `app/server/player.rs` | ~190 | Audio decode + ffmpeg extraction (`try_decode`) |
| `app/ui/playlist.rs` | ~2200 | Main playlist: track management, playback, scrobbling |
| `app/ui/playlist/effect_handlers_playlist.rs` | ~390 | Frontend effect handlers for metadata/lyrics/album art results |
| `app/scrobbler.rs` | 70 | ScrobbleState + submit_scrobble to Last.fm |
| `app/ui/footer.rs` | ~210 | Current song display + progress bar |
| `app/ui/draw_media_controls.rs` | ~80 | MPRIS media controls integration |
| `app/structures.rs` | ~610 | ListSong, DownloadStatus, BrowserSongsList |
| `youtube_downloader/yt_dlp.rs` | ~210 | yt-dlp command builder |
| `config/keymap.rs` | | All keybindings by context |
| `main.rs` | ~600 | Entry point, cookie loading, app init |

## Tests
- `insert_album_tracks_sets_correct_metadata` — start_offset accumulation, duration_string, year fallback
- `album_download_shares_arc_with_tracks` — Arc sharing via handle_song_downloaded
- `play_song_id_uses_start_offset_in_decode` — DecodeSong includes offset in effect chain
- `progress_is_relative_to_start_offset` — handle_set_song_play_progress subtracts start_offset (album tracks use d directly)
- `non_album_progress_subtracts_offset` — non-album entries with start_offset still subtract
