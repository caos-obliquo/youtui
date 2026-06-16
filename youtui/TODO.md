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
- `C-b`/`C-f` page up/down, `g`/`G` first/last
- `e`/`E` add song(s) to existing playlist, `n` save new playlist
- Config file at `~/.config/youtui/config.toml`

### Playlist Features

- **Save to new playlist**: `n` key, opens form popup for name/description
- **Add to existing playlist**: `e` (single song) / `E` (all songs), opens list popup
- Popups use direct key routing in `handle_crossterm_event` before standard KeyRouter pipeline

### Branches

| Branch | Purpose |
|---|---|
| `merge/friends-fork` | Main development branch |
| `fix/audio-ytdlp` | yt-dlp + vim keybinds (stable) |
| `fix/playlist-update-popup` | Cookie fix + popup improvements (current) |

### Remaining

- [ ] Context menu (`o` key, ncspot-style)
- [ ] Search: retrieve EPs and singles in browser
- [ ] Playlist creation 401 — cookie expired, need fresh cookie or debug OAuth (didn't work before)
- [ ] Plain-text config for easy editing
