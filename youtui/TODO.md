# youtui (caos-obliquo fork)

## User Preferences (Strict)
- **No sudo** without explicit permission. Never run sudo commands automatically.
- **No AUR.** Only official repos + local compilation.
- **Suckless.** Minimal deps, focused scope, ASCII-only words, no bloat.
- **Rust only.** No shell plugins, no non-Rust dependencies. zsh-vi-mode is conceptual reference only.
- **Subagent stack**: `rustacean` for Rust code review, `akita` for architecture/tooling decisions.

## Architecture & Key Decisions

### Key Mapping Principle
- **Direct keys reserved for vim motions/operators only**: `j/k/h/l`, `gg/G`, `d/y/V/u/n/N`, `[/]`, `g` prefix, `/` search, `Esc` clear search
- **Context menu (`o` mode) holds everything else**: shuffle, repeat, like, quality, filters, delete all, song info, album cover, go to artist/album, save to playlist
- Rationale: muscle memory keys consistent; app actions discovered via `o` (never guessed)

### F-Key System
- F1 = toggle native YTM search, F2 = toggle Browser, F3 = toggle Queue
- F7 = ChangeSearchType (Browser), F11 = ViewLogs

### Search Split
- F1 = backend API search with YTM suggestions (SearchBlock)
- `/` = in-memory fuzzy filter (ViTextEditor, no API)

## ViTextEditor Status
- 41 tests, 41 pass, 0 fail
- Features: d/y/p/P/C-r/./J/~ operators, % match, text objects iw/aw/i(/a(/i"/a"
- f/F/t/T motions, ;/, repeat, r replace, count prefix, visual mode (V/v with visual_start)
- `yy` yank line works, visual mode operates on selection range (not entire buffer)
- `set_text`/`clear` reset both undo and redo stacks

## Tests
- youtui: 122 pass, 0 fail, 4 ignored
- ViTextEditor: 41/41 pass

## Done This Session
- Config parse fix: `o." "` → `o.s`, dead actions removed, `browser_library.*` → `browser_songs.*`
- Footer: square album art (7×6), `+`/`-` volume buttons removed, double "Album:" prefix stripped
- ViTextEditor bugs: `test_match_bracket_nested`, `yy` yank line, `visual_start` field, `redo_stack` cleared
- F3 trap: `close_popup()` no longer leaves stale popup context in `prev_context`
- Line number width: fixed 3-digit minimum in lyrics popup
- Album art popup (`o.v`): full-window `ratatui_image`, replaces `xdg-open`
- Annotations unified list (kopuz): `Vec<LyricLine>` + `line_is_background`, `j/k/{/}/gg/G` traverse all
- Context menu: `o.a`/`o.b` GoToArtist/GoToAlbum in all browser views
- `o.r` → `o.l` in context menus, direct `r` lyrics key removed, `V` removed from lyrics popup
- `/` unified: Browser Library uses `LocalFilter` (not API Search)
- `BrowserPlaylistSongsAction` context mislabel fixed

## Remaining Work

### Cleanup
- [ ] Remove wide config entries (user config already clean)
- [ ] Config completeness audit

### ViTextEditor Future (from zsh-vi-mode + binvim comparison)
- [ ] Visual `o` exchange point/mark (tiny)
- [ ] `s`/`S` substitute (tiny)
- [ ] `D`/`C`/`Y` synonyms (tiny)
- [ ] `W`/`B`/`E` BIG-word motions (small)
- [ ] `want_col` field — preserve column across short lines (small)
- [ ] Nested-pair text-object depth-counter walk (small)
- [ ] `i'`/`a'`/`` i` ``/`` a` `` quote text objects (tiny)
- [ ] `MotionKind` enum — replace per-(op,motion) arms (med)
- [ ] Count prefix inside crate — two-slot `2d3w` (med)
- [ ] proptest invariants (med)
- [ ] Surround `cs`/`ds`/`ys` (large)
- [ ] Switch keyword `^A`/`^X` numbers + booleans (med)

### Kopuz-inspired Future
- [ ] `NavigationController` struct — centralize GoToArtist/GoToAlbum
- [ ] `fetch_gen` race guard — discard stale `GetLyrics`/`ValidateMetadata`
- [ ] Inflight dedup — `LYRICS_INFLIGHT: HashSet` + Drop guard
- [ ] LRU + persistent lyrics cache + negative TTL
- [ ] `Enter` on active lyric line seeks to timestamp

## Known Issues
- Album cover may disappear on tmux visual line mode (sixel protocol cleared)
- Annotations display: last entry may be cut off (lyrics_popup.rs height calc)
- `o.a` conflict: `browser_artist_songs` uses `o.a` = PlayAlbum (not GoToArtist). All other views use `o.a` = GoToArtist
- Native downloader 403 Forbidden (use yt-dlp downloader)
- Crossterm 0.29 `Event::Key` destructure mismatch

## Configs
| File | Purpose |
|---|---|
| `~/.config/youtui/config.toml` | youtui keybinds, auth type, downloader |
| `~/.config/lyr/config.toml` | lyr fetcher order (Genius first) |
| `~/.config/youtui/cookie.txt` | Browser auth cookie |
