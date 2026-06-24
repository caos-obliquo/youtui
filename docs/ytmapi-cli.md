# ytmapi-cli — YouTube Music API CLI Debug Tool

## Overview

ytmapi-cli is a CLI debug tool for ytmapi-rs. It exposes all 50+ ytmapi-rs endpoints as CLI commands. Located at `libs/ytmapi-cli/`.

**Source**: `libs/ytmapi-cli/src/main.rs` (1426 lines)
**Deps**: ytmapi-rs, tokio, serde_json, tracing, metadata-provider, reqwest, genius-rs

## Build & Run

```bash
cargo run --release -p ytmapi-cli -- [options] <command> [args...]
```

From ytmapi-cli directory:
```bash
cargo run --release -- [options] <command> [args...]
```

## Authentication

Most commands need YT Music auth cookie. Two ways:
1. `--cookie <file>` flag
2. `YTMAPI_COOKIE` env var

Get cookie: Export `music.youtube.com` cookies via browser extension (Get cookies.txt).

## Global Options

| Option | Description |
|--------|-------------|
| `--cookie <file>` | Cookie file for auth (or YTMAPI_COOKIE env) |
| `--json` | Machine-readable JSON output |

## Commands

### SEARCH

| Command | Args | Description |
|---------|------|-------------|
| `search <query>` | query string | Search songs |
| `search-artists <query>` | query string | Search artists |
| `search-albums <query>` | query string | Search albums |
| `search-playlists <query>` | query string | Search playlists |
| `search-videos <query>` | query string | Search videos |
| `search-community-playlists <query>` | query string | Search community playlists |
| `search-featured-playlists <query>` | query string | Search featured playlists |
| `search-episodes <query>` | query string | Search episodes |
| `search-podcasts <query>` | query string | Search podcasts |
| `search-profiles <query>` | query string | Search profiles |
| `search-suggestions <query>` | query string | Get search autocomplete suggestions |

### PLAYLIST

| Command | Args | Description |
|---------|------|-------------|
| `playlist <id>` | playlist_id | Get playlist tracks |
| `playlist-details <id>` | playlist_id | Get playlist metadata |
| `playlist-songs <id>` | playlist_id | Get playlist tracks (streaming debug) |
| `create-playlist <title>` | title [--description <d>] [--privacy private|public|unlisted] | Create new playlist |
| `delete-playlist <id>` | playlist_id | Delete a playlist |
| `edit-playlist <id>` | playlist_id [--title <t>] [--description <d>] [--privacy <p>] | Edit playlist metadata |
| `rate-playlist <id> <rating>` | playlist_id, like/indifferent/dislike | Rate a playlist |
| `remove-items <id> <vid...>` | playlist_id, video_id(s) | Remove items from playlist |
| `add-to-playlist <id> <vid...>` | playlist_id, video_id(s) | Add videos to playlist |
| `merge-playlist <dest> <src>` | dest_playlist_id, src_playlist_id | Merge src playlist tracks into dest |

### ALBUM / ARTIST / SONG

| Command | Args | Description |
|---------|------|-------------|
| `album <id>` | album_id/browse_id | Get album details + tracks |
| `artist <channel_id>` | channel_id | Get artist profile + discography |
| `artist-albums <channel_id>` | channel_id [--params <browse_params>] | Get artist album list |
| `subscribe <channel_id>` | channel_id | Subscribe to artist |
| `unsubscribe <channel_id>...` | channel_id(s) | Unsubscribe from artist(s) |
| `rate-song <video_id> <rating>` | video_id, like/indifferent/dislike | Rate a song |
| `lyrics <video_id>` | video_id | Get lyrics for a song |
| `tracking-url <video_id>` | video_id | Get song tracking URL |
| `watch-playlist <video_id>` | video_id | Get related/watch playlist |

### LIBRARY

| Command | Args | Description |
|---------|------|-------------|
| `library playlists` | - | List library playlists |
| `library songs` | - | List library songs |
| `library albums` | - | List library albums |
| `library artists` | - | List library artists |
| `library artist-subscriptions` | - | List subscribed artists |
| `library podcasts` | - | List library podcasts |
| `library channels` | - | List library channels |
| `library upload-songs` | - | List uploaded songs |
| `library upload-artists` | - | List upload artists |
| `library upload-albums` | - | List upload albums |
| `library upload-album <id>` | upload_album_id | Get upload album details |
| `library upload-artist <id>` | upload_artist_id | Get upload artist songs |
| `library upload <file>` | file_path | Upload a song file to library |
| `delete-upload <entity_id>` | upload_entity_id | Delete uploaded entity |

