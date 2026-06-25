# External API Services

youtui talks to several external APIs. Most work out of the box. Some need
API keys for extra features (metadata, lyrics, scrobbling).

## Required

### YouTube Music (Innertube API)

Used for: **everything** — search, browse, play, library, playlists, ratings.

No setup needed. Auth is handled automatically:
- **Browser auth** (default): reads Chromium cookies from `~/.config/chromium/`
- **OAuth**: interactive device-code flow on first run

## Optional — Metadata Providers

These improve album info (tracklists, years, genres, artist names).
Without them, albums still split using YouTube's built-in data.

### Last.fm

Used for: album metadata, track info, album art, scrobbling.

1. Get an API key at https://www.last.fm/api/account/create
2. Add to `~/.config/youtui/config.toml`:

```toml
[scrobbling]
api_key = "your_lastfm_api_key"
```

For scrobbling (submits plays to your Last.fm/Libre.fm account):

```toml
[scrobbling]
enabled = true
api_key = "your_lastfm_api_key"
api_secret = "your_lastfm_api_secret"
session_key = "your_lastfm_session_key"
api_url = "https://libre.fm"   # default: last.fm
```

Priority in pipeline: 10 (runs 2nd, after Metal Archives).

### Discogs

Used for: album metadata (years, tracklists, genres, styles).

1. Generate a personal access token at https://www.discogs.com/settings/developers
2. Add to `~/.config/youtui/config.toml`:

```toml
[scrobbling]
discogs_token = "your_discogs_token"
```

Priority in pipeline: 8 (runs 3rd, after Last.fm).

### MusicBrainz

Used for: album metadata fallback.

No setup needed — MusicBrainz API is free and open. Runs automatically.

Priority in pipeline: 7 (runs 4th).

### Genius

Used for: lyrics display + annotations.

1. Register an app at https://genius.com/api-clients (free)
2. Generate a Client Access Token
3. Add to `~/.config/youtui/config.toml`:

```toml
[scrobbling]
genius_token = "your_genius_token"
```

Without token: lyrics still work (page scrape fallback). Annotations are
unreliable without token.

Priority in pipeline: 40 (runs 5th, lowest — only when no other provider
matches).

### Metal Archives

Used for: metal band metadata (genre, year, tracklists).

**Only works with a Cloudflare bypass cookie.** The public REST API
(metal-api.dev) returns 500 errors and is unusable.

The only working path is direct Metal Archives access with a
`cf_clearance` cookie:

**Option A — env var (easiest):**
1. Open https://www.metal-archives.com/ in your browser
2. Open DevTools → Application → Cookies → copy `cf_clearance` value
3. Run: `export MA_COOKIE="cf_clearance=your_value"`
4. Start youtui in same terminal (or add to `.bashrc`/`.zshrc`)

**Option B — local proxy (auto-cookie):**
```bash
cargo run --release -p metal-proxy -- --get-cookie
```
This opens Chromium, you log into Metal Archives manually, and it saves
the cookie to `~/.config/youtui/ma_cookie`. The cookie expires ~30 min
and needs periodic refresh.

**Without a cookie**, Metal Archives metadata is skipped entirely.
Other providers (MusicBrainz, Discogs, Last.fm) still run.

Priority in pipeline: 5 (highest — runs first if cookie available).

## Metadata Pipeline Order

Providers run in priority order. First provider to return a scored result
wins. Lower number = checked first.

| Priority | Provider | Requires |
|----------|----------|----------|
| 5 | Metal Archives | `MA_COOKIE` env or local proxy |
| 7 | MusicBrainz | nothing |
| 8 | Discogs | `discogs_token` |
| 10 | Last.fm album | `api_key` |
| 10 | Last.fm track | `api_key` |
| 40 | Genius | `genius_token` |

## Cache

Metadata is cached in `~/.local/share/youtui/metadata_cache.json` (200
entries LRU). Survives restarts. Clear by deleting the file.
