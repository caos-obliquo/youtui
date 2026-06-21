# TODO

## Vision

Full vim-driven TUI for YouTube Music. Keyboard-only. No mouse.

**Vim motions = direct keys.** `j/k/h/l/g/G/d/y/V/u/n/N/[/]` are muscle memory — always direct, never buried in menus.

**Context menu = everything else.** API calls, toggles, settings, info views → `o` mode context menu. Never guess random direct keys.

**Reusable component crates.** ViTextEditor extracted to `libs/vi-text-editor/`. SearchBlock, ScrollingTable are future extraction candidates for Libre.fm, Bandcamp nameyourprice, embedded player.

**Suckless philosophy.** Minimal deps, focused scope, no bloat. Keyboard warrior stack: dwl, Arch, Neovim, Vimium, zsh-vi-mode.

## Architecture Decisions

### Key Mapping Principle
- **Direct keys reserved for vim motions/operators only**: `j/k/h/l`, `gg/G`, `d/y/V/u/n/N`, `[/]`, `g` prefix, `/` search, `Esc` clear search
- **Context menu (`o` mode) holds everything else**: shuffle, repeat, like, quality, filters, delete all, song info, album cover, go to artist/album, save to playlist
- Rationale: muscle memory keys consistent; app actions discovered via `o` (never guessed)

### Only views that may differ
- **Browser > Library** (4th tab) — unique layout with categories
- **Playlist/Queue** (F3) — core view, untouched
- Everything else (browser tabs 1-3, all popups) — must be consistent

### F-Key System
- F1 = toggle native YTM search (SearchBlock, everywhere)
- F2 = cycle Browser tabs / enter Browser
- F3 = toggle Queue/Playlist (prev_context restore)
- F7 = ChangeSearchType (Browser)
- F11 = ViewLogs

### Search Split
- F1 = backend API search with YTM suggestions (SearchBlock)
- `/` = in-memory fuzzy filter (ViTextEditor, no API)

## Done

### Phases A–M (All Implemented)
- **A**: Annotations cutoff fixed — `lyrics_popup.rs:547` added `.saturating_sub(1)`
- **B**: GoToArtist/Album moved to `o.a`/`o.b` in context menu (was broken `g` mode shadowed by list keybinds)
- **C**: Count prefix carries through modes — `5dd` deletes 5 items
- **D**: `delete_selected(count)` — deletes N items from current position
- **E**: ViTextEditor extracted to `libs/vi-text-editor/` — standalone crate, 12 tests pass
- **F**: Popup consistency verified — all use Cyan borders, ALL, Esc closes, j/k, footer hints
- **G**: `o.E` sends ALL queue IDs (not just current song), overwrite toggle (`O` key), gg/G vi motions in update popup, title shows `[Replace]`/`[Append]`
- **H**: Duplicate `r`/`l` lyrics removed — `r` direct key deleted, keep `o.l` only
- **K**: `e` motion (end of word), `c` operator (change = delete+insert), visual char mode (`v`)
- **M**: Non-vim direct keys moved to context menu — removed `s/A/c/D/z/;/I/E/Z/L` from direct, added to `o` mode: `o.s/A/c/D/I/z/t/E`

### Context menu (`o` mode) now has:
| Key | Action | Key | Action |
|-----|--------|-----|--------|
| enter | Play Selected | s | Toggle Shuffle |
| d | Delete Selected | A | Set Best Quality |
| l | View Lyrics | c | Toggle Category Filter |
| y | Copy Song URL | D | Delete All |
| v | View Album Cover | I | View Song Info |
| a | Go to Artist | z | Toggle Repeat |
| b | Go to Album | t | Toggle Like |
| E | Save to Existing Playlist | | |

## Remaining (for next session)

### Priority 1: Semantic conflicts
1. **`o.a` conflict** — playlist = GoToArtist, artist_songs browser = PlayAlbum. Pick one or rename.
2. **`o.A` conflict** — playlist = SetBestQuality, artist_songs = AddAlbumToPlaylist. Same fix.

### Priority 2: UI consistency
3. **Lyrics popup** — uses `top_anchored_rect()` instead of `centered_rect_fixed()`. Make it centered like all other popups.
4. **`centered_rect_fixed`** duplicated in 4 popup files — extract to shared utility.

### Priority 3: Config completeness
5. **`o.y`/`o.r` missing** from browser config.toml sections (songs, artist_songs, playlist_songs).
6. **Browser library** has no config.toml section — add it.
7. **`o.E` in config.toml playlist** — present in keymap.rs but verify it's in config.toml.

### Priority 4: Keymap routing
8. **Popups bypass keymap** — all route through raw `handle_key()` instead of `apply_action()` via keymap. Works but not extensible. Add keymap sections for update popup.

## Blocked
- Cross-platform clipboard (Wayland-only `wl-copy` — low priority, sidequest)
- Config template syntax (`o.enter`/`enter.enter` 2 pre-existing test failures)
- YouTube API format drift (external issue)
