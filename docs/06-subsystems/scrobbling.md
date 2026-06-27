# Subsystem: Scrobbling

## Protocol

File: `youtui/src/app/scrobbler.rs`

Implements the [Last.fm scrobbling protocol](https://www.last.fm/api/scrobbling) - compatible with Libre.fm, GNU FM, and Last.fm itself.

## Configuration

```toml
[scrobbling]
enabled = true
api_key = "your_lastfm_api_key"
api_secret = "your_lastfm_secret"
session_key = "your_lastfm_session_key"
```

## Scrobble Flow

```
1. Song starts playing -> "now playing" notification sent
   POST /2.0/?method=track.updateNowPlaying&...
   Params sorted alphabetically before HMAC signing (Last.fm requirement)

2. Song plays for >30s OR >50% duration -> scrobble submitted
   POST /2.0/?method=track.scrobble&...
   album param sent only if album name is available

3. Progress checked at ~10Hz:
   handle_set_song_play_progress -> should_scrobble() -> true -> submit
```

## Scrobble State

File: `app/ui/playlist.rs:1511-1522`

Each song creates a `ScrobbleState`:

```rust
struct ScrobbleState {
    artist: String,
    track: String,
    album: Option<String>,
    duration: Duration,
    started_at: Instant,
    scrobbled: bool,
}
```

Condition: `self.album_tracks.is_none() || song.track_no.is_some()` - track entries scrobble individually (not the full-album entry). Album boundary scrobbler (`playlist.rs`, progress handler) only runs for non-split tracks (`!is_album_track`). Split tracks scrobble individually via ScrobbleState.

## Persistent Scrobble

Unlike scrobbling only at song change, youtui checks `should_scrobble()` on every progress update (~10Hz). This ensures scrobbles work in any context: lyrics popup, browser, playlist - not just when the queue view is focused.

## Failed Scrobble Cache

File: `~/.config/youtui/scrobble_cache.json`

> **Cross-Platform:** Path resolved via `directories` crate (`ProjectDirs::config_local_dir()`) - `~/.config/youtui/` on Linux, `~/Library/Application Support/com.nick42.youtui/` on macOS. Temp files use `std::env::temp_dir()` for platform-correct temp directory.

Failed scrobbles are persisted to disk and retried:

- `save_failed_scrobble()` - writes failed submission to JSON array with `retry_count` field
- `retry_failed_scrobbles()` - called on startup + every 5 min in background loop
- `remove_cached_scrobble()` - removes entry after successful retry (`#[allow(dead_code)]`, tests only)
- Max retries: 3 per entry (dropped after 3 failures)
- Max cache size: 200 entries (oldest evicted)
- `ScrobbleResult` enum: `Success`, `Failure(String)`, `RateLimited`
- Rate limit (error 29) stops retry loop to avoid hammering Last.fm API
- 2-second delay between retries

## Album Mode Scrobbling

Album-split tracks scrobble individually via `ScrobbleState` created in `play_song_id()` and `autoplay_song_id()`. Boundary checker (progress handler) only runs on non-split original entry. Album name uses canonical Last.fm name resolved via `album.getInfo` in `FetchAlbumArt` pipeline.

## Autoplay Scrobble (v1.0.3)

`autoplay_song_id()` now mirrors `play_song_id()` scrobble setup: resets `scrobble_pending`, submits previous pending scrobble, creates `ScrobbleState`, sends `submit_now_playing`, and triggers `FetchAlbumArt` for tracks without art. Previously autoplay had zero scrobble setup - tracks auto-advanced after the first never scrobbled.

## Signature Requirement

Last.fm API requires all POST params sorted alphabetically BEFORE HMAC signing:
```rust
params.sort_by(|a, b| a.0.cmp(&b.0));
```
This fixed error 13 (invalid signature).

## Rate Limit Handling

Error 29 (rate limit exceeded) handled via:
- `ScrobbleResult::RateLimited` stops the retry loop
- Next startup retries cached scrobbles
- Background 5-min retry loop continues retrying until cleared

## Known Issues

- Session key management is manual (obtained via OAuth flow outside youtui)
- Rescrobbled systemd service double-submits scrobbles if running alongside embedded scrobbler
