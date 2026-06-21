# Subsystem: Scrobbling

## Protocol

File: `app/scrobbler.rs`

Implements the [Last.fm scrobbling protocol](https://www.last.fm/api/scrobbling) — compatible with Libre.fm, GNU FM, and Last.fm itself.

## Configuration

```toml
[scrobbling]
enabled = true
api_url = "https://libre.fm"    # Last.fm: "https://ws.audioscrobbler.com"
api_key = "your_api_key"
api_secret = "your_api_secret"
session_key = "your_session_key"
```

## Scrobble Flow

```rust
1. Song starts playing → "now playing" notification sent
   POST /2.0/?method=track.updateNowPlaying&...
   
2. Song plays for >30s OR >50% duration → scrobble submitted
   POST /2.0/?method=track.scrobble&...
   
3. Progress checked at ~10Hz:
   handle_set_song_play_progress → should_scrobble() → true → submit
```

## Scrobble State

File: `app/ui/playlist.rs:818,2009`

Each song creates a `ScrobbleState`:

```rust
struct ScrobbleState {
    song_id: ListSongID,
    started_at: Instant,
    submitted: bool,
}
```

Condition: `self.album_tracks.is_none() || song.track_no.is_some()` — track entries scrobble individually (not the full-album entry).

## Persistent Scrobble

Unlike scrobbling only at song change, youtui checks `should_scrobble()` on every progress update (~10Hz). This ensures scrobbles work in any context: lyrics popup, browser, playlist — not just when the queue view is focused.

## Rescrobbled Integration

File: `app.rs:225`

Spawning the external `rescrobbled` daemon for Libre.fm scrobbling:

```rust
tokio::process::Command::new("rescrobbled")
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
```

Conditionally spawned based on `config.scrobbling.enabled`.

## Known Issues

- Session key management is manual (obtained via OAuth flow outside youtui)
- No retry on network failure — scrobble silently dropped if HTTP fails
