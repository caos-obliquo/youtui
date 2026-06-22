# Roadmap

## Completed (2026-06-22 — Day 1 + Day 2, 76 commits)

| # | Feature | Files |
|---|---------|-------|
| 1 | Library browser tab (playlist tracks, context menu, visual mode) | `library.rs`, `playlist.rs`, `keymap.rs` |
| 2 | ViTextEditor complete (65 tests: f/F/t/T/;/,/%, w/b/e/W/B/E, 0/$/gg/G, r/~/J/./C-r, text objects, surround) | `libs/vi-text-editor/` |
| 3 | Lyrics popup (visual mode, hybrid line numbers, pagination) | `lyrics_popup.rs` |
| 4 | Album art 1920x1080 HD, decode loop guard, throttle | `album_art_popup.rs` |
| 5 | Navigation hub (o→a/b, g→a/b, local search, go-to) | `songsearch.rs`, `albumsearch.rs`, `keymap.rs` |
| 6 | Keybind standard across all tabs | `keymap.rs` |
| 7 | Crossterm 0.29 migration, 15→0 warnings | All files |
| 8 | Nerd icons removed (suckless) | 3 files |
| 9 | Genius annotations (unified list, scraping, JSON API, right panel, Enter seeks) | `genius-rs/`, `messages.rs`, `lyrics_popup.rs` |
| 10 | PlaylistEditor (:w/:q/:wq, :rename/:privacy/:rate) | `playlist_editor_popup.rs` |
| 11 | NavigationController (:cmd parser, skip URL album split) | `shared_components.rs`, `albumsearch.rs` |
| 12 | Lyrics n/p next/prev song, <> seek, ( ) nav, race guard, LRU cache | `lyrics_popup.rs`, `app/ui.rs` |
| 13 | Queue sort (o.r cycles columns) | `playlist.rs` |
| 14 | Genius-rs crate (CLI fetch/search/all/slug, 14 tests) | `libs/genius-rs/` (new) |
| 15 | Albums tab (replaced Playlists, table columns, sort/filter, YTM search, LRU cache) | `albumsearch.rs`, `draw.rs`, `keymap.rs` |
| 16 | 46 warnings eliminated, 10 ytmapi-rs fixtures regenerated | All files |
| 17 | Batch-merge o.M context menu | `library.rs`, `keymap.rs`, `playlist_update_popup.rs` |
| 18 | Config reload (:reload) + SeekTo callback | `app.rs`, `app/ui.rs` |
| 19 | Playlist popups (rename, edit 4-field, details loading→display, save privacy) | `playlist_rename_popup.rs`, `playlist_edit_popup.rs`, `playlist_details_popup.rs`, `playlist_save_popup.rs` |
| 20 | Visual mode Shift+HJKL + arrows in VL/VC + lyrics | `vi-text-editor/`, `lyrics_popup.rs` |

## Completed (2026-06-22 — Day 3 Polish, 32 commits)

| # | Feature | Files |
|---|---------|-------|
| 21 | ytmapi-cli (live queries: search/search-artists/search-albums/playlist/album/artist/library/fixture) | `libs/ytmapi-cli/` |
| 22 | Edit playlist 400 fix (privacy_status serialization) | `edit.rs` |
| 23 | Genius annotations gate removed (always try) | `messages.rs` |
| 24 | Album art centering (vertical) | `album_art_popup.rs` |
| 25 | Comprehensive docs (5.4k-line reference, man pages) | `docs/` |
| 26 | Final clean builds (0 warnings) | All workspace |

## Completed (2026-06-23 — This Session, uncommitted)

