# youtui - caos-obliquo fork

Vim-driven TUI for YouTube Music. Originally forked from [nick42d/youtui](https://github.com/nick42d/youtui) / [Icedwolf/youtui](https://github.com/Icedwolf/youtui), now independently maintained. Custom playlist management, yt-dlp audio, minimal F-keys (F1 search, F2/F3 toggle, F11 logs).

**Upstream diff**: this fork adds playlist creation/update popups, EP/single labels in artist browser, yt-dlp audio backend by default, vim-only keybinds with minimal header, queue persistence, and effect-driven playlist management. Drifted significantly - we own the feature set now.

## Workspace

| Crate | Description |
|---|---|
| `youtui` | TUI binary - ratatui, crossterm, rodio |
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

- `alsa-lib` (Linux) - audio playback
- `yt-dlp` - audio download (default backend)
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
|---|---|
| `1` | Now Playing |
| `2` | Song Search |
| `3` | Artist Search |
| `4` | Playlist Search |
| `5` | View Browser |
| `j` / `k` | Up / Down |
| `h` / `l` | Prev / Next tab |
| `C-b` / `C-f` | Page up / down |
| `g` / `G` | First / last |
| `s` | Shuffle |
| `A` | Set best quality |
| `n` | Save queue as new playlist |
| `e` / `E` | Add song(s) to existing playlist |
| `?` | Help |
| `Space` | Play / Pause |
| `q` | Quit |

Full keybinds at `~/.config/youtui/config.toml`.

## Features

- **Vim navigation** - j/k/h/l/g/G throughout, minimal function keys (F1 search, F2/F3 tab nav)
- **yt-dlp audio** - streams with `android_vr` extractor-args, no PO token needed
- **Playlist management** - create from queue, add to existing, unlisted by default
- **EP / Single labels** - artist browser shows `Album:`, `EP:`, `Single:` prefix on release names
- **Persistent queue** - survives restarts
- **Configurable** - keybinds, downloader, auth style via `config.toml`

## Config

`~/.config/youtui/config.toml`:

```toml
auth_type = "Browser"
downloader_type = "YtDlp"
yt_dlp_command = "yt-dlp"
```

## Upcoming

- [ ] Lyrics - native Musixmatch integration (`musixmatch-cli` crate) with dedicated binding
- [ ] Scrobbling - embed Rescrobbled natively (Rust → ListenBrainz / Maloja)
- [ ] Context menu (`o` key, ncspot-style)
- [ ] Plain-text config editor

## Known Issues

- **ytmapi-rs (YouTube Music API)**: Google changes internal API frequently. yt-dlp is the primary/reliable backend for audio streaming. ytmapi-rs lib tests pass (82/82) but live integration calls may break without notice.
- Playlist creation requires a fresh browser cookie (write operations need active session)
- Client version extracted from YouTube Music page at startup - some endpoints may need updates

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

API generic over `AuthToken` (Browser / OAuth / NoAuth) - enforced at compile time. Runtime dispatch via `DynamicYtMusic` enum.

## License

MIT