### HISTORY

| Command | Args | Description |
|---------|------|-------------|
| `history` | - | Get listening history |
| `remove-history <token...>` | feedback_token(s) | Remove items from history |

### RECOMMENDATIONS

| Command | Args | Description |
|---------|------|-------------|
| `taste-profile` | - | Get taste profile artists |
| `set-taste-profile <tokens...>` | taste_token(s) | Set taste profile |
| `mood-categories` | - | Get mood/genre categories |
| `mood-playlists <params>` | mood_params | Get playlists for mood |

### PODCASTS

| Command | Args | Description |
|---------|------|-------------|
| `channel <channel_id>` | channel_id | Get podcast channel details |
| `channel-episodes <channel_id>` | channel_id | Get channel episodes |
| `podcast <podcast_id>` | podcast_id | Get podcast details |
| `episode <episode_id>` | episode_id | Get episode details |
| `new-episodes` | - | Get new episodes |

### USER

| Command | Args | Description |
|---------|------|-------------|
| `user <channel_id>` | user_channel_id | Get user profile |
| `user-videos <channel_id>` | user_channel_id | Get user uploaded videos |
| `user-playlists <channel_id>` | user_channel_id | Get user playlists |

### FIXTURE (offline)

| Command | Args | Description |
|---------|------|-------------|
| `fixture <file>` | file [--type search|playlist|album] | Parse saved API JSON with appropriate parser |

### DEBUG (offline, metadata-provider integration)

| Command | Args | Description |
|---------|------|-------------|
| `debug meta <title>` | title [artist] | Test title cleaning + artist normalization |
| `debug clean <title>` | title | Test title cleaning only |
| `debug artist <name>` | name | Test artist normalization only |
| `debug resolve <artist> <title>` | artist, title | Test full metadata resolution pipeline |
| `debug genre <genre>` | genre | Test genre normalization |
| `debug genre-list [filter]` | [filter] | List known genres with optional filter |

### GENIUS (no auth needed, uses GENIUS_TOKEN env)

| Command | Args | Description |
|---------|------|-------------|
| `genius search <artist> <title>` | artist, title | Search Genius for song |
| `genius annotations <artist> <title>` | artist, title | Fetch song annotations |
| `genius lyrics <artist> <title>` | artist, title | Fetch song lyrics |
| `genius all <artist> <title>` | artist, title | Fetch lyrics + annotations |

## Architecture

The CLI follows a simple pattern:

1. Parse global options (--cookie, --json)
2. Route command to handler
3. Auth commands construct `YtMusic::from_cookie_file()` and call simplified query methods
4. No-auth commands (fixture, debug, genius) skip auth
5. Commands call either simplified methods (`yt.search_songs()`) or direct queries (`yt.query(GetArtistAlbumsQuery::new(...))`)
6. Results printed via generic `print_results()` (human-readable or JSON)

### Command Categories

```
command routing:
  fixture|debug|genius  -> no auth needed
  search-*|playlist|album|artist|rate-song|lyrics|tracking-url|watch-playlist
    -> uses YtMusic<A> methods (any AuthToken)
  subscribe|unsubscribe|rate-playlist|create-playlist|delete-playlist|edit-playlist
    |remove-items|add-to-playlist|merge-playlist|library*|history*|delete-upload
    -> uses YtMusic<A: LoggedIn> methods (needs auth)
  taste-profile|set-taste-profile|mood-categories|mood-playlists
    -> uses YtMusic<A> methods (no auth needed for recommendations)
  user*|channel*|podcast|episode|new-episodes
    -> uses YtMusic<A> methods (no auth needed for public data)
```

## ytmapi-rs Endpoint Coverage

All 16 unique Innertube API paths covered:
- `search` - all 10 search filters + basic search + upload search + library search
- `browse` - artist, album, playlist, library, podcast, user, taste profile, mood
- `next` - watch playlist, lyrics ID
- `player` - song tracking URL
- `feedback` - remove history, edit library status
- `like/like|dislike|removelike` - rate song, rate playlist
- `playlist/create|delete|edit` - playlist CRUD
- `browse/edit_playlist` - add/remove playlist items
- `music/get_search_suggestions` - autocomplete
- `music/delete_privately_owned_entity` - delete uploads
- `subscription/subscribe|unsubscribe` - artist subscriptions

## Test Status

```bash
cargo test --release -p ytmapi-cli  # 7/7 pass, 0 warnings
```
