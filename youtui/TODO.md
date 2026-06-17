# youtui (caos-obliquo fork)

## Architecture & Key Decisions

### Audio Download: yt-dlp with Cookie Auth

**Problem**: Native downloader (`rusty_ytdl`) gets 403 Forbidden errors from YouTube.

**Solution**: Switch to `yt-dlp` downloader with cookie-based authentication:

1. **Config**: `config.toml` sets `downloader_type = "YtDlp"` (default since our fork)
2. **Cookie file**: Browser auth stores cookie at `<config_dir>/cookie.txt`
3. **Cookie forwarding**: `YtDlpDownloader` passes `--cookies <path>` to yt-dlp subprocess
4. **PO token bypass**: yt-dlp uses `--extractor-args youtube:player_client=android_vr` to avoid PO token requirement

**Flow**: `main.rs` → `get_config_dir() + cookie.txt` → `RuntimeInfo.cookie_path` → `Server` → `SongDownloader` → `YtDlpDownloader` → yt-dlp `--cookies <path>`

### Keybindings (Vim/k9s-style)

| Key | Action |
|---|---|
| `1` | Playlist view |
| `2` | Song search |
| `3` | Artist search |
| `4` | Playlist search |
| `5` | Browser view |
| `6` | Change search type |
| `j`/`k` | Up / Down |
| `h`/`l` | Left / Right |
| `C-b`/`C-u` | Page up |
| `C-f`/`C-d` | Page down |
| `g`/`G` | First / Last |
| `d`/`D` | Delete selected / all |
| `y` | Lyrics popup (any view) |
| `c` | Category filter (artist albums) |
| `o` | Context menu |
| `s` | Shuffle |
| `A` | Set best quality |
| `n` | Save to new playlist |
| `e`/`E` | Add song(s) to existing playlist |
| `C-n`/`C-p` | Search suggestion navigation |
| `Esc` | Close search/filter/sort |
| `?` | Toggle help |
| `Space` | Play / Pause |
| `q` | Quit |

### Lyrics Architecture

```
y pressed -> GetLyrics(artist, title) backend task
  |
  +-> Musixmatch API (musixmatch-inofficial crate, no API key)
  |     |
  |     +-> success -> return lyrics
  |     |
  |     +-> Error::NotFound or error -> try lyr CLI
  |
  +-> lyr CLI (supports Genius, AZLyrics, JahLyrics, Musixmatch)
  |     |
  |     +-> tries 6 artist/title variants for fuzzy matching:
  |     |     - original (full artist string)
  |     |     - first artist only
  |     |     - first 2 artists joined with " and "
  |     |     - normalized title (lowercase, collapse spaces)
  |     |     - normalized artist
  |     |     - normalized both
  |     |
  |     +-> configured via ~/.config/lyr/config.toml (Genius first)
  |
  +-> results returned to LyricsPopup -> display with j/k scroll
```

### Playlist & API

- **Save to new playlist**: `n` key, opens form popup for name/description
- **Add to existing playlist**: `e` (single song) / `E` (all songs), opens list popup
- **Context menu**: `o` key opens mode (bottom bar) with Play/Delete/Lyrics
- **Lyrics**: `y` key in any view (playlist, song search, artist albums, playlist search)
- **Category filter**: `c` key to cycle All/Albums/EPs/Singles in artist album view
- Popups use direct key routing in `handle_crossterm_event` before standard KeyRouter pipeline
- **Client version**: scraped from YouTube Music page, canary suffix stripped automatically
- **Performance**: redraw only after events processed (no busy-loop rendering)

### Branches

| Branch | Purpose |
|---|---|
| `merge/friends-fork` | Main development branch |
| `feat/lyrics` | Lyrics popup with musixmatch-inofficial |
| `feat/esc-close-search` | Esc close + category filter + C-n/C-p + all fixes |

### Done (this session)

- [x] Playlist creation 400 — canary version suffix stripped, `"user":{}` placement fixed
- [x] `d`/`D` delete direct — moved out of Enter mode to top-level playlist keybinds
- [x] `C-d` page down — added to list keybinds
- [x] `C-u` page up — added to list keybinds
- [x] `o` context menu — mode with Enter->Play, d->Delete (consistent with Enter's mode UX)
- [x] Tab/Shift-Tab search suggestion navigation
- [x] Debug logging removed from auth.rs
- [x] **Lyrics popup** — `y` key, async fetch via musixmatch-inofficial (no API key), scrollable
- [x] **Lyrics from all views** — Playlist, SongSearch, ArtistAlbums, PlaylistSearch
- [x] **Multi-provider lyrics** — Musixmatch -> lyr (Genius/AZLyrics/JahLyrics) fallback
- [x] **Fuzzy matching** — 6 artist/title variants including normalization
- [x] **lyr configured** — `~/.config/lyr/config.toml` with Genius priority
- [x] **Scrollable lyrics popup** — j/k/Up/Down navigation with indicator
- [x] **Esc closes browser search** — BrowserSearchAction::Close + Esc keybind
- [x] **C-n/C-p search nav** — emacs-style, replaces j/k/Tab for suggestions
- [x] **Category filter** — `c` key, cycle All/Albums/EPs/Singles in artist album view
- [x] **Singles/EPs parsing fix** — `ArtistTopReleaseCategory::Singles => ()` was silently discarding data
- [x] **Keyword-based section matching** — replaces serde enum with contains() matching (handles localized headers like "Singles e EPs")
- [x] **Performance** — redraw only after events processed (eliminated busy-loop)
- [x] **Rebuild filtered_cache** — category filter actually filters displayed items, not just count
- [x] **cur_selected clamped** — after filter change, selection clamped to filtered list bounds
- [x] **Optimized release build** — `cargo install --path . --force` for global `youtui` command
- [x] **Zero compiler warnings** — 6 warnings eliminated
- [x] **Direct Genius scrape** — bypasses lyr exact-matching issues, uses Genius search API + page scrape
- [x] **Merged to main** — all feature branches merged to `main`
- [x] **Dark Souls quit screen** — `q` shows "YOU DIED" overlay with "y/N" prompt
- [x] **Config editor** — `C-e` opens config.toml in TextArea (Ctrl+s save, Esc cancel)
- [x] **Playlist category filter** — `c` key in playlist view (All/Albums/EPs/Singles)
- [x] **Bandcamp lyrics CLI** — `bandcamp-lyrics` crate (suckless: ureq + scraper, no tokio)
- [x] **Dark Souls quit screen** — full red-bordered "YOU DIED" panel with y/N
- [x] **Genius lyrics fix** — multi-container scrape, HTML entity decode, paren merging
- [x] **Vim search** — `n`/`N` navigate search matches, match counter in title

### Remaining

- [ ] **YouTube fallback** — fetch non-YTMusic tracks from YouTube for metadata + scrobbling
- [ ] **Vi-mode text editing** — `i`/`Esc`/`dd`/`dw`/`db`/`w`/`b`/`0` for search boxes
- [ ] **Genius annotations** — fetch highlighted annotations per song
- [ ] **Native scrobbling** — Last.fm API integration in-app
- [ ] **License review** — verify all dependency licenses

### Configs

| File | Purpose |
|---|---|
| `~/.config/youtui/config.toml` | youtui keybinds, auth type, downloader |
| `~/.config/lyr/config.toml` | lyr fetcher order (Genius first) |
| `~/.config/youtui/cookie.txt` | Browser auth cookie |
