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

### ViTextEditor Steps 0–2
- **0**: Delete stale `components/vi_text_editor.rs` (unused duplicate)
- **1**: `f`/`F`/`t`/`T` motions + `;`/`,` repeat
- **2**: `r` replace single char

### Session 2026-06-24 (Committed)
- Footer heart icon, 5-line Status block, album art 7-char wide
- Nerd Font MDI icons: repeat `󰑖`/`󰑗`/`󰑘`, shuffle `󰒝`, heart `󰋑`
- Library tracks Phase C+D: sort/filter popups via o.z/o.c, [SEARCH] indicator
- Like/subscribe/unsubscribe from album tracks view (o.t/o.S/o.U)
- Force-split (o.f) + playlist editor overwrite save
- Album URL auto-detection (OLAK5uy_ via playlist?list=)
- Green lettering for playing song across ALL browser tabs
- Album art popup (o.v): 95% centered, sixel data stored for cleanup
- Metadata pipeline: resolver scoring, Discogs fix, url_added removed, per-track validation removed
- 29 new tests (youtui: 103→124)

## Priority Order (next steps)

| # | Step | File(s) | Est |
|---|------|---------|-----|
| 1 | **Annotations integration + `:` in lyrics** | `lyrics_popup.rs`, `app.rs` | med |
| 2 | **Visual mode cyan** (queue: all lines cyan, no green-on-nonplaying) | `playlist.rs`, `view/draw.rs` | small |
| 3 | `C-r` redo (commit) | `libs/vi-text-editor/src/lib.rs` + 6 callers | ✓ ready |
| 4 | `.` repeat last change | `libs/vi-text-editor/src/lib.rs` | med |
| 5 | `J` join lines | `libs/vi-text-editor/src/lib.rs` | small |
| 6 | `~` toggle case | `libs/vi-text-editor/src/lib.rs` | small |
| 7 | Lyrics hybrid line numbers | `lyrics_popup.rs` | med |
| 8 | Like album to library (add to YT Music profile) | `albumsearch.rs` + ytmapi-rs | med |
| 9 | Sixel album art persistence | `draw.rs` | med |
| 10 | Remove wide config | `~/.config/youtui/config.toml` | tiny |
| 11 | Text objects iw, i(, a(, i", a" | `libs/vi-text-editor/src/lib.rs` | large |
| 12 | `%` bracket match | `libs/vi-text-editor/src/lib.rs` | med |
| 13 | Build + full test suite | verify | verify |

### Step details

**Step 1**: Annotations integration + `:` in lyrics. Annotations display end-to-end with GENIUS_TOKEN. `:` Opens URL from lyrics popup (currently blocked by key interception). Add `AppCallback::OpenUrlCommand`, route through lyrics_popup `handle_key`.

**Step 2**: Visual mode cyan. `view/draw.rs` — change `visual_range_style` to cyan bg. `playlist.rs` — fix `get_highlighted_row()` to not return playing index when visual mode active. All highlighted lines cyan, no green-on-nonplaying.

**Step 3**: `C-r` redo. `handle_key` API gains `ctrl: bool` param. `undo()` pushes to `redo_stack`. New `redo()` method. Internal tests updated.

**Step 4**: `.` repeat last change. Store last edit (insert/delete/change/replace) as `LastChange` enum. On `.` press, replay it.

**Step 5**: `J` join lines. `buffer.remove(cursor)` if next char is `\n`, replacing with ` `.

**Step 6**: `~` toggle case at cursor. ASCII `a-z` ↔ `A-Z`.

**Step 7**: Hybrid line numbers in lyrics popup. `abs_line == cursor` show absolute, else show relative offset. Dim `Color::DarkGray`. Both render paths (side-by-side + full-width). `max_digits` from total line count.

**Step 8**: Footer album format. `footer.rs:98-101` — construct `format!("{artists} - {album}")` composite string instead of artist/album on separate lines.

**Step 9**: `~/.config/youtui/config.toml` — revert custom keybinds to clean defaults. Remove "wide" overrides.

**Step 10**: Text objects. `iw` inner word, `i(`/`i)` inner parens, `a(`/`a)` a parens (including parens), `i"`/`a"` inner/a string. Works with `d`, `c`, `y` operators.

**Step 11**: `%` bracket match. `([{}])` — find matching pair. Forward/backward cursor move.

**Step 12**: ~~F7 back-nav (FIXED). `handle_change_search_type()` now calls `push_snapshot()`.~~

**Step 13**: `cargo build --release`, `cargo test --release`, verify no regressions.

## Blocked
- Cross-platform clipboard (Wayland-only `wl-copy` — low priority, sidequest)
- Config template syntax (`o.enter`/`enter.enter` 2 pre-existing test failures)
- YouTube API format drift (external issue)
- Crossterm 0.29 `Event::Key` destructure mismatch (pre-existing, not our changes)
- Related tracks metadata enrichment (YTM API doesn't return album/year)

## Known Gaps (Consistency)
- **Library tab missing playing indicator**: No second highlight showing which song is currently playing
- **Footer album art**: Fetches async, brief blank on song change. Cache helps but not instantaneous.
