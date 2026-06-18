# youtui

Vim-style YouTube Music player for your terminal. Keyboard-only, no mouse, no Electron — just your terminal and your music.

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

# Get your cookie (see Authentication section)
youtui
```

---

## Authentication

### Method A: Browser cookie (recommended)

1. Open **Chrome** (or Firefox), go to `music.youtube.com`, **log in**
2. Press **F12** → **Network** tab → **Reload** (F5) → filter by `music`
3. Click any POST request → scroll to **Request Headers** → **Cookie:**
4. Right-click → **Copy value**
5. Save to `~/.config/youtui/cookie.txt` (single line, no quotes, no `Cookie:` prefix)

```sh
mkdir -p ~/.config/youtui
cat > ~/.config/youtui/cookie.txt
# Paste, Ctrl+D
```

### Method B: Auto-extract from browser

If youtui is configured to use the same browser cookie file, it also passes `--cookies-from-browser chromium` to yt-dlp for restricted videos. No extra setup needed when your browser session is active.

### Cookie rotation

Cookies expire. When you see `Error 400` or `Sign in to confirm you're not a bot`, refresh your cookie from `music.youtube.com`.

---

## How to use

### Global keys

| Key | Action |
|---|---|
| `1` | Playlist view |
| `0` | Log viewer |
| `?` | Help (all keys) |
| `q` | Quit (press `y` to confirm) |
| `:` | Open URL (`: https://youtu.be/...`) |
| `C-c` | Quit |
| `C-e` | Edit config |
| `C-y` | Copy current song URL |

### Playlist navigation

| Key | Action |
|---|---|
| `j` / `k` | Move down / up |
| `g` / `G` | First / last |
| `C-u` / `C-d` | Page up / down |
| `Enter` | Play selected |
| `s` | Toggle shuffle |
| `/` | Search within playlist |

### Playback

| Key | Action |
|---|---|
| `Space` | Play / Pause |
| `<` / `>` | Previous / Next track |
| `[` / `]` | Seek -5s / +5s |
| `+` / `-` | Volume up / down |
| `A` | Toggle audio quality |

### Song actions

| Key | Action |
|---|---|
| `y` | Show lyrics |
| `a` | Toggle annotations (in lyrics popup, requires Genius token) |
| `R` | Toggle romaji mode (Japanese → latin) |
| `d` | Delete selected song |
| `D` | Delete all |

### Context menu

Press `o` then:

| Key | Action |
|---|---|
| `Enter` | Play |
| `d` | Delete |
| `y` | Lyrics / Copy URL |
| `l` | Lyrics |

---

## Album splitting

Paste a full-album YouTube URL with `:` and youtui automatically splits it into individual tracks.

**How it works:**

1. **Title extraction** — yt-dlp gets the video title (e.g. `"Artist - Album FULL ALBUM (year - genre)"`)
2. **Title cleaning** — strips `"Artist - "` prefix, then `"FULL ALBUM"`, parenthetical genre tags, year suffixes, etc.
3. **Album search** — queries Last.fm `album.search` → `album.getInfo` to fetch the official tracklist with durations
4. **Fallback chain** — if Last.fm has no data, tries Discogs API → MusicBrainz
5. **Track creation** — each track becomes a playlist entry with correct title, duration, artist, album, year
6. **Audio extraction** — ffmpeg extracts each track's section from the full video (`-ss offset -t duration`)
7. **Gapless playback** — tracks auto-advance at track boundary via `QueueDecodedSong`
8. **Scrobbling** — each track scrobbles individually to Last.fm at ~50% playback

**What gets matched:**

| YouTube title pattern | Result |
|---|---|
| `"Artist - Album (year)"` | Album found on Last.fm → 8+ tracks ✅ |
| `"Album FULL ALBUM (genre)"` | FULL ALBUM stripped → clean album name → matched ✅ |
| `"Artist - Track"` | Not an album → plays as single track (no split) ✅ |
| `"Underground Band - Demo"` | Not on Last.fm → Discogs fallback → matched ✅ |

**If splitting fails** (no database has the album): plays the full video as a single track. No crash, no error.

---

## Lyrics, Annotations & Romaji

| Feature | How |
|---|---|
| **Lyrics** | `y` on any song → fetches from Musixmatch → Genius → Bandcamp CLI |
| **Annotations** | `a` in lyrics popup → Genius annotation fragments with explanations |
| **Romaji** | `R` in playlist or lyrics → converts Japanese text to latin. Uses embedded IPADIC dictionary + ib-romaji |

Requires `genius_token` in config for annotations (see Config).

---

## Configuration

File: `~/.config/youtui/config.toml`

```toml
auth_type = "Browser"
downloader_type = "YtDlp"       # or "Native" (uses ytmapi-rs directly)
yt_dlp_command = "yt-dlp"

[scrobbling]
enabled = false
api_key = ""
api_secret = ""
session_key = ""
genius_token = ""                # Required for annotations in lyrics
```

Press `C-e` inside youtui to edit this file.

### Full default keybinds

See `?` inside the app for the complete keybind reference per context (global, playlist, browser, text_entry, filter, etc.).

---

## Build & Test

```sh
cargo build --release
./target/release/youtui

cargo test --release -p youtui --bin youtui
```

---

## Troubleshooting

| Problem | Fix |
|---|---|
| `Sign in to confirm you're not a bot` | Cookie expired. Refresh from `music.youtube.com`. Or set `downloader_type = "Native"` in config. |
| `Requested format is not available` | yt-dlp format changed. Try `downloader_type = "Native"` or update yt-dlp. |
| `App crashes on startup` | Missing deps: `sudo pacman -S yt-dlp alsa-lib ffmpeg` (Arch) or `sudo apt install ...` (Debian) |
| `No audio` | Check `yt-dlp --version` works. Reinstall with `sudo pacman -S yt-dlp`. |
| Album didn't split | Not in Last.fm, Discogs, or MusicBrainz. Plays as single video. No data loss. |
| Wrong track durations | YouTube video length differs from database entry. Gapless extraction uses the database durations. |

---

## Features

- **Vim keys** — j/k/h/l, C-b/C-f, g/G, no F-keys
- **Album splitting** — paste album URL, auto-split into tracks with accurate durations
- **Metadata pipeline** — 6-layer search (album → track → Discogs → MusicBrainz), year from Last.fm
- **Gapless playback** — seamless transitions between album tracks
- **Lyrics** — Musixmatch + Genius + Bandcamp pipeline
- **Annotations** — Genius annotation fragments in lyrics view
- **Romaji** — Japanese text → latin conversion (embedded IPADIC)
- **Scrobbling** — per-track scrobbling to Last.fm, persistent across all views
- **Dark Souls quit** — `q` shows YOU DIED
- **Copy URL** — `C-y` copies current song link
- **Open URL** — `:` paste any YouTube URL
- **Browser** — search songs, artists, playlists with vim-style navigation

---

## License

MIT
