# youtui — caos-obliquo fork

Vim-driven TUI for YouTube Music. Fork of [nick42d/youtui](https://github.com/nick42d/youtui) / [Icedwolf/youtui](https://github.com/Icedwolf/youtui) with custom playlist management, yt-dlp audio, zero F-keys.

**Upstream diff**: this fork adds playlist creation/update popups, EP/single labels in artist browser, yt-dlp audio backend by default, vim-only keybinds with minimal header, queue persistence, and effect-driven playlist management. Drifted significantly — we own the feature set now.

## Workspace

| Crate | Description |
|---|---|
| `youtui` | TUI binary — ratatui, crossterm, rodio |
| `ytmapi-rs` | Async YouTube Music API (generic over auth) |
| `json-crawler` | JSON traversal utilities |
| `async-callback-manager` | Task/effect framework connecting UI to backend |

## Install

```sh
git clone https://github.com/caos-obliquo/youtui
cd youtui
cargo install --path youtui --force
```

No AUR. Local compilation only.

### Dependencies

- `alsa-lib` (Linux) — audio playback
- `yt-dlp` — audio download (default backend)
- Font with FontAwesome glyphs

## Authentication

Any of the three methods:

### Browser (default)

1. Open `music.youtube.com` logged into your account
2. DevTools → Network → reload → click any POST request
3. Copy the `Cookie` header value
4. Save to `~/.config/youtui/cookie.txt`

### OAuth

```sh
youtui setup-oauth <client_id> <client_secret>
```

Requires a Google Cloud project with "TVs and Limited Input devices" OAuth client.

### Keybinds

| Key | Action |
|---|---|---|
| `1` | Now Playing |
| `2` | Song Search |
| `3` | Artist Search |
| `4` | Playlist Search |
| `5` | View Browser |
| `j` / `k` | Up / Down |
| `h` / `l` | Prev / Next tab |
| `C-b` / `C-u` | Page up |
| `C-f` / `C-d` | Page down |
| `g` / `G` | First / last |
| `d` / `D` | Delete selected / all |
| `y` | View lyrics (any song view) |
| `c` | Cycle category filter (artist albums) |
| `o` | Context menu (Enter→Play, d→Delete, l→Lyrics) |
| `s` | Shuffle |
| `A` | Set best quality |
| `n` | Save queue as new playlist |
| `e` / `E` | Add song(s) to existing playlist |
| `Tab` / `S-Tab` | Search suggestion navigation |
| `C-n` / `C-p` | Search suggestion navigation |
| `Esc` | Close search/sort/filter pane |
| `?` | Help |
| `Space` | Play / Pause |
| `q` | Quit |

Full keybinds at `~/.config/youtui/config.toml`.

## Features

- **Vim navigation** — j/k/h/l/g/G, C-b/C-u/C-f/C-d, no function keys
- **yt-dlp audio** — streams with `android_vr` extractor-args, no PO token needed
- **Playlist management** — create from queue (`n`), add to existing (`e`/`E`), unlisted by default
- **Delete** — `d` delete selected, `D` delete all (direct, no Enter prefix)
- **Lyrics** — `y` key, Musixmatch (no API key needed)
- **Category filter** — `c` key in artist album view (All/Album/EP/Single)
- **Context menu** — `o` opens mode with Play + Delete + Lyrics
- **EP / Single labels** — artist browser shows `Album:`, `EP:`, `Single:` prefix on release names
- **Persistent queue** — survives restarts
- **Configurable** — keybinds, downloader, auth style via `config.toml`

## Config

`~/.config/youtui/config.toml`:

```toml
auth_type = "Browser"
downloader_type = "YtDlp"
yt_dlp_command = "yt-dlp"
```

## Upcoming

- [ ] Lyrics — native Musixmatch integration (`musixmatch-cli` crate) with dedicated binding
- [ ] Scrobbling — embed Rescrobbled natively (Rust → ListenBrainz / Maloja)
- [ ] Plain-text config editor

## Known Issues

- Playlist creation requires a fresh browser cookie (write operations need active session)
- Client version scraped from YouTube Music page — canary suffix stripped automatically

## Build

```sh
cargo build --release
./target/release/youtui

cargo test --bins --lib        # unit tests
cargo clippy                   # lint
```

## Architecture

```
User KeyEvent → crossterm → Action → Effect (AsyncTask)
→ TaskManager → Server (API/Player/Downloader)
→ Response mutation → UI state → Redraw
```

API generic over `AuthToken` (Browser / OAuth / NoAuth) — enforced at compile time. Runtime dispatch via `DynamicYtMusic` enum.

## License

MIT — see [LICENSE](./LICENSE.txt) file.

**Note:** This project incorporates code from external crates with various licenses (MIT, Apache 2.0, BSD-2-Clause, and others). Refer to each crate's license for details. This fork does not change the licensing terms of the original project.
