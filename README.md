# youtui - caos-obliquo fork

Vim-driven TUI for YouTube Music. Originally forked from [nick42d/youtui](https://github.com/nick42d/youtui), now independently maintained. yt-dlp audio, minimal F-keys (F1 search, F2/F3 toggle, F11 logs).

**Upstream diff**: this fork adds native scrobbling, album splitting with metadata enrichment, vim playlist editor, 6-provider metadata pipeline, context menu, sixel album art, lyrics with annotations, suckless codebase (-630 lines). Drifted significantly - we own the feature set now.

## Workspace

| Crate | Description |
|---|---|
| `youtui` | TUI binary - ratatui, crossterm, rodio |
| `ytmapi-rs` | Async YouTube Music API (generic over auth) |
| `json-crawler` | JSON traversal utilities |
| `async-callback-manager` | Task/effect framework connecting UI to backend |
| `metadata-provider` | 6 metadata providers (MusicBrainz, Discogs, Last.fm, Genius, etc.) |
| `audio-player` | Async rodio-based audio player (extracted from youtui) |
| `vi-text-editor` | Vim text editor widget (used in playlist/notes/config popups) |
| `genius-rs` | Genius lyrics + annotations API client |
| `lrclib-rs` | LRCLIB lyrics provider (free, no API key) |
| `rym-genre-data` | RYM genre/descriptor hierarchy for metadata enrichment |

## Install

```sh
git clone https://github.com/caos-obliquo/youtui
cd youtui
cargo install --path youtui --force
```

No AUR. Local compilation only.

### Dependencies

- `yt-dlp` - audio download (default backend)
- `ffmpeg` - album track splitting

#### Linux audio (ALSA)

| Distro | Package |
|---|---|
| Debian / Ubuntu / Mint | `libasound2-dev` |
| Arch / Manjaro | `alsa-lib` |
| Fedora | `alsa-lib-devel` |
| Void | `alsa-lib-devel` |
| NixOS | `alsaLib` (in `buildInputs`) |
| Gentoo | `media-libs/alsa-lib` |

#### macOS

Audio is built-in (CoreAudio). Install yt-dlp + ffmpeg:

```sh
brew install yt-dlp ffmpeg
```

#### BSD

Audio is built-in (OSS). Install yt-dlp + ffmpeg:

```sh
pkg_add yt-dlp ffmpeg    # FreeBSD / OpenBSD
```

## Authentication

Pick one:

### Browser (default)

1. Open `music.youtube.com` logged into your account
2. DevTools -> Network -> reload -> click any POST request
3. Copy the `Cookie` header value
4. Save to `~/.config/youtui/cookie.txt`

### OAuth

```sh
youtui setup-oauth <client_id> <client_secret>
```

Requires a Google Cloud project with "TVs and Limited Input devices" OAuth client.

## Keybinds

All keybinds are **customizable** in `~/.config/youtui/config.toml`.

Quick reference:

| Key | Action |
|---|---|
| `Space` | Play / Pause |
| `>` / `<` | Next / Prev track |
| `]` / `[` | Seek forward / back 5s |
| `+` / `-` | Volume up / down |
| `Enter` | Primary action (play song, open album, focus tab) |
| `o` | Context menu (all secondary actions) |
| `j` / `k` | Up / Down |
| `h` / `l` | Prev / Next tab |
| `g` / `G` | First / Last |
| `J` / `K` | Move song up / down in queue |
| `/` | Local fuzzy filter across visible items |
| `F1` | YTM search |
| `F2` / `F3` | Toggle browser / queue |
| `F11` | Logs |
| `?` | Help |
| `q` | Quit |
| `:` | Command prompt |
| `d` | Delete from queue |
| `u` | Undo delete |
| `V` | Visual mode (queue context) |

Full keybinds by context: `docs/05-keybindings.md`

## Features

- **Vim navigation** - j/k/h/l/g/G throughout, minimal F-keys
- **yt-dlp audio** - streams with `android_vr` extractor-args, no PO token needed
- **Album splitting** - full-album detection, track metadata enrichment, gapless playback
- **Native scrobbling** - Last.fm with persistent offline cache, retry on startup + 5-min background loop
- **Metadata pipeline** - 6 providers (MusicBrainz, Discogs, Last.fm, Genius, LRCLIB, RYM) with scoring
- **Lyrics** - Genius annotations + LRCLIB fallback, romaji toggle, vim navigation
- **Sixel album art** - footer thumbnail + full-size popup (`o.v`)
- **Playlist management** - create from queue, add to existing, rename, delete, merge, details, privacy, vim-driven editor
- **5 browser tabs** - Artists, Songs, Albums, Library, PlaylistSearch - all with F1 search, sort/filter, context menu
- **Persistent queue** - survives restarts
- **Configurable** - keybinds, downloader, auth style via `config.toml`

## Known Issues

- **ytmapi-rs (YouTube Music API)**: Google changes internal API format frequently. yt-dlp is the primary/reliable backend for audio streaming. ytmapi-rs lib tests pass (82/82) but 54 integration tests need a browser cookie and may fail without notice.
- **Playlist creation**: write operations require an active authenticated session (need fresh cookie).
- **Year metadata**: Some tracks show `None` when no provider returns data and album/song title has no `(YYYY)`.
- **Libre.fm scrobble**: fails silently - no retry on HTTP failure.
- **Metal Archives**: blocked by Cloudflare (cf_clearance cookie expires ~30 min).

## Build

```sh
cargo build --release
./target/release/youtui

cargo test --workspace --release --exclude ytmapi-rs   # 181 tests
cargo clippy --workspace -- -A warnings                # lint (0 warnings)
```

## Architecture

```
User KeyEvent -> crossterm -> Action -> Effect (AsyncTask)
-> TaskManager -> Server (API/Player/Downloader)
-> Response mutation -> UI state -> Redraw
```

See `docs/01-architecture.md` for details.

## Special Thanks

- **[nick42d](https://github.com/nick42d/youtui)** - Original youtui author. This fork would not exist without his work.
- **[sigma67/ytmusicapi](https://github.com/sigma67/ytmusicapi)** - Python YT Music API that ytmapi-rs was ported from.
- **[ncspot](https://github.com/hrkfdn/ncspot)** - Enter = primary action (never sub-menu) design copied directly. Queue-centric playback model.
- **[kopuz](https://github.com/kopuz-music/kopuz)** - Graphical desktop music player with Last.fm native scrobbling. Inspired the embedded scrobbler architecture.
## License

MIT
