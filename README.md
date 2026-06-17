# youtui

Vim-driven TUI for YouTube Music. Listen, search, manage playlists, view lyrics, scrobble - all from your terminal.

## Quick Start

```sh
# 1. Install
git clone https://github.com/caos-obliquo/youtui
cd youtui
cargo install --path youtui --force

# 2. Get cookie (YouTube Music auth)
# Open music.youtube.com in Chrome/Firefox, logged in
# DevTools -> Network -> reload -> click any POST request
# Copy the "Cookie" header value, save:
mkdir -p ~/.config/youtui
echo "PASTE_COOKIE_HERE" > ~/.config/youtui/cookie.txt

# 3. Run
youtui
```

Press `1` for playlist, `j`/`k` to navigate, `Enter` to play, `Space` to pause.

---

## Authentication

### Browser (easiest, recommended)
1. Open `music.youtube.com` logged into your Google account
2. Open DevTools (F12) -> Network tab -> reload page
3. Click any POST request (filter by "music.youtube.com")
4. Scroll to Request Headers, find `Cookie:`
5. Copy the ENTIRE cookie value (long string starting with `__Secure-...`)
6. Save to `~/.config/youtui/cookie.txt`

### OAuth (alternative)
```sh
youtui setup-oauth <client_id> <client_secret>
```
Requires Google Cloud OAuth client (TVs and Limited Input devices).

---

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

Press `C-e` to edit config from within the app. Press `?` for help.

---

## Default Keybinds

| Key | Action |
|---|---|
| **Navigation** | |
| `j`/`k` | Up / Down |
| `h`/`l` | Left / Right |
| `C-b`/`C-u` | Page up |
| `C-f`/`C-d` | Page down |
| `g`/`G` | First / Last |
| | |
| **Playback** | |
| `Enter` | Play selected (or Enter then Enter) |
| `Space` | Play / Pause |
| `s` | Shuffle |
| `<`/`>` | Prev / Next song |
| `[`/`]` | Seek back / forward |
| | |
| **Views** | |
| `1` | Now Playing / Playlist |
| `2` | Song Search |
| `3` | Artist Search |
| `4` | Playlist Search |
| `5` | View Browser |
| | |
| **Actions** | |
| `d`/`D` | Delete selected / all |
| `y` | View lyrics |
| `o` | Context menu (Enter->Play, d->Delete, l->Lyrics, y->Share) |
| `c` | Category filter (Album/EP/Single) |
| `n` | Save queue to new playlist |
| `e`/`E` | Add song(s) to existing playlist |
| `A` | Set best quality |
| `:` | Open URL (paste YouTube Music link) |
| `C-y` | Copy song URL to clipboard |
| `C-e` | Edit config file |
| `0` | View logs |
| | |
| **Search** | |
| `/` | Search (any view) |
| `Esc` | Close search / vim normal mode |
| `n`/`N` | Next / previous match |
| `C-n`/`C-p` | Search suggestions |
| Tab/S-Tab | Search suggestions |
| _vi-mode:_ `h`/`l`/`w`/`b`/`0`/`$` move, `i` insert, `dd`/`dw`/`db` delete |
| | |
| **System** | |
| `?` | Toggle help |
| `q` | Quit (YOU DIED confirmation) |
| `C-c` | Force quit |

Full keybinds at `~/.config/youtui/config.toml`.

---

## Features

- **Vim navigation** - j/k/h/l/g/G, C-b/C-u/C-f/C-d, no function keys needed
- **yt-dlp audio** - streams via `android_vr` extractor, no PO token needed
- **Lyrics** - `y` key, fetches from Musixmatch + Genius + lyr CLI fallback
- **Annotations** - press `a` in lyrics popup to view Genius annotations
- **Category filters** - `c` toggles Album/EP/Single in artist and playlist views
- **Config editor** - `C-e` to edit config.toml directly in the app
- **Context menu** - `o` for quick Play/Delete/Lyrics/Share
- **Copy URL** - `C-y` copies current song's YouTube Music link to clipboard
- **Open URL** - `:` to paste and play a YouTube Music link
- **Playlist management** - create from queue (`n`), add to existing (`e`/`E`)
- **Delete** - `d`/`D` direct, no Enter prefix needed
- **Dark Souls quit** - `q` shows YOU DIED screen with y/N confirmation
- **EP/Single labels** - Album:/EP:/Single: prefixes in artist browser
- **Persistent queue** - survives restarts
- **Scrobbling** - Rescrobbled embedded (spawns on start with scrobbling config)
- **YouTube fallback** - searches YouTube when YTMusic has no results

---

## Dependencies

- `alsa-lib` (Linux) - audio playback
- `yt-dlp` - audio download (install separately: `sudo pacman -S yt-dlp`)
- Font with FontAwesome glyphs
- `wl-copy` (Wayland) or `xclip` (X11) - for copy URL feature

---

## Build & Test

```sh
cargo build --release
./target/release/youtui

cargo test --bins --lib
```

---

## Architecture

```
KeyEvent -> crossterm -> Action -> Effect (AsyncTask)
-> TaskManager -> Server (API/Player/Downloader)
-> Response -> UI state -> Redraw
```

---

## License

MIT - see [LICENSE](./LICENSE.txt).
