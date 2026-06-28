# CLI Tools

This document covers all CLI entry points in the workspace.

## Table of Contents

- [youtui (main binary)](#youtui-main-binary)
  - [YT Music API commands](#yt-music-api-commands)
  - [test-validate-metadata](#test-validate-metadata)
  - [test-scrobble](#test-scrobble)
- [genius-rs](#genius-rs)
- [lrclib-rs](#lrclib-rs)
- [External tools](#external-tools)

---

## youtui (main binary)

**Source**: `youtui/src/main.rs` (673 lines)
**Binary**: `target/release/youtui`

50+ API subcommands. All use your saved config auth (cookie/oauth).

### Basic usage

```bash
# Search all
youtui search "orchid"

# Search by category
youtui search-songs "combat wounded veteran"
youtui search-albums "combat wounded veteran"
youtui search-artists "the blood brothers"
youtui search-playlists "screamo"

# Get details
youtui get-artist <channel_id>
youtui get-album <browse_id>
youtui get-playlist-tracks <playlist_id>

# Library
youtui get-library-songs
youtui get-library-playlists
youtui get-library-artists

# Rate
youtui rate-song <video_id> LIKE
youtui rate-song <video_id> DISLIKE
youtui rate-song <video_id> INDIFFERENT

# Playlist CRUD
youtui create-playlist "my screamo mix"
youtui delete-playlist <playlist_id>
youtui add-videos-to-playlist <playlist_id> <video_id1> <video_id2>

# History
youtui get-history
```

### test-validate-metadata

Tests the metadata pipeline (Last.fm, Discogs, MusicBrainz, etc.) without running the UI.

```bash
youtui test-validate-metadata "combat wounded veteran" "53. Folded Space - Lead Poisoning & Distortion"
```

Output example (metadata resolved by Last.fm + MusicBrainz):

```
Resolving: combat wounded veteran - 53. Folded Space - Lead Poisoning & Distortion
--- RESULT ---
Artist:    Some("Combat Wounded Veteran")
Album:     Some("I Know A Girl Who Develops Crime Scene Photos")
Year:      Some(1999)
Track no:  Some(1)
Tracks:    21
Genres:    ["Rock"]
Styles:    ["Hardcore", "Emo", "Screamo"]
  1. 53. Folded Space - Lead Poisoning & Distortion (79s) Some("Combat Wounded Veteran")
  2. People That Can't Be Replaced (58s) Some("Combat Wounded Veteran")
  ... (21 tracks total)
```

Uses config's `api_key` (Last.fm), `discogs_token`, `genius_token`. 120s overall timeout, 20s per-provider timeout. Rejects providers that hang.

### test-scrobble

Submits a scrobble to Last.fm using your config credentials.

```bash
youtui test-scrobble --artist "Orchid" --track "Chaos Is Me" --album "Chaos Is Me" --duration 180
```

Output:

```
ARTIST=Orchid
TRACK=Chaos Is Me
ALBUM=Some("Chaos Is Me")
DURATION=180s
API_KEY=xxx
API_SECRET_PRESENT=true
SESSION_KEY=xxx
--- Sending scrobble request ---
RESULT=OK (scrobble accepted)
```

### setup-oauth

Interactive OAuth token generation for YouTube Music.

```bash
youtui setup-oauth <client_id> <client_secret>
```

---

## genius-rs

**Source**: `libs/genius-rs/src/main.rs` (243 lines)
**Binary**: `target/release/genius-rs`
**Dep**: `GENIUS_TOKEN` env var (set in config)

Fetches lyrics and annotations from Genius.

### Commands

```bash
# Search for song
GENIUS_TOKEN=xxx genius-rs search "the blood brothers" "I`m a monster"
# -> Found: The Blood Brothers - I'm A Monster (id=18713394)
# ->   Path: /The-blood-brothers-im-a-monster-lyrics

# Fetch lyrics
GENIUS_TOKEN=xxx genius-rs fetch "the blood brothers" "set fire to the face on fire"
# -> --- The Blood Brothers - Set Fire To The Face On Fire (id=1407154) ---
# -> (lyrics text)

# Fetch annotations (line-by-line explanations)
GENIUS_TOKEN=xxx genius-rs annotations "the blood brothers" "set fire to the face on fire"

# Fetch both
GENIUS_TOKEN=xxx genius-rs all "the blood brothers" "set fire to the face on fire"

# Compute slug URL (no network)
genius-rs slug "orchid" "chaos is me"
# -> /Orchid-chaos-is-me-lyrics

# JSON output
GENIUS_TOKEN=xxx genius-rs --json search "the blood brothers" "I`m a monster"
```

### Options

| Flag | Description |
|------|-------------|
| `--json` | Machine-readable JSON |
| `--verbose` | Debug logs |
| `--fixture <dir>` | Save raw HTML + parsed output to dir |
| `--raw-html` | Print raw page HTML |

### Notes

- Screamo/emo bands like The Blood Brothers, Orchid are well-covered on Genius
- Very underground/skramz may return "No results" (try lrclib-rs instead)
- `annotations` may return empty for bands with low annotation count

---

## lrclib-rs

**Source**: `libs/lrclib-rs/src/main.rs` (199 lines)
**Binary**: `target/release/lrclib-rs`

Fetches lyrics from LRCLIB (community-maintained, better coverage for underground).

### Commands

```bash
# Fetch lyrics by artist + title
lrclib-rs fetch "combat wounded veteran" "53. Folded Space - Lead Poisoning & Distortion"
# -> --- combat wounded veteran - 53. Folded Space - Lead Poisoning & Distortion ---
# -> (lyrics text)

# Fetch with album hint
lrclib-rs fetch "combat wounded veteran" "53. Folded Space - Lead Poisoning & Distortion" "I Know A Girl Who Develops Crime Scene Photos"

# Search
lrclib-rs search "combat wounded veteran"
# -> 1. Combat Wounded Veteran - 53. Folded Space - Lead Poisoning ...
# -> 2. Orchid - Eye Gouger [Split Combat Wounded Veteran & Orchid]
# -> 3. Combat Wounded Veteran - 67. Activate the Corpses [Duck Down for the Torso]

lrclib-rs search "reversal of man"
# -> 1. Reversal of Man - Bless the Printing Press [This Is Medicine]
# -> 2. Reversal of Man - Enoch Ardon [This Is Medicine]
# -> 3. Reversal of Man - Hills Have Eyes [This Is Medicine]
# -> ...

# Raw API response
lrclib-rs raw "combat wounded veteran" "53. Folded Space - Lead Poisoning & Distortion"
# -> Full JSON: plainLyrics, syncedLyrics, artistName, trackName, albumName, duration
```

### Flags

| Flag | Description |
|------|-------------|
| `--json` | JSON output |
| `--synced` | Show synced lyrics (timestamped) |
| `--all` | Show all search results |

### Notes

- Best option for underground/skramz/emoviolence/sasscore/cybergrind
- No auth token needed
- Returns both plain and synced lyrics when available

---

## External tools

### lyr (cargo install)

Multi-source lyrics fetcher (`cargo install lyr`).

```bash
# Search + fetch
lyr -s "the blood brothers" "set fire to the face on fire"

# Output lyrics only
lyr -s "orchid" "chaos is me"
```

Sources: Genius, LRCLIB, Musixmatch, AZLyrics, etc. Falls back between providers automatically.

### bandcamp-lyrics (cargo install)

Fetches lyrics from Bandcamp track pages.

```bash
bandcamp-lyrics search "combat wounded veteran" "folded space"
# or by URL:
bandcamp-lyrics https://combatwoundedveteran.bandcamp.com/track/folded-space
```

Works best for bands that host on Bandcamp (common for screamo/skramz).

### yt-dlp

Required runtime dep. Used for audio download and URL extraction.

```bash
yt-dlp --extract-audio --audio-format best <youtube-url>
```

### ffmpeg

Required runtime dep. Audio transcoding for yt-dlp downloads.

```bash
ffmpeg -i input.mp3 output.ogg
```

---

## Common workflows

### Debug metadata for a URL-added song

```bash
youtui test-validate-metadata "combat wounded veteran" "53. Folded Space - Lead Poisoning & Distortion"
```

### Find lyrics for an underground screamo track

```bash
# Try lrclib first (best coverage)
lrclib-rs fetch "reversal of man" "bless the printing press"

# Fallback to genius
GENIUS_TOKEN=xxx genius-rs fetch "the blood brothers" "set fire to the face on fire"

# Fallback to lyr
lyr -s "combat wounded veteran" "activate the corpses"
```

### Search YT Music from terminal

```bash
youtui search-songs "orchid" --show-source    # Show raw API JSON
youtui search-albums "reversal of man"         # Search albums only
youtui get-artist <channel_id>                 # Get artist details
```
