# Subsystem: Audio

## Download Pipeline

File: `app/server/song_downloader.rs` + `app/server/messages.rs`

### yt-dlp (default)

```rust
yt-dlp --print-json {url}              ← metadata fetch (add_yt_video)
yt-dlp -f bestaudio -o {tempfile} {url} ← audio download
```

**Key flags:**
- `--force-overwrites` — prevents yt-dlp resume from treating 0-byte temp files as complete
- `--extractor-args youtube:player_client=web_creator` — only with cookie_path
- `--cookies-from-browser chromium` — when cookie path configured
- Writes to tempfile via `tempfile::Builder::new().suffix(".m4a")`

**Timeout:** 5-minute proc wait prevents hung processes.

**Container validation:** Post-download checks for valid audio header:
- MP4: `ftyp` magic bytes
- M4A: M4A brand in ftyp
- WebM: `\x1a\x45\xdf\xa3` (EBML)
- WAV: `RIFF`
- Ogg: `OggS`

### Native (rusty_ytdl, broken)

File: `app/youtube_downloader/native.rs`

Uses `rusty_ytdl::stream()` but ignores custom filter for some videos, downloads video-only MPEG-4. Workaround: use `:` command with yt-dlp.

## Decode + Playback

File: `app/server/player.rs` + `app/async_rodio_sink.rs`

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

`async_rodio_sink.rs` wraps `rodio` + `symphonia` for audio playback:
- `Sink::new()` — create playback sink
- `Sink::append(source)` — queue audio
- `Sink::seek(duration, direction)` — seek (Forward/Back)
- `Sink::stop()` — stop playback
- `Sink::current_position()` — query position

Supported codecs: MP4/AAC, WebM/Opus, WAV, Ogg/Vorbis (via symphonia codecs).

### Gapless Auto-Advance

File: `app/ui/playlist.rs` (inline, in queue management)

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

File: `app/media_controls.rs`

Uses `souvlaki` crate for MPRIS integration:
- Play/Pause, Next/Previous, Seek, Volume
- Metadata: title, artist, album, cover art URL
- Playback status: Playing/Paused/Stopped
