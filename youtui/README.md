# youtui — caos-obliquo fork

Vim-keyboard-driven YouTube Music TUI. Casual fork of [Icedwolf/youtui](https://github.com/Icedwolf/youtui) with custom playlist management, yt-dlp audio, and zero F-keys.

## Quick Start

```sh
git clone https://github.com/caos-obliquo/youtui
cd youtui/youtui
cargo install --path . --force
```

**Authentication** — paste a browser Cookie header into `~/.config/youtui/cookie.txt`:
1. Open youtube.com in your browser while logged in
2. DevTools → Network → reload → click any request to youtube.com
3. Find `Cookie:` in request headers, copy the entire value
4. `echo 'PASTE_HERE' > ~/.config/youtui/cookie.txt`

## Keybinds

| Key | Action |
|---|---|
| `1` | Now Playing / Queue |
| `2` | Song Search |
| `3` | Artist Search |
| `4` | Playlist Search |
| `5` | View Browser (tab through modes) |
| `j` / `k` | Scroll down / up |
| `h` / `l` | Previous / next tab |
| `C-b` / `C-f` | Page up / down |
| `g` / `G` | First / last item |
| `s` | Toggle shuffle |
| `A` | Set best audio quality |
| `n` | Save queue to new playlist |
| `e` / `E` | Add song(s) to existing playlist |
| `?` | Toggle help overlay |
| `Space` | Play / Pause |
| `q` | Quit |

Full keybind reference: `~/.config/youtui/config.toml`

## Features

- **Playlist management** — create playlists from queue, add songs to existing ones, unlisted by default so they appear on other devices
- **yt-dlp audio** — streams via yt-dlp with `android_vr` extractor-args (no PO token needed)
- **EP/Single labels** — artist browser shows `Album:`, `EP:`, `Single:` prefix on release names
- **Vim navigation** — no function keys required; j/k/h/l/g/G throughout
- **Minimal header** — clean UI with only essential controls visible
- **Persistent queue** — survives restarts

## Branches

| Branch | Purpose |
|---|---|
| `main` | Upstream sync |
| `merge/friends-fork` | Active development |
| `fix/audio-ytdlp` | yt-dlp + vim config (stable) |
| `fix/playlist-update-popup` | Popup fixes, playlist create |
| `feat/ep-singles` | EP/single category display |

## Config

`~/.config/youtui/config.toml` — bundled defaults in `youtui/config/config.toml`:

```toml
auth_type = "Browser"
downloader_type = "YtDlp"
yt_dlp_command = "yt-dlp"
```

## Building

```sh
cargo install --path . --force
# or for development:
cargo build --release
./target/release/youtui
```

## TODO

- [ ] Context menu (`o` key, ncspot-style)
- [ ] Playlist creation 401 — cookie expired, need fresh cookie or debug OAuth
- [ ] Plain-text config for easy editing
