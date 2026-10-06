# Configuration

File: `~/.config/youtui/config.toml`

> **Cross-platform**: All paths use XDG convention (`~/.config/`, `~/.local/share/`). Works on Linux (XDG-compliant), macOS (`~/.config/` directory works despite no official XDG support), and BSD (XDG-compliant).

## ConfigIR Fields

All fields parsed from TOML into `ConfigIR` (youtui/src/config.rs:149), then converted to `Config`. `ConfigIR` uses `#[serde(default, deny_unknown_fields)]` so unknown keys are rejected.

### auth_type

| Value | Auth Method | Requires |
|-------|-------------|----------|
| `Browser` (default) | Browser cookie extraction | Chromium cookie file |
| `OAuth` | OAuth device code flow | Interactive setup |
| `Unauthenticated` | No authentication | Limited/breaks |

### downloader_type

| Value | Downloader | Status |
|-------|------------|--------|
| `Native` | rusty_ytdl (Rust native) | Partially broken - 403 errors |
| `YtDlp` (default) | yt-dlp external process | Working - recommended |

### yt_dlp_command

Default: `"yt-dlp"` - path to yt-dlp binary.

### bandcamp_yt_dlp_command

Optional dedicated yt-dlp binary for Bandcamp URLs (uv-tool build with curl_cffi). Falls back to `yt_dlp_command` when unset.

### cookie_browser

Default: `"chromium"` - browser to read cookies from for YTM auth and yt-dlp `--cookies-from-browser`.

### scrobbling

```toml
[scrobbling]
enabled = false
api_key = ""
api_secret = ""
session_key = ""
genius_token = ""         # Required for reliable Genius annotations
discogs_token = ""        # Better Discogs album metadata coverage
listenbrainz_token = ""  # ListenBrainz metadata + recommendations
musicbrainz_bearer_token = ""  # MusicBrainz authenticated requests
musicbrainz_client_id = ""     # MusicBrainz OAuth client id
musicbrainz_client_secret = "" # MusicBrainz OAuth client secret
```

### keybinds

Organized by context as TOML tables. Key names are the literal key (e.g., `h`, `enter`, `space`, `C-n`, `F1`, `o.a`). Actions use the snake_case action name from the codebase.

See [Keybindings](05-keybindings.md) for all contexts, actions, and defaults.

Override example (in `~/.config/youtui/config.toml`):

```toml
[keybinds.global]
F1 = {action = "toggle_browser", visibility = "global"}
space = "play_pause"

[keybinds.playlist]
"o.s" = "playlist.toggle_shuffle"
"o.l" = "playlist.view_lyrics"
```

### mode_names

Custom mode names for context menus. Rarely used.

## Example Config

Minimal overrides (all keybinds use Rust defaults unless specified):

```toml
auth_type = "Browser"
downloader_type = "YtDlp"
yt_dlp_command = "yt-dlp"

[scrobbling]
enabled = true
api_key = "your_lastfm_api_key"
api_secret = "your_lastfm_secret"
session_key = "your_lastfm_session_key"
genius_token = "your_genius_token"
discogs_token = "your_discogs_token"
```

Custom keybind example (override specific keys):

```toml
[keybinds.browser]
h = "browser.left"
l = "browser.right"

[keybinds.list]
k = {action = "list.up", visibility = "hidden"}
j = {action = "list.down", visibility = "hidden"}
```

See `youtui/config/config.toml.vim-example` in the source tree for a vim-navigation-only override example.

## Config Loading Sequence

```
Config::new(debug)
  1. get_config_dir() → ~/.config/youtui/
  2. read config.toml
  3. toml::from_str::<ConfigIR>(content)
  4. Config::try_from(ir) - converts IR + validates keybinds
  5. Fallback: use defaults if file missing
```

## Hot Reload

`:reload` in command mode re-reads `config.toml` from disk and rebuilds the Config struct. Only keybinds and scrobbling config are reloaded - auth and downloader type require restart.
