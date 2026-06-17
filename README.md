# youtui

Vim-driven TUI for YouTube Music. Fork with custom playlist management, yt-dlp audio, lyrics, category filters, and vim-only keybinds.

## Quick Start

```sh
# Install
git clone https://github.com/caos-obliquo/youtui
cd youtui
cargo install --path youtui --force

# Get a cookie (see Authentication below)
# Then run:
youtui
```

Press `1` to view playlist, `2`-`6` for browser views, `j`/`k` to navigate, `Enter` to play.

## Authentication

### Browser (easiest)
1. Open `music.youtube.com` logged into your account
2. DevTools -> Network -> reload -> click any POST request
3. Copy the `Cookie` header value
4. Save to `~/.config/youtui/cookie.txt`

### OAuth
```
youtui setup-oauth <client_id> <client_secret>
```

Requires a Google Cloud project with "TVs and Limited Input devices" OAuth client.

## Config

`~/.config/youtui/config.toml`:

```toml
auth_type = "Browser"
downloader_type = "YtDlp"
yt_dlp_command = "yt-dlp"

[scrobbling]
enabled = false
api_key = ""
api_secret = ""
session_key = ""
```

Press `C-e` to edit config from within the app.

## Keybinds

| Key | Action |
|---|---|
| `1` | Now Playing |
| `2` | Song Search |
| `3` | Artist Search |
| `4` | Playlist Search |
| `5` | View Browser |
| `6` | Change Search Type |
| `j`/`k` | Up / Down |
| `h`/`l` | Left / Right |
| `C-b`/`C-u` | Page up |
| `C-f`/`C-d` | Page down |
| `g`/`G` | First / Last |
| `d`/`D` | Delete selected / all |
| `y` | View lyrics (any view) |
| `c` | Category filter (artist/playlist) |
| `o` | Context menu (Enter->Play, d->Delete, l->Lyrics) |
| `s` | Shuffle |
| `A` | Set best quality |
| `n` | Save to new playlist |
| `e`/`E` | Add song(s) to existing playlist |
| `/` | Search (any view) |
| `Esc` | Close search/filter/sort |
| `C-n`/`C-p` | Search suggestion navigation |
| `C-e` | Edit config file |
| `?` | Toggle help |
| `Space` | Play / Pause |
| `q` | Quit (y/N to confirm) |

Search boxes support **vi-mode**: `Esc` for normal mode, `i` for insert, `h`/`l`/`w`/`b`/`0`/`$` to move, `dd`/`dw`/`db` to delete. Use `n`/`N` to jump between search matches.

Full keybinds at `~/.config/youtui/config.toml`.

## Features

- **Vim navigation** - j/k/h/l/g/G, C-b/C-u/C-f/C-d, no function keys
- **yt-dlp audio** - streams with `android_vr` extractor-args, no PO token
- **Lyrics** - `y` key, Musixmatch + Genius + `lyr` CLI fallback
- **Category filters** - `c` toggles Album/EP/Single in artist and playlist views
- **Config editor** - `C-e` to edit config.toml in-app
- **Context menu** - `o` for Play/Delete/Lyrics
- **Playlist management** - create from queue (`n`), add to existing (`e`/`E`)
- **Delete** - `d`/`D` direct, no Enter prefix
- **Dark Souls quit** - `q` shows YOU DIED confirmation
- **EP/Single labels** - Album:/EP:/Single: prefixes in artist browser
- **Seamless queue** - persists across restarts
- **Scrobbling** - Rescrobbled compatible (native Last.fm module ready)

## Build

```sh
cargo build --release
./target/release/youtui

cargo test --bins --lib
```

## Dependencies

- `alsa-lib` (Linux) - audio playback
- `yt-dlp` - audio download (default backend)
- Font with FontAwesome glyphs

## Known Issues

- Playlist creation requires a fresh browser cookie
- Client version scraped from YouTube Music page at startup

## Architecture

```
KeyEvent -> crossterm -> Action -> Effect (AsyncTask)
-> TaskManager -> Server (API/Player/Downloader)
-> Response -> UI state -> Redraw
```

## License

MIT - see [LICENSE](./LICENSE.txt).
