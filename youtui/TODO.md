# youtui (caos-obliquo fork)

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

## User Preferences
- One feature per branch: implement → test → commit → user approves → merge to main
- Keyboard-only, vim muscle memory — no mouse
- Suckless: minimal deps, ASCII-only word boundaries, no bloat
- Album art square (7×6 like original youtui), no volume buttons in footer
- Relative line numbers with 3-digit fixed width
- Never trap user — always have escape routes (F2/F3/Esc)
- `o.v` should open full-window album art popup inside youtui (NOT browser)
- Annotations should use kopuz-inspired unified list model
- Visual mode on annotations too
- Rust only — no shell plugins, no non-Rust dependencies

## Remaining Work

### Current Branch (fix/config-space-key)
- [x] Config parse fix (`o." "` → `o.s`, dead actions removed)
- [x] Footer: square album art (7×6), volume buttons removed
- [x] ViTextEditor bugs fixed (test_match_bracket_nested, yy, visual mode, redo_stack)
- [x] F3 trap fix (close_popup no longer leaves stale context)
- [x] Line number 3-digit width in lyrics popup
- [x] Footer double "Album:" prefix stripped
- [x] Context menu: o.a/o.b GoToArtist/GoToAlbum in browser_library defaults
- [ ] o.a GoToArtist + o.b GoToAlbum in config.toml (repo + user) for songs, playlist_songs
- [ ] Album art full-window popup (o.v) — NOT browser, clean popup with ratatui_image
- [ ] Build + test + commit

### Annotations Navigation Fix (kopuz-inspired)
- [ ] Refactor annotations to background:true lyric lines with parent_line_index
- [ ] Add relative line numbers to unified list (3-digit width, dimmer background lines)
- [ ] Visual mode on annotations — V selects range in unified list, d/y work on both

### Cleanup (Phase 3)
- [ ] Remove r direct key for lyrics
- [ ] o.a/o.A conflict + o.r→o.l rename
- [ ] Remove wide config entries
- [ ] Config completeness audit

### ViTextEditor Future (from zsh-vi-mode + binvim comparison)
- [ ] Visual o exchange point/mark (tiny)
- [ ] s/S substitute (tiny)
- [ ] D/C/Y synonyms (tiny)
- [ ] W/B/E BIG-word motions (small)
- [ ] want_col field — preserve column across short lines (small)
- [ ] Nested-pair text-object depth-counter walk (small)
- [ ] i'/a'/i`/a` quote text objects (tiny)
- [ ] MotionKind enum — replace per-(op,motion) arms (med)
- [ ] Count prefix inside crate — two-slot 2d3w (med)
- [ ] proptest invariants (med)
- [ ] Surround cs/ds/ys (large)
- [ ] Switch keyword ^A/^X numbers + booleans (med)

### Kopuz-inspired Future
- [ ] NavigationController struct — centralize GoToArtist/GoToAlbum
- [ ] fetch_gen race guard — discard stale GetLyrics/ValidateMetadata
- [ ] Inflight dedup — LYRICS_INFLIGHT HashSet + Drop guard
- [ ] LRU + persistent lyrics cache + negative TTL
- [ ] Enter on active lyric line seeks to timestamp

## Known Issues
- Annotations: j/k/{/}/gg/G always control lyrics (no focus check) — fix via kopuz unified list
- Album cover may disappear on tmux visual line mode (sixel protocol cleared)
- o.v currently opens Browser instead of album art popup
- Native downloader 403 Forbidden (use yt-dlp downloader)
- Crossterm 0.29 Event::Key destructure mismatch

## Configs
| File | Purpose |
|---|---|
| `~/.config/youtui/config.toml` | youtui keybinds, auth type, downloader |
| `~/.config/lyr/config.toml` | lyr fetcher order (Genius first) |
| `~/.config/youtui/cookie.txt` | Browser auth cookie |
