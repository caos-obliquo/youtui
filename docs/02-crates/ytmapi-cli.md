# ytmapi-cli

YouTube Music API debug CLI - search, playlist, album, artist, library, fixture.

## Purpose

- Debug YTM API responses without launching youtui
- Parse saved JSON fixture files offline (no auth needed)
- Run live queries against YTM API (requires cookie auth)
- Generate test fixtures by saving API responses + parsed output

## Dependencies

- `ytmapi-rs` - core API library (simplified-queries feature)
- `tokio` - async runtime
- `serde_json` - JSON handling

## Auth Setup

Live queries require YouTube Music authentication via browser cookies.

```bash
# 1. Install a cookies.txt export extension for your browser:
#    Chrome: "Get cookies.txt" by Rehan Ahmad
#    Firefox: "cookies.txt" by Shrimp

# 2. Export cookies for https://music.youtube.com

# 3. Run with --cookie flag:
ytmapi-cli search "Beatles" --cookie ~/Downloads/cookies.txt

# Or set env var (recommended):
export YTMAPI_COOKIE=~/Downloads/cookies.txt
ytmapi-cli search "FIDLAR"
```

### How cookies work

The cookie file contains a `SAPISID` value that ytmapi-rs uses to sign API requests via `YtMusic::from_cookie_file()`. This gives `YtMusic<BrowserToken>` which satisfies both `AuthToken` and `LoggedIn` - all YTM API queries work.

**Cookie expiry**: YouTube Music cookies are long-lived (months). Re-export if you get auth errors.

## CLI Usage

### Commands

| Command | Arguments | Auth | Description |
|---------|-----------|------|-------------|
| `search` | `<query>` | Required | Search songs matching query |
| `search-artists` | `<query>` | Required | Search artists matching query |
| `search-albums` | `<query>` | Required | Search albums matching query |
| `playlist` | `<id>` | Required | Get all tracks in a playlist |
| `album` | `<id>` | Required | Get album details + track list |
| `artist` | `<channel_id>` | Required | Get artist info + songs |
| `library` | `playlists` or `songs` | Required | List library items |
| `fixture` | `<file> [--type ...]` | None | Parse offline JSON fixture |

### Options

| Flag | Description |
|------|-------------|
| `--cookie <file>` | Path to cookies.txt file (or `YTMAPI_COOKIE` env var) |
| `--json` | Machine-readable JSON output |

## Examples

### Live queries (requires auth)

```bash
# Search songs
ytmapi-cli search "FIDLAR" --cookie cookies.txt

# Search with JSON output for piping
ytmapi-cli search "Beatles" --cookie cookies.txt --json

# Get playlist tracks
ytmapi-cli playlist "PL1Q2uZ1WIhIdj477HZMLHG_crU28UyQdR" --cookie cookies.txt

# Get album details
ytmapi-cli album "MPREb_pyQa1mky9hE" --cookie cookies.txt

# Get artist with songs
ytmapi-cli artist "UCfP6GqHv9J_dVnGvLHIa1xQ" --cookie cookies.txt

# List library playlists
ytmapi-cli library playlists --cookie cookies.txt

# List library songs
ytmapi-cli library songs --cookie cookies.txt
```

### Fixture mode (offline, no auth)

```bash
# Parse a search fixture
ytmapi-cli fixture ytmapi-rs/test_json/search_songs_20231226.json --type search

# Parse a playlist fixture
ytmapi-cli fixture ytmapi-rs/test_json/get_playlist_tracks_20250604_output.txt --type playlist

# Parse an album fixture
ytmapi-cli fixture ytmapi-rs/test_json/get_album_20240724_output.txt --type album
```

### Using YTMAPI_COOKIE env var

```bash
# Set once in your shell profile
echo 'export YTMAPI_COOKIE=~/Downloads/cookies.txt' >> ~/.zshrc
source ~/.zshrc

# Then use without --cookie flag
ytmapi-cli search "FIDLAR"
ytmapi-cli playlist "PL..."
ytmapi-cli album "MPRE..."
```

## Architecture

```rust
// Auth flow:
let yt = YtMusic::from_cookie_file("cookies.txt").await?;

// Query flow (simplified methods):
let songs = yt.search_songs("Beatles").await?;
let tracks = yt.get_playlist_tracks(id).await?;
let album = yt.get_album(id).await?;
let artist = yt.get_artist(id).await?;
let playlists = yt.get_library_playlists().await?;

// Fixture flow (offline parsing):
let json = std::fs::read_to_string("fixture.json")?;
let query = GetPlaylistTracksQuery::new(PlaylistID::from_raw(""));
let result = process_json::<_, BrowserToken>(json, &query)?;
println!("{:#?}", result);
```

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `Error loading cookie file` | File not found or invalid format | Re-export cookies from browser, ensure SAPISID is present |
| `Search error: ...` | API returned error or no auth | Check cookie validity, ensure query is not empty |
| `Library error: ...` | Library queries require logged-in auth | Ensure cookie has valid SAPISID for music.youtube.com |
| Fixture parse error | File format doesn't match query type | Use correct `--type` flag (search/playlist/album) |

## Tests

```bash
# 7 tests pass
cargo test --release -p ytmapi-cli
```
