# Subsystem: Auth

Three authentication strategies for YTM API access.

## Browser Cookie Auth (default)

File: `ytmapi-rs/src/auth/browser.rs`

**Requires:** `cookie.txt` from yt-dlp. Browser is configurable via `cookie_browser` in `config.toml` (default: `chromium`).

Extract per platform:

| Platform | Command | Notes |
|----------|---------|-------|
| Linux (Chromium) | `yt-dlp --cookies-from-browser chromium --cookies cookie.txt` | Default browser |
| Linux (Firefox) | `yt-dlp --cookies-from-browser firefox --cookies cookie.txt` | Also works on BSD |
| Linux (Brave) | `yt-dlp --cookies-from-browser brave --cookies cookie.txt` | |
| macOS (Safari) | `yt-dlp --cookies-from-browser safari --cookies cookie.txt` | Built-in |
| macOS (Chrome) | `yt-dlp --cookies-from-browser chrome --cookies cookie.txt` | |
| macOS (Firefox) | `yt-dlp --cookies-from-browser firefox --cookies cookie.txt` | Also works on BSD |
| BSD (Firefox) | `yt-dlp --cookies-from-browser firefox --cookies cookie.txt` | Widest compatibility |

**Flow:**
1. Parse Netscape-format cookie file
2. Deduplicate via `BTreeMap` (last-wins) - yt-dlp appends duplicates without removing old entries
3. Extract critical cookies: `OSID`, `__Secure-3PSIDCC`, `__Secure-3PSID`, `LOGIN_INFO`, `SAPISID`
4. Build request context header with authentication state
5. All subsequent API calls use this context

**Cookie dedup fix** (`ytmapi-rs/src/auth/browser.rs:96-130`):
- yt-dlp auto-refresh appends cookies without removing old ones
- Duplicates have DIFFERENT values for critical auth cookies
- Fix: `BTreeMap<{name, domain, path}, value>` insert (last-wins) matching Python dict behavior

## OAuth Auth

File: `ytmapi-rs/src/auth/oauth.rs`

**Flow:**
1. Request device code from Google OAuth endpoint
2. Display code to user (opens browser)
3. Poll for token approval
4. Store OAuth token
5. Build request context with OAuth credentials

## NoAuth

File: `ytmapi-rs/src/auth/noauth.rs`

Sends requests without authentication. Most queries return empty results or 403 errors. Not practically usable.

## API Key

File: `ytmapi-rs/src/builder.rs`, `app/config.rs`

Two key types:

```rust
pub enum ApiKey {
    None,           // No auth
    OAuth,          // OAuth device flow
    ApiKey(String), // Cookie-based (browser auth)
}
```

## Token Refresh

File: `app/api.rs`

OAuth tokens are refreshed automatically when they expire:

```rust
pub async fn refresh_token(&mut self) -> Result<Option<OAuthToken>>
```

## Cookie File Location

`~/.config/youtui/cookie.txt` (XDG - works on Linux, macOS, BSD). Path flows from `main.rs → app.rs → YoutuiWindow::new → Playlist → Server`.

## OAuth Token Location

`~/.local/share/youtui/` (XDG - works on all platforms). Token file auto-generated on first OAuth flow.
