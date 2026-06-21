# Configuration

File: `~/.config/youtui/config.toml`

## ConfigIR Fields

All fields parsed from TOML into `ConfigIR` (app/config.rs:106), then converted to `Config`.

### auth_type

| Value | Auth Method | Requires |
|-------|-------------|----------|
| `browser` (default) | Browser cookie extraction | Chromium cookie file |
| `oauth` | OAuth device code flow | Interactive setup |
| `noauth` | No authentication | Limited/breaks |

### downloader_type

| Value | Downloader | Status |
|-------|------------|--------|
| `native` | rusty_ytdl (Rust native) | Partially broken — 403 errors |
| `yt-dlp` (default) | yt-dlp external process | Working — recommended |

### yt_dlp_command

Default: `"yt-dlp"` — path to yt-dlp binary.

### scrobbling

```toml
[scrobbling]
enabled = false
api_url = "https://libre.fm"
api_key = ""
api_secret = ""
session_key = ""
```

### keybinds

TOML tables mapping key → action for each context. See [Keybindings](05-keybindings.md) for defaults.

Example:
```toml
[keybinds]
"global.play_pause" = "space"
"global.next_song" = "l"
"playlist.up" = "k"
"playlist.down" = "j"
```

### mode_names

Custom mode names for context menus. Rarely used.

## Example Config

```toml
auth_type = "browser"
downloader_type = "yt-dlp"
yt_dlp_command = "yt-dlp"

[scrobbling]
enabled = true
api_url = "https://libre.fm"
api_key = "your_api_key"
api_secret = "your_api_secret"
session_key = "your_session_key"

[keybinds]
"global.play_pause" = "space"
"global.next_song" = "l"
"global.prev_song" = "h"
```

## Config Loading Sequence

```
Config::new(debug)
  1. get_config_dir() → ~/.config/youtui/
  2. read config.toml
  3. toml::from_str::<ConfigIR>(content)
  4. Config::try_from(ir) — converts IR + validates keybinds
  5. Fallback: use defaults if file missing
```

## Hot Reload

`:reload` in command mode re-reads `config.toml` from disk and rebuilds the Config struct. Only keybinds and scrobbling config are reloaded — auth and downloader type require restart.
