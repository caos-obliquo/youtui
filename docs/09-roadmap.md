# Roadmap

## Immediate (Next Session)

| # | Feature | Est | Files |
|---|---------|-----|-------|
| 1 | Queue sort (`o.s` popup) | med | `playlist.rs`, `keymap.rs` |
| 2 | Race guard (`generation: u64` on lyrics/validation) | med | `messages.rs`, `effect_handlers_playlist.rs` |
| 3 | Inflight dedup (`HashSet` for lyrics requests) | med | `messages.rs` |
| 4 | LRU lyrics cache with negative TTL | med | `messages.rs`, `lyrics_popup.rs` |
| 5 | `Enter` on timestamp line seeks (DONE in 2026-06-21) | small | `lyrics_popup.rs` |
| 6 | Annotations right-side panel (DONE in 2026-06-21) | med | `lyrics_popup.rs` |
| 7 | Config reload (`:reload`) (DONE in 2026-06-21) | small | `app.rs`, `app/ui.rs` |

## Short Term

| # | Feature | Est | Notes |
|---|---------|-----|-------|
| 8 | NavigationController struct | small | Centralize GoToArtist/GoToAlbum (kopuz) |
| 9 | Recommendations (`o.r` context menu) | med | New `GetRelatedTracks` backend task |
| 10 | Library refresh fixes | small | Already exists as `r` key, review behavior |

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