| # | Feature | Files |
|---|---------|-------|
| 27 | ytmapi-rs locale parameterization (+3 tests) | `client.rs`, `auth.rs`, `lib.rs` |
| 28 | ytmapi-cli watch-playlist subcommand (Debug-First) | `ytmapi-cli/main.rs` |
| 29 | Metadata-provider crate extraction (19 tests, 8 files → 1 crate) | `libs/metadata-provider/` (new) |
| 30 | Queue sort popup improvements (j/k nav, Enter/Esc, o.S) | `playlist.rs`, `keymap.rs` |
| 31 | Lyrics race guard (generation: u64 counter) | `app/ui.rs` |
| 32 | LRU lyrics cache + negative TTL (5-min error, cross-song) | `lyrics_popup.rs`, `app/ui.rs` |
| 33 | **CRITICAL: PlaylistSearch tab fixed** (deprecated→real types, dispatch wired, keybindings populated) | `action.rs`, `app/ui.rs`, `keymap.rs` |
| 34 | Recommendations o.r context menu (GetRelatedTracks → WatchPlaylistQuery) | `messages.rs`, `effect_handlers_playlist.rs`, `songsearch.rs`, `library.rs`, `keymap.rs` |
| 35 | NavigationController: fix albumsearch GoToAlbum | `albumsearch.rs` |
| 36 | Library refresh fixes (4 missing playlists_fetched = false) | `app.rs` |
| 37 | Auth test infra (3 cookie path fallbacks) | `ytmapi-rs/tests/utils/mod.rs` |
| 38 | 9 stale #[allow(dead_code)] annotations removed | 4 files |
| 39 | Keybinding additions: o.q/o.L/o.Q/o.m/o.n (queue), o.r (library) | `keymap.rs` |

## Immediate (Next Session)

| # | Feature | Est | Files |
|---|---------|-----|-------|
| 1 | `AppCallback::Back` — wire back navigation | small | `app.rs` |
| 2 | Genius annotations page scrape fallback (no `__INITIAL_STATE__`) | med | `genius-rs/`, `messages.rs` |
| 3 | Genius lyrics Musixmatch integration | small | `lyrics_popup.rs` |

## Medium Term

| # | Feature | Est | Notes |
|---|---------|-----|-------|
| 4 | audio-player crate extraction | large | Deep async_rodio_sink coupling |
| 5 | ytmapi-cli more fixture types + streaming tests | small | `ytmapi-cli/main.rs` |
| 6 | Rate toggle from details popup (parse like_status) | med | `messages.rs` |
| 7 | Batch reorder (not just swap) in ytmapi-rs | large | `ytmapi-rs/` |

## Crate Extraction Status

| # | Crate | Status | Tests |
|---|-------|--------|-------|
| 1 | ytmapi-rs | ✅ Extracted | 85 lib + 28/52 auth |
| 2 | json-crawler | ✅ Extracted | 8 |
| 3 | async-callback-manager | ✅ Extracted | 15 |
| 4 | vi-text-editor | ✅ Extracted | 65 |
| 5 | genius-rs | ✅ Extracted | 14 |
| 6 | ytmapi-cli | ✅ Extracted | 3 |
| 7 | metadata-provider | ✅ Extracted | 19 |
| 8 | audio-player | ❌ Blocked | Deep async_rodio_sink coupling |

## ViTextEditor — Complete ✅

65 tests, all pass. Full feature set. See `libs/vi-text-editor/`.

## Browser Tabs Feature Parity

| Tab | Search | Columns | Sort/Filter | o Menu | Navigation | Fully Wired |
|-----|--------|---------|-------------|--------|------------|-------------|
| Artists | ✅ F1 | ✅ | ✅ | ✅ o.S/o.U | ✅ g→a/g→b | ✅ |
| Albums | ✅ F1 | ✅ | ✅ | ✅ all actions | ✅ g→a/g→b | ✅ |
| Songs | ✅ F1 | ✅ | ✅ | ✅ all actions | ✅ g→a/g→b | ✅ |
| Library | ✅ F1 | ✅ | ✅ | ✅ all actions | ✅ g→a/g→b | ✅ |
| PlaylistSearch | ✅ F1 | ✅ | ✅ | ✅ all actions | ✅ g→a/g→b | **✅ FIXED** |
