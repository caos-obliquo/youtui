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

- `?` help toggle, `l` logs, `q` quit
- `1-6` for view switching (playlist, search, sort, filter, etc.)
- `j/k` up/down, `h/l` left/right
- `C-b`/`C-u` page up, `C-f`/`C-d` page down, `g`/`G` first/last
- `o` context menu (mode: Enter→Play, d→Delete)
- `d` delete selected, `D` delete all (direct, no Enter prefix)
- `e`/`E` add song(s) to existing playlist, `n` save new playlist
- Tab/Shift-Tab navigate search suggestions
- Config file at `~/.config/youtui/config.toml`

### Playlist & API

- **Save to new playlist**: `n` key, opens form popup for name/description
- **Add to existing playlist**: `e` (single song) / `E` (all songs), opens list popup
- **Context menu**: `o` key opens mode (bottom bar) with Play / Delete
- Popups use direct key routing in `handle_crossterm_event` before standard KeyRouter pipeline
- **Client version**: scraped from YouTube Music page, canary suffix stripped automatically

### Branches

| Branch | Purpose |
|---|---|
| `merge/friends-fork` | Main development branch |
| `fix/audio-ytdlp` | yt-dlp + vim keybinds (stable) |
| `fix/playlist-update-popup` | Cookie fix + popup improvements (current) |

### Done (this session)

- [x] Playlist creation 400 — fixed (canary version `-canary_control_` suffix stripped from scraped client version)
- [x] `d`/`D` delete direct — moved out of Enter mode to top-level playlist keybinds
- [x] `C-d` page down — added to list keybinds
- [x] `C-u` page up — added to list keybinds
- [x] `o` context menu — mode with Enter→Play, d→Delete (consistent with Enter's mode UX)
- [x] Tab/Shift-Tab search suggestion navigation
- [x] Debug logging removed from auth.rs

### Remaining

- [ ] **Lyrics multi-provider** — Genius fallback via `lyr` CLI when Musixmatch misses
- [ ] **Bandcamp lyrics CLI** — new crate to fetch lyrics from Bandcamp (no source exists today)
- [ ] **Scrobbling** — embed Rescrobbled natively (ListenBrainz / Maloja)
- [ ] **License review** — verify all dependency licenses, add proper attribution
- [ ] Plain-text config for easy editing
