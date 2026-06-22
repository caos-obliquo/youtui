# Roadmap

## Completed (2026-06-21)

| # | Feature | Files |
|---|---------|-------|
| 5 | `Enter` on timestamp line seeks | `lyrics_popup.rs` |
| 6 | Annotations right-side panel | `lyrics_popup.rs` |
| 7 | Config reload (`:reload`) | `app.rs`, `app/ui.rs` |
| 17 | Eliminate 46 youtui warnings (0 remaining) | All youtui files |
| 18 | Fix 10 ytmapi-rs fixture-drift test failures | 10 expected output files |
| 19 | Annotate planned dead code with `#[allow]` + TODO | 12 files |

## Completed (2026-06-22)

| # | Feature | Files |
|---|---------|-------|
| 20 | `libs/genius-rs` crate — search, HTML scrape, annotations CLI | `libs/genius-rs/` (new) |
| 21 | Genius lyrics pipeline: 403 fix via HTML scraping + `[Verse]` preservation | `messages.rs:659-710`, `scrape.rs` |
| 22 | Genius annotations: ALL annotations from `__INITIAL_STATE__` (no pagination) | `scrape.rs`, `messages.rs:919-966` |
| 23 | Fix `NavTarget::Album` → Albums tab with `GetAlbumQuery` | `browser.rs:525-529` |
| 24 | Wire F1 search in Albums tab (`TextHandler` delegate to `SearchBlock`) | `albumsearch.rs:276-279` |
| 25 | Wire `GoToAlbum` context menu in album track view | `albumsearch.rs:223-229` |
| 26 | Remove tracks from playlist (`o.x`) | `library.rs`, `app.rs`, `messages.rs` |
| 27 | Reorder tracks in playlist (`o.J`/`o.K`) | `library.rs`, `app.rs`, `keymap.rs` |
| 28 | Playlist Editor Popup (7 methods wired into dispatch) | `playlist_editor_popup.rs`, `app/ui.rs` |
| 29 | Albums draw quadrants consistency | `draw.rs` |
| 30 | Artist subscribe/unsubscribe (`o.S`/`o.U`) | `messages.rs`, `app.rs`, `library.rs`, `keymap.rs` |
| 31 | Back navigation wired | `app.rs` |

| # | Feature | Files |
|---|---------|-------|
| 5 | `Enter` on timestamp line seeks | `lyrics_popup.rs` |
| 6 | Annotations right-side panel | `lyrics_popup.rs` |
| 7 | Config reload (`:reload`) | `app.rs`, `app/ui.rs` |
| 17 | Eliminate 46 youtui warnings (0 remaining) | All youtui files |
| 18 | Fix 10 ytmapi-rs fixture-drift test failures | 10 expected output files |
| 19 | Annotate planned dead code with `#[allow]` + TODO | 12 files across messages, api, UI |

## Immediate (Next Session)

| # | Feature | Est | Files |
|---|---------|-----|-------|
| 1 | Queue sort (`o.s` popup) | med | `playlist.rs`, `keymap.rs` |
| 2 | Race guard (`generation: u64` on lyrics/validation) | med | `messages.rs`, `effect_handlers_playlist.rs` |
| 3 | Inflight dedup (`HashSet` for lyrics requests) | med | `messages.rs` |
| 4 | LRU lyrics cache with negative TTL | med | `messages.rs`, `lyrics_popup.rs` |

## Short Term

| # | Feature | Est | Notes |
|---|---------|-----|-------|
| 8 | NavigationController struct | small | Centralize GoToArtist/GoToAlbum (kopuz) |
| 9 | Recommendations (`o.r` context menu) | med | New `GetRelatedTracks` backend task |
| 10 | Library refresh fixes | small | Already exists as `r` key, review behavior |

## Visual Mode Enhancements

| # | Feature | Est | Status |
|---|---------|-----|--------|
| 11 | H/J/K/L + arrows in VisualChar mode | tiny | Done |
| 12 | Full VisualChar motion parity in VisualLine mode | small | Done |
| 13 | H/L/arrows + 0/$ in lyrics normal mode | tiny | Done |
| 14 | H/J/K/L/arrows/0/w/b/e in lyrics visual mode | small | Done |
| 15 | Phase 3: Rate toggle (parse like_status) | med | Blocked (needs details response field) |
| 16 | Phase 4: Reorder UI wiring | med | Blocked (needs setVideoId parsed) |

## Wiring Backlog (Remaining Dead Code)

Grep `TODO: Wire` for all remaining unconnected features:

| # | Feature | Files | Notes |
|---|---------|-------|-------|
| 20 | Playlist search tab | `messages.rs`, `api.rs` | New browser tab for playlist search |
| 21 | Batch playlist song streaming | `api.rs`, `messages.rs` | Stream all songs from playlist |
| 28 | Queue column sort | `playlist.rs` | Header click sort |
| 29 | Batch append for merge/reorder | `structures.rs` | Raw playlist item batch ops |
| 30 | Albums tab caching | `albumsearch.rs` | Cache flag + fetch trigger |
| 31 | Album search text input | `albumsearch.rs` | Search suggestions + routing |
| 32 | Lyrics filter | `lyrics_popup.rs` | Filter text in lyrics view |
| 34 | Artist subscription toggle | `library.rs` | Check current subscription status |

## Medium Term — Crate Extraction

| # | Crate | Files | Reason |
|---|-------|-------|--------|
| 11 | `search-block` | `browser/shared_components.rs` | Reuse for Libre.fm, Bandcamp |
| 12 | `scrolling-table` + `scrolling-list` | `widgets/` | Generic TUI widgets |
| 13 | `metadata-provider` trait | `app/server/providers/` | Swap YTM ↔ Bandcamp ↔ Libre.fm |
| 14 | `audio-player` | `app/server/player.rs` | Standalone gapless player crate |

## Long Term — Libre Source Stack

| # | Project | Description |
|---|---------|-------------|
| 15 | **Bandcamp name-your-price** | Metadata provider + scraper for Bandcamp |
| 16 | **Libre.fm client** | Scrobbling → full library management |
| 17 | **Embedded music player** | Via already-decoupled `TaskManager` + `DecodeSong` pipeline |

## ViTextEditor — All Complete ✅

Everything from the [zsh-vi-mode + binvim comparison](02-crates/vi-text-editor.md) has been implemented and tested (65 tests, all pass).

Items intentionally deferred:
- **MotionKind enum** — not needed since `apply_motion_op` helper already DRYed the operator-pending handler
- **Count prefix inside crate** — outer keymap system owns count routing; would conflict
