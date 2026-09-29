# Subsystem: Audio

## Download Pipeline

File: `youtui/src/app/server/song_downloader.rs` + `youtui/src/app/server/messages.rs`

### yt-dlp (default)

```rust
yt-dlp --dump-json --no-warnings {url} ← metadata fetch (add_yt_video, async FetchYtVideoMetadata backend task, 60s timeout, optimistic pending row replaced on HandleYtVideoMetadataOk, removed on HandleYtVideoMetadataError; full JSON keeps title/uploader/duration/thumbnail/channel fields)
yt-dlp -f bestaudio/best --cookies {cookie.txt} -o {tempfile} -- {video_id} ← audio download (`--` end-of-options guard so dash-leading ids never parse as flags)
```

Bandcamp URLs flow through the same pipeline: `yt_dlp_target_arg()` passes a
bandcamp URL verbatim to yt-dlp (anything else is wrapped as `https://youtu.be/`).
Track URLs queue like YouTube videos (`add_yt_video`, `VideoID` holds the full
normalized URL); album/discography URLs resolve to their track list via the
`FetchBandcampAlbumEntries` backend task (`--flat-playlist --dump-json`, 60s
timeout) and each entry queues individually. `bandcamp-resolve <url>` is the CLI
debug tool exercising the same resolution off the TUI.

**Bandcamp requirement:** yt-dlp must be the uv-tool install with `curl_cffi`
(`~/.local/bin/yt-dlp`, from `uv tool install yt-dlp`). The distro
`/usr/bin/yt-dlp` fails on the 2026 Bandcamp Client Challenge ("Unable to extract
tralbum data"). Free streams are mp3-128 only; stream tokens expire in minutes
and are never cached.

## Bandcamp Search Merge (Phase 2)

Songs-tab F1 search and the `:` command fallback run a merged search:
`SearchSongs` (YouTube) and `SearchBandcamp` (Bandcamp) dispatch concurrently
(`AsyncTask::push` -> Multi); bandcamp rows append after the YouTube results
(deterministic via the `search_pending`/`pending_bandcamp` gate on
`SongSearchBrowser`). Bandcamp rows carry a `BC` badge in the `Src` column
(`source_badge_for_song`, `is_bandcamp_url` on `video_id`). The bandcamp
`video_id` IS the track URL, so playing a merged row flows through the exact
`add_yt_video` path from Phase 1.

`SearchBandcamp` POSTs to
`https://bandcamp.com/api/bcsearch_public_api/1/autocomplete_elastic`
(body `{"fan_id":null,"full_page":false,"search_filter":"t","search_text":q}`,
Firefox UA + `https://bandcamp.com/search` Referer; no Client Challenge on this
endpoint). One defensive 3s sleep retry on 429 (onetagger pattern). Pure fn
`parse_bandcamp_search_results(json)` maps `auto.results[]` track rows to
`SearchResultSong` (title, `band_name` artist, `album_name` album, URL as
video_id), filtering out non-track rows. SPIKE evidence:
`.omo/evidence/bandcamp-search-spike.md`.

**Key flags:**
- `--force-overwrites` - prevents yt-dlp resume from treating 0-byte temp files as complete
- `--extractor-args youtube:player_client=web_creator` - only with cookie_path
- `--cookies {cookie.txt}` - exported cookie file when present (legacy `--cookies-from-browser chromium` fallback otherwise)
- Writes to tempfile via `tempfile::Builder::new().suffix(".m4a")`

**Timeout:** 5-minute proc wait prevents hung processes.

**Container validation:** Post-download `detect_container()` checks for valid audio header:
- MP4: `ftyp` magic bytes
- M4A: M4A brand in ftyp
- WebM: `\x1a\x45\xdf\xa3` (EBML)
- WAV: `RIFF`
- Ogg: `OggS`
- MP3: `ID3` tag or MPEG frame sync (`0xFF` + 3-bit version/algo bits) - needed for Bandcamp streams

### Native (rusty_ytdl, broken)

File: `youtui/src/youtube_downloader/native.rs`

Uses `rusty_ytdl::stream()` but ignores custom filter for some videos, downloads video-only MPEG-4. Workaround: use `:` command with yt-dlp.

## Decode + Playback

File: `youtui/src/app/server/player.rs` + `libs/audio-player/`

### DecodeSong

```rust
struct DecodeSong(
    Arc<InMemSong>,         // Song data (audio bytes)
    Option<Duration>,       // Start offset (for album tracks)
    Option<Duration>,       // Actual duration (for album tracks)
);
```

Three cases:

| offset | actual_duration | Behavior |
|--------|----------------|----------|
| `Some(o)` | `Some(d)` | ffmpeg: `-ss {o} -t {d}` → exact section |
| `Some(o)` | `None` | ffmpeg: `-ss {o}` → from offset to end |
| `None` | `None` | use full audio, no extraction |

### Decoded file naming

```
format: "{video_id}_{offset_ms}_{duration_ms}.m4a"
example: "abc123_0_240000.m4a"
```

Files cached in temp dir. Cleanup on youtui exit via `create_or_clean_directory`.

### Player backend

### audio-player crate

`libs/audio-player/` wraps `rodio` + `symphonia` for audio playback:
- `Sink::new()` - create playback sink
- `Sink::append(source)` - queue audio
- `Sink::seek(duration, direction)` - seek (Forward/Back)
- `Sink::stop()` - stop playback
- `Sink::current_position()` - query position

Supported codecs: MP4/AAC, WebM/Opus, WAV, Ogg/Vorbis (via symphonia codecs).

### Gapless Auto-Advance

File: `youtui/src/app/ui/playlist.rs` (inline, in queue management)

```
Gapless threshold: 1s before track end
On progress update:
  if track-relative progress >= actual_duration - 1s:
    → spawn DecodeSong for next track
    → queue next track in audio sink
    → seamless transition
```

### Progress Tracking

```
On every progress update (~10Hz):
  track entries (track_no.is_some()):
    → use d directly (ffmpeg already extracted section)
  non-album entries with offset:
    → d.saturating_sub(offset)
  capped at actual_duration
```

## MPRIS Media Controls

File: `youtui/src/app/media_controls.rs`

Uses `souvlaki` crate for MPRIS integration:
- Play/Pause, Next/Previous, Seek, Volume
- Metadata: title, artist, album, cover art URL
- Playback status: Playing/Paused/Stopped
