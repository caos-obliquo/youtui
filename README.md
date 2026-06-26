# youtui

Vim-style YouTube Music player for your terminal. Keyboard-only, no mouse, no Electron.

## Requirements

- **Rust** (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **yt-dlp** (`sudo pacman -S yt-dlp` or `brew install yt-dlp`)
- **ffmpeg** (`sudo pacman -S ffmpeg` or `brew install ffmpeg`)
- **alsa-lib** (Linux: `sudo pacman -S alsa-lib` or `sudo apt install libasound2-dev`)
- A **YouTube Music account** (free Google account)

---

## Quick start

```sh
git clone https://github.com/caos-obliquo/youtui
cd youtui
cargo build --release
cargo install --path youtui --force

# Set up cookie then:
youtui
```

---

## Authentication

### Method A: Browser cookie (recommended)

1. Open **Chrome** (or Firefox), go to `music.youtube.com`, **log in**
2. Press **F12** > **Network** tab > **Reload** (F5) > filter by `music`
3. Click any POST request > scroll to **Request Headers** > **Cookie:**
4. Right-click > **Copy value**
5. Save to `~/.config/youtui/cookie.txt` (single line, no quotes, no `Cookie:` prefix)

### Method B: Auto-extract from browser

If Chrome is logged in to YouTube Music, youtui reads the cookie file directly and passes `--cookies-from-browser chromium` to yt-dlp for restricted videos.

### Cookie rotation

Cookies expire. When you see `Error 400` or `Sign in to confirm you're not a bot`, refresh your cookie from `music.youtube.com`.

---

## How to use

### Design philosophy

- **Enter** does the primary action (play song, open album, focus panel). No sub-menus.
- **`o`** opens the context menu for secondary actions (lyrics, queue, save, etc.)
- **`/`** triggers local fuzzy filter in any list view.
- **`F1`** opens YTM search in browser tabs.
- All 5 browser tabs (Artists, Songs, Albums, Library, PlaylistSearch) share the same UI: vim navigation, sort/filter columns, context menus.

### Global keys

| Key | Action |
|---|---|
| `Space` | Play / Pause |
| `>` / `<` | Next / Previous track |
| `]` / `[` | Seek +5s / -5s |
| `+` / `-` | Volume up / down |
| `?` | Help (all keybindings per context) |
| `F1` | YTM search (in browser) |
| `F2` | Toggle browser/playlist view |
| `F3` | Toggle queue/playlist view |
| `F7` | Change browser search tab |
| `F11` | View logs |
| `q` | Quit (with confirmation) |
| `:` | Open URL (`: https://youtu.be/...`) |
| `C-c` | Quit immediately |
| `C-e` | Edit config file |

### Playlist / Queue

| Key | Action |
|---|---|
| `j` / `k` | Move down / up |
| `g` / `G` | First / last |
| `C-u` / `C-d` | Page up / down |
| `Enter` | Play selected song |
| `d` | Delete selected |
| `u` | Undo last delete |
| `y` / `Y` | Copy song / album URL |
| `/` | Local fuzzy filter |
| `n` / `N` | Next / prev search result |
| `V` | Toggle visual mode (line selection) |
| `p` / `P` | Paste yanked songs below/above |
| `Esc` | Clear search / close popup |

### Context menu (press `o` then)

| Key | Action |
|---|---|
| `s` | Toggle shuffle |
| `r` / `S` | Sort queue |
| `R` | Get related tracks |
| `l` | View lyrics |
| `a` / `b` | Go to artist / album page |
| `v` | Album art popup (sixel graphics) |
| `I` | Song info (metadata, genres, RYM tags) |
| `t` | Like / unlike song |
| `z` | Toggle repeat |
| `m` | Toggle romaji (Japanese > latin text) |
| `n` / `E` | Save to new / existing playlist |
| `q` / `L` / `Q` | Save / load / delete queue file |
| `D` | Delete all from queue |
| `A` | Set best audio quality |
| `c` | Category filter |
| `f` | Force-split album |
| `y` / `Y` | Copy song / album URL |

### Browser (5 tabs)

| Tab | What it shows |
|---|---|
| Artists | Search artists, view their songs |
| Songs | Search songs, with sort/filter columns |
| Albums | Search albums, like/subscribe |
| Library | Your library: playlists, artists, albums, songs |
| PlaylistSearch | Search YT Music playlists, browse tracks |

| Key | Action |
|---|---|
| `h` / `l` | Switch tab left / right |
| `F1` | YTM search across current tab |
| `/` | Local fuzzy filter in current view |
| `Enter` | Primary action (play song, open album, focus) |
| `o` | Context menu |
| `3` / `4` | Open filter / sort popup |
| `g` / `G` | First / last row |
| `r` | Reload category |
| `Backspace` | Navigate back |

### Context menu in browser (press `o`)

| Key | Action |
|---|---|
| `Enter` | Play song |
| `p` | Play all songs |
| `P` | Queue all songs |
| `s` | Add song to playlist |
| `a` / `b` | Go to artist / album |
| `l` | View lyrics |
| `y` / `Y` | Copy URL |
| `t` | Rate playlist / toggle like |
| `S` / `U` | Subscribe / unsubscribe artist |
| `N` | Insert next in queue |
| `r` | Get related tracks |

### Lyrics popup

| Key | Action |
|---|---|
| `Esc` / `q` | Close |
| `j` / `k` | Scroll |
| `g` / `G` | First / last line |
| `R` | Toggle romaji (for Japanese lyrics) |
| `a` | Toggle annotations panel (requires Genius token) |
| `Tab` / `l` / `h` | Switch focus between lyrics and annotations |
| `Enter` | Seek to timestamp `[m:ss]` |
| `(` / `)` | Previous / next song in queue |
| `V` | Visual mode (yank text with `y`) |
| `y` | Yank selection to clipboard |

### Playlist editor (vim-driven list editor)

| Key | Action |
|---|---|
| `j`/`k` | Move cursor down/up |
| `dd` | Delete line |
| `yy` | Yank line |
| `p`/`P` | Paste below/above |
| `u` | Undo (100-level stack) |
| `C-r` | Redo |
| `V` | Visual line selection |
| `:w` | Save |
| `:wq` | Save and quit |
| `:q` | Quit |
| `:q!` | Force quit |
| `E` | Save to existing playlist |

---

## Album splitting

Paste a full-album YouTube URL with `:` and youtui automatically splits it into individual tracks.

**How it works:**

1. **Title extraction** - yt-dlp gets video title
2. **Title cleaning** - strips `"FULL ALBUM"`, parenthetical tags, year suffixes
3. **Album search** - queries 6 metadata providers (Last.fm, Discogs, MusicBrainz, YTM, Genius, Metal Archives)
4. **Track creation** - each track becomes a playlist entry with correct title/duration/artist/album/year
5. **Audio extraction** - ffmpeg extracts each track from the full video (`-ss offset -t duration`)
6. **Gapless playback** - tracks auto-advance at track boundary
7. **Scrobbling** - each track scrobbles individually to Last.fm at ~50% playback

If splitting fails (no provider has the album): plays the full video as a single track. No crash, no error.

---

## Lyrics, Annotations & Romaji

| Feature | How |
|---|---|
| **Lyrics** | `o.l` > Genius (primary, slug URL search) > LRCLIB (free, no API key) > bandcamp-lyrics CLI > lyr CLI |
| **Annotations** | `a` in lyrics popup > Genius annotation fragments with explanations |
| **Romaji** | `R` in lyrics popup or `o.m` in queue > converts Japanese to latin using embedded IPADIC + ib-romaji |

Requires `genius_token` in config for annotations (see [API services](docs/api-services.md)).

---

## Configuration

File: `~/.config/youtui/config.toml`

```toml
auth_type = "Browser"
downloader_type = "YtDlp"    # or "Native" (rusty_ytdl, partially broken)
yt_dlp_command = "yt-dlp"

[scrobbling]
enabled = false
api_key = ""
api_secret = ""
session_key = ""
genius_token = ""             # Required for reliable annotations
discogs_token = ""            # Better album metadata coverage
```

Press `C-e` inside youtui to edit this file. `:reload` hot-reloads config at runtime.

### Keybind customization

All keybindings can be overridden in `config.toml` per context. See the example config at `youtui/config/config.toml` for all contexts and action names. Press `?` inside the app for the complete default keybind reference.

---

## Features

- **Vim keys everywhere** - j/k/h/l, C-u/C-d, g/G, gg, visual mode, operators
- **5 browser tabs** - Artists, Songs, Albums, Library, PlaylistSearch
- **Sort/filter columns** - Accessible via context menu (`o`) or `g` mode in any table view
- **Album splitting** - Paste album URL, auto-split into tracks with metadata
- **Metadata pipeline** - 6 providers (Last.fm, Discogs, MusicBrainz, YTM enrichment, Genius, Metal Archives)
- **Gapless playback** - Seamless transitions between album tracks
- **Lyrics** - Genius + LRCLIB + bandcamp-lyrics + lyr CLI
- **Annotations** - Genius annotation fragments with explanation panel
- **Romaji** - Japanese text to latin (embedded IPADIC dictionary)
- **Playlist CRUD** - Create, rename, edit, delete, rate, merge playlists
- **Playlist editor** - Vim-driven list editor with undo/redo/yank/paste
- **Album art** - Sixel graphics popup with pagination
- **Notes popup** - Vim-driven text editor for song links, personal notes
- **Scrobbling** - Per-track scrobbling to Last.fm
- **Queue persistence** - Save/load/delete queue files
- **Config hot-reload** - `:reload` without restart
- **MPRIS** - Media controls via souvlaki

---

## Build & Test

```sh
cargo build --release
./target/release/youtui

# Run tests
cargo test --release -p youtui --bin youtui        # 136 tests
cargo test --release -p metadata-provider           # 47 tests
cargo test --release -p vi-text-editor              # 65 tests
cargo test --release -p ytmapi-rs --lib             # 85 tests
```

Full test reference: see [docs/README.md](docs/README.md). 388+ tests across 11 crates.

---

## Troubleshooting

| Problem | Fix |
|---|---|
| `Sign in to confirm you're not a bot` | Cookie expired. Refresh from `music.youtube.com`. Or set `downloader_type = "Native"`. |
| `Requested format is not available` | yt-dlp format changed. Try `downloader_type = "Native"` or update yt-dlp. |
| App crashes on startup | Missing deps: `sudo pacman -S yt-dlp alsa-lib ffmpeg` |
| No audio | Check `yt-dlp --version` works. Reinstall yt-dlp. |
| Album didn't split | Not in any metadata provider. Plays as single video. No data loss. |
| Wrong track durations | YouTube video length differs from database. Gapless uses database durations. |

---

## License

MIT
