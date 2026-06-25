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

### Session 2026-06-25 — Test gaps + dead_code + add-to-playlist + lowercase preserve

- **3 test holes filled**: resolve() integration (3 tests), metal_api parsing (6 tests), insert_album_tracks propagation (1 test)
- **#[allow(dead_code)] cleanup**: removed from scrobbler, api, actionhandler MouseHandler trait. Gated test-only builders with `#[cfg(test)]`
- **E button rename**: "Save Queue to Existing Playlist" → "Add Queue to Playlist" (always appends, no toggle)
- **normalize_artist_name**: preserves intentional lowercase (e.g. "data da morte"). Same fix in metal_api
- Test counts: youtui 136, metadata-provider 45, workspace ~357

### Session 2026-06-25 — Metadata Cache Persistence + Library Album Fix

- **Library songs now keep album data**: `HandleLibrarySongsOk` no longer drops `ts.album.name` and `ts.album.id` — Album column in library browser now shows real names from YTM API.
- **Genre pipeline loop closed**: `MetadataEffect::Validated` handler now copies `data.genres`/`data.styles` into `ListSong` — SongInfoPopup (`o.I`) shows real genres where providers return them.
- **Metadata cache persists to disk**: `~/.local/share/youtui/metadata_cache.json` — JSON file, atomic write via `.tmp`+rename. Loaded on startup, saved after each successful resolve. Survives restart.
- **youtui tests**: 134 passed (was 133 — 1 new `full_length_detected` test fixed via tag normalization)
- **CLI cache-test**: `ytmapi debug cache-test <artist> <title>` — verifies cache file write+reload end-to-end.

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

### Session 2026-06-24 (All 5 Batches Committed)
- **Batch A**: `:` colon routing in lyrics, annotations Bearer-first + pagination
- **Batch B**: Metadata scoring + Discogs artist filter, browser play triggers album split, artist/album normalization
- **Batch C**: Visual mode yank/paste (p), Esc exit visual mode, consistent color
- **Batch D**: Sixel belt-and-suspenders clear, heart spacing
- **Batch E**: Annotation visual mode (cyan highlight, cross-panel range clearing)
- Tests: 124 youtui, 35 metadata-provider, 324 workspace total

## Priority Order (next steps)

| # | Step | File(s) | Est |
|---|------|---------|-----|
| 1 | Annotations integration + colon in lyrics | DONE | |
| 2 | Visual mode cyan | DONE | |
| 3 | Genius annotations fallback (no token) | `genius-rs/src/annotations.rs` | med |
| 4 | Genius lyrics: Musixmatch/LRCLIB integration | new crate | med |
| 5 | YTM album provider in metadata pipeline | DONE | |
| 6 | Like album to library (YTM profile) | DONE | |
| 7 | Sixel album art persistence | DONE | |
| 8 | Album browser j/k when tracks shown inline | DONE | |
| 9 | Count-in-header standardization | DONE | |
| 10 | ytmapi-rs TODO cleanup (62/99 stale removed) | DONE | |
| 11 | Crate extraction: audio-player | new crate | large |

**Step 12**: ~~F7 back-nav (FIXED). `handle_change_search_type()` now calls `push_snapshot()`.~~

**Step 13**: `cargo build --release`, `cargo test --release`, verify no regressions.

## Blocked
- Cross-platform clipboard (Wayland-only `wl-copy` — low priority, sidequest)
- Config template syntax (`o.enter`/`enter.enter` 2 pre-existing test failures)
- YouTube API format drift (external issue)
- Crossterm 0.29 `Event::Key` destructure mismatch (pre-existing, not our changes)
- Related tracks metadata enrichment (YTM API doesn't return album/year)

## Known Gaps (Consistency)
- **Footer album art**: Fetches async, brief blank on song change. Cache helps but not instantaneous.
