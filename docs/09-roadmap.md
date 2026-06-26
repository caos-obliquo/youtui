# Roadmap

## Completed (2026-06-22 -- Day 1 + Day 2, 76 commits)

| # | Feature | Files |
|---|---------|-------|
| 1 | Library browser tab (playlist tracks, context menu, visual mode) | `library.rs`, `playlist.rs`, `keymap.rs` |
| 2 | ViTextEditor complete (65 tests: f/F/t/T/;/,/%, w/b/e/W/B/E, 0/$/gg/G, r/~/J/./C-r, text objects, surround) | `libs/vi-text-editor/` |
| 3 | Lyrics popup (visual mode, hybrid line numbers, pagination) | `lyrics_popup.rs` |
| 4 | Album art 1920x1080 HD, decode loop guard, throttle | `album_art_popup.rs` |
| 5 | Navigation hub (o->a/b, g->a/b, local search, go-to) | `songsearch.rs`, `albumsearch.rs`, `keymap.rs` |
| 6 | Keybind standard across all tabs | `keymap.rs` |
| 7 | Crossterm 0.29 migration, 15->0 warnings | All files |
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
| 19 | Playlist popups (rename, edit 4-field, details loading->display, save privacy) | `playlist_rename_popup.rs`, `playlist_edit_popup.rs`, `playlist_details_popup.rs`, `playlist_save_popup.rs` |
| 20 | Visual mode Shift+HJKL + arrows in VL/VC + lyrics | `vi-text-editor/`, `lyrics_popup.rs` |

## Completed (2026-06-22 -- Day 3 Polish, 32 commits)

| # | Feature | Files |
|---|---------|-------|
| 21 | ytmapi-cli (live queries: search/search-artists/search-albums/playlist/album/artist/library/fixture) | `libs/ytmapi-cli/` |
| 22 | Edit playlist 400 fix (privacy_status serialization) | `edit.rs` |
| 23 | Genius annotations gate removed (always try) | `messages.rs` |
| 24 | Album art centering (vertical) | `album_art_popup.rs` |
| 25 | Comprehensive docs (5.4k-line reference, man pages) | `docs/` |
| 26 | Final clean builds (0 warnings) | All workspace |

## Completed (2026-06-23 -- PlaylistSearch fix, locale, metadata-provider extraction)

| # | Feature | Files |
|---|---------|-------|
| 27 | ytmapi-rs locale parameterization (+3 tests) | `client.rs`, `auth.rs`, `lib.rs` |
| 28 | ytmapi-cli watch-playlist subcommand (Debug-First) | `ytmapi-cli/main.rs` |
| 29 | Metadata-provider crate extraction (19 tests, 8 files -> 1 crate) | `libs/metadata-provider/` (new) |
| 30 | Queue sort popup improvements (j/k nav, Enter/Esc, o.S) | `playlist.rs`, `keymap.rs` |
| 31 | Lyrics race guard (generation: u64 counter) | `app/ui.rs` |
| 32 | LRU lyrics cache + negative TTL (5-min error, cross-song) | `lyrics_popup.rs`, `app/ui.rs` |
| 33 | **CRITICAL: PlaylistSearch tab fixed** (deprecated->real types, dispatch wired, keybindings populated) | `action.rs`, `app/ui.rs`, `keymap.rs` |
| 34 | Recommendations o.r context menu (GetRelatedTracks -> WatchPlaylistQuery) | `messages.rs`, `effect_handlers_playlist.rs`, `songsearch.rs`, `library.rs`, `keymap.rs` |
| 35 | NavigationController: fix albumsearch GoToAlbum | `albumsearch.rs` |
| 36 | Library refresh fixes (4 missing playlists_fetched = false) | `app.rs` |
| 37 | Auth test infra (3 cookie path fallbacks) | `ytmapi-rs/tests/utils/mod.rs` |
| 38 | 9 stale #[allow(dead_code)] annotations removed | 4 files |
| 39 | Keybinding additions: o.q/o.L/o.Q/o.m/o.n (queue), o.r (library) | `keymap.rs` |
| -- | Enter = primary action (ncspot-style): NEVER opens sub-menu. Context via `o` only. | All browser files |

## Completed (2026-06-23 -- Mutation fixes, VL prefix, test suite)

| # | Feature | Files |
|---|---------|-------|
| 40 | **VL prefix behavior fixed**: 4 mutation PostQuery impls strip VL for delete/edit/add/rate. Browse endpoints keep VL. | `ytmapi-rs/src/query/playlist.rs`, `edit.rs`, `additems.rs`, `rate.rs` |
| 41 | **Playlist editor empty tracks fix**: checks `playlist_tracks` emptiness instead of always-populated `playlist_data` | `library.rs` |
| 42 | **Rate playlist 404 fixed**: `like/like` endpoint also needs VL stripped | `ytmapi-rs/src/query/rate.rs` |
| 43 | Tmux anti-music icon config | External |
| 44 | Full test suite passes | All crates |

## Completed (2026-06-24 -- Album art, footer, annotations, visual mode, ytmapi-cli, dead code)

| # | Feature | Files |
|---|---------|-------|
| 45 | Album art popup (o.v, full-screen sixel, pagination h/l cycle, centering fix) | `album_art_popup.rs`, `draw.rs`, `app.rs` |
| 46 | Footer restructure (5-line, album art 7-char, progress bar, status icons) | `footer.rs` |
| 47 | Lyrics popup improvements (Space pause, hint text, green lettering across tabs) | `lyrics_popup.rs`, `draw.rs` |
| 48 | Library tracks sort/filter/SEARCH + visual mode + delete re-route | `library.rs`, `messages.rs`, `keymap.rs` |
| 49 | Annotations integration (Tab/Alt+l/h focus switch, Enter copies, R romaji guard, vimline C-d/C-u) | `lyrics_popup.rs`, `annotations_popup.rs` |
| 50 | Visual mode cyan highlight + yank/paste (yank_buffer, clipboard) | `playlist.rs`, `draw.rs`, `keymap.rs` |
| 51 | Sixel belt-and-suspenders clear + centering (Protocol::area() offset) | `draw.rs`, `app.rs` |
| 52 | ytmapi-cli full rewrite (1426 lines, 44 commands, all 16 Innertube paths) | `libs/ytmapi-cli/` |
| 53 | ytmapi-rs 62 stale TODOs removed, sort order API, GetAlbumBrowseId, CLI --help | `ytmapi-rs/` |
| 54 | 35 clippy warnings fixed across 3 dep crates | `vi-text-editor`, `metadata-provider`, `genius-rs` |
| 55 | 17 stale `#[allow(dead_code)]` removed, 206 lines dead code deleted | `youtui/src/` |
| 56 | Library sort order UI (o.O, [A-Z]/[Z-A]/[Recent] title display) | `library.rs`, `messages.rs`, `keymap.rs` |
| 57 | Test gaps: GetAlbumBrowseId doc test + sort order cycle test (125/125 youtui) | `ytmapi-rs/`, `youtui/` |

## Completed (2026-06-25 -- Metadata cache, library album fix)

| # | Feature | Files |
|---|---------|-------|
| 58 | Metadata cache persistence (~/.local/share/youtui/metadata_cache.json, atomic write) | `libs/metadata-provider/` |
| 59 | ValidatedMetadata + AlbumTrack now Serialize/Deserialize | `metadata-provider/` |
| 60 | Library songs keep album data (HandleLibrarySongsOk maps album.name/id) | `library.rs` |
| 61 | Genre pipeline closed (genres/styles into ListSong, SongInfoPopup shows real genres) | `effect_handlers_playlist.rs` |
| 62 | CLI cache-test/cache-check subcommands | `ytmapi-cli/main.rs` |
| 63 | YTM album enrichment post-registry | `messages.rs`, `albumsearch.rs` |

## Completed (2026-06-25 -- Split pipeline revision, 13 commits on branch)

| # | Feature | Files |
|---|---------|-------|
| 64 | Split pipeline revision: word-boundary tag matching, album param on all 6 providers, last track duration | `playlist.rs`, `effect_handlers_playlist.rs`, `messages.rs`, `libs/metadata-provider/` |
| 65 | Metadata pipeline fixes: short date filter, zero-dur track filter (not reject), single-track albums (Last.fm/Discogs), cache threshold >=20, MA_COOKIE reorder | `libs/metadata-provider/` |
| 66 | Propagation: split tracks keep genres/styles/thumbnails/like_status from parent | `playlist.rs`, `effect_handlers_playlist.rs` |
| 67 | YTM enrichment best-effort (log warning, keep resolved data on failure) | `messages.rs` |
| 68 | Duration ratio split heuristic (video_dur / meta_total >= 0.3) with tag/10min/4-track fallbacks | `effect_handlers_playlist.rs` |
| 69 | Library album library_status propagation from GetAlbum API for correct o.t toggle | `albumsearch.rs` |
| 70 | Albums browser is_text_handling shadowing fix, count-in-header standardization | `albumsearch.rs`, all browser tabs |
| 71 | Metadata cache enrichment for library songs (deferred) | `messages.rs`, `library.rs` |
| 72 | ? toggle help (global keybind), o (Menu) in header, API setup URLs in help popup | `keymap.rs`, `header.rs`, `draw.rs` |
| 73 | Context menu descriptions clarified across all browser tabs | `library.rs`, `songsearch.rs`, `search_panel.rs` (all) |
| 74 | docs/api-services.md created, known-issues.md + album-splitting.md + roadmap.md refreshed | `docs/` |

## Completed (2026-06-25 -- Toggle fixes, annotations, audit, docs)

| # | Feature | Files |
|---|---------|-------|
| 75 | ToggleSubscribeArtist global (single o.S toggle, subscribed_artists HashSet) | `keymap.rs`, `albumsearch.rs` |
| 76 | RatePlaylist toggle bug fixed (insert→contains+remove/insert) | `albumsearch.rs`, `library.rs` |
| 77 | Annotation wrapping fix (Paragraph line-wrap counting in rendered_lines) | `lyrics_popup.rs` |
| 78 | ytmapi-rs test fix (get_library_artists sig, 25→1 deprecation warnings) | `ytmapi-rs/tests/` |
| 79 | Dead file/dead duplicate removed (async_rodio_sink.rs, rym-hierarchy.txt) | `youtui/src/`, `libs/` |
| 80 | Context menu awareness (is_song_action_visible per variant/sub-state) | `ui.rs`, `albumsearch.rs` |
| 81 | Phase 5: Related tracks yt-dlp enrichment (bounded 30/5 semaphore) | `messages.rs`, `effect_handlers_playlist.rs` |
| 82 | Docs hygiene: CLAUDE.md + TODO.md + docs/ synced (test counts, phases, 9→12 crates) | All docs |

## What Was Tried and Abandoned

| Attempt | Why Abandoned |
|---------|---------------|
| **AppCallback::Back** | Removed as dead code. Backspace works via `BrowserAction::Back` directly. |
| **Genius page scrape fallback** (no `__INITIAL_STATE__`) | DONE -- not abandoned. Direct scraping implemented in `genius-rs/src/scrape.rs`. |
| **Musixmatch integration** | DONE -- not abandoned. Multi-provider lyrics via `musixmatch-inofficial`. |
| **GetSavedEpisodes (ytmapi-rs)** | Committed then reverted (commit a066298). Feature needed but user chose not to pursue podcast content. |
| **Per-track ValidateMetadata for album splits** | Overwrote correct artist/album. Removed. Year still propagates. |
| **`url_added` flag** | Prevented URL-added song splitting. Removed. All sources split equally. |
| **Tag-only split gate** | Missed official label uploads without tags. Replaced with duration ratio heuristic. |
| **Substring tag matching** | "ep" matched "Epic". Replaced with word-boundary token matching. |
| **metal-api.dev (Metal Archives REST API)** | Returns 500 errors. Provider code written but unusable. Only MA_COOKIE works. |
| **OAuth token refresh** | Manual only. No refresh flow in youtui itself. |

## Current Test Suite

| Crate | Passed | Ignored |
|-------|--------|---------|
| youtui | 151 | 4 |
| metadata-provider | 47 | 0 |
| vi-text-editor | 65 | 0 |
| ytmapi-rs (lib) | 85 | 0 |
| genius-rs | 18 | 0 |
| async-callback-manager | 14 | 0 |
| json-crawler | 2 | 0 |
| ytmapi-cli | 7 | 0 |
| lrclib-rs | 4 | 0 |
| rym-genre-data | 10 | 0 |
| **Total** | **403** | **4** |

1 warning (pre-existing ytmapi-cli deprecation), 0 failures across workspace.

## Completed 2026-06-26 — Scrobbler + Suckless + PR #3 Perf

### Scrobbler Fixes (fix/scrobbler-signature branch)
| # | Item | Status | Files |
|---|------|--------|-------|
| 71 | params.sort_by() before HMAC signing (Last.fm alpha requirement) | ✅ | scrobbler.rs |
| 72 | Remove should_scrobble() guard on album tracks | ✅ | playlist.rs |
| 73 | scrobble_pending guard in play_song_id() and stop() | ✅ | playlist.rs |
| 74 | Remove rescrobbled spawn (no systemd duplicates) | ✅ | app.rs |
| 75 | 5 scrobbler unit tests | ✅ | scrobbler.rs |
| 76 | Persistent scrobble cache (save/retry/remove) | ✅ | scrobbler.rs, ui.rs |
| 77 | CLI test-scrobble tool | ✅ | querybuilder.rs, app.rs |
| 78 | Known: stop/disable rescrobbled systemd service | ✅ | docs |

### Suckless Refactoring (refactor/suckless branch, -630 lines)
| # | Item | Status | Files |
|---|------|--------|-------|
| 79 | Batch 1: Fix 6 panic paths | ✅ | api.rs, playlist.rs, shared_components.rs, keybind.rs, structures.rs, core.rs |
| 80 | Batch 2: Delete dead crates (metal-proxy, rym-definitions, -606 lines) | ✅ | Cargo.toml |
| 81 | Batch 3: Extract boilerplate (macro, conversion, thumbnail) | ✅ | effect_handlers_playlist.rs |
| 82 | Batch 4a: Subdivide MetadataEffect::apply (180→40 lines) | ✅ | effect_handlers_playlist.rs |
| 83 | Batch 4b: Split clean_title_for_metadata into 4 helpers | ✅ | playlist.rs |
| 84 | Batch 4d: Extract handle_force_split (75→1 line in apply_action) | ✅ | playlist.rs |

### PR #3 Performance Fixes (perf/enter-cancel-render + test coverage)
| # | Feature | Files |
|---|---------|-------|
| 85 | **Render throttle**: needs_redraw + 33ms interval, max ~30fps | `app.rs` |
| 86 | **Stale download cancel**: cancel_all_downloads() calls .cancel() on tokens | `playlist.rs` |
| 87 | **Enter-spam guard**: PlayDebouncer struct, 300ms cooldown | `app.rs` |
| 88 | **Library lazy iterator**: Box<dyn Iterator> instead of eager .collect() | `library.rs` |
| 89 | **Footer protocol cache**: cached_album_protocol skips re-encode on same art | `ui.rs`, `footer.rs` |
| 90 | **Help menu single-pass**: collect to [String; 3] once, reuse | `draw.rs` |
| 91 | **15 new unit tests**: PlayDebouncer (5), protocol cache (3), download cancel (3), library lazy (4) | 4 files |
| 92 | **invalidate_protocol_cache()** method on YoutuiWindow | `ui.rs` |

### PR #3 Test Coverage (perf/pr3-test-coverage branch)
| # | Feature | Files |
|---|---------|-------|
| 85 | **Enter-spam guard**: PlayDebouncer struct, 300ms cooldown | `app.rs` |
| 86 | **Stale download cancel**: cancel_all_downloads() calls .cancel() on tokens | `app/ui/playlist.rs` |
| 87 | **Library lazy iterator**: Box<dyn Iterator> instead of eager .collect() | `app/ui/browser/library.rs` |
| 88 | **Footer protocol cache**: cached_album_protocol skips re-encode on same art | `app/ui.rs`, `app/ui/footer.rs` |
| 89 | **Help menu single-pass**: collect to [String; 3] once, reuse | `app/ui/draw.rs` |
| 90 | **15 new unit tests**: PlayDebouncer (5), protocol cache (3), download cancel (3), library lazy iterator (4) | 4 files |
| 91 | **invalidate_protocol_cache()** method on YoutuiWindow | `app/ui.rs` |

## Medium Term

| # | Feature | Est | Notes |
|---|---------|-----|-------|
| 1 | Cross-platform clipboard | med | Wayland-only wl-copy. Add X11/macOS fallback. |
| 2 | Liked songs in browser tables | med | Parse like_status from YTM search, add column to AdvancedTableView in all tabs. |
| 3 | ytmapi-rs artist categories (5 TODOs) | med | Incomplete parse fields in GetArtist |
| 4 | Batch reorder (not just swap) in ytmapi-rs | large | `ytmapi-rs/` |
| 5 | **Footer FFT bars**: ringbuffer → `rustfft` → 1-line freq bars | med | `footer.rs`. Rust-only, no Cava dep. Cosmetic. |

## Crate Extraction Status

| # | Crate | Status | Tests |
|---|-------|--------|-------|
| 1 | ytmapi-rs | Extracted | 85 lib |
| 2 | json-crawler | Extracted | 2 |
| 3 | async-callback-manager | Extracted | 14 |
| 4 | vi-text-editor | Extracted | 65 |
| 5 | genius-rs | Extracted | 18 |
| 6 | ytmapi-cli | Extracted | 7 |
| 7 | metadata-provider | Extracted | 47 |
| 8 | lrclib-rs | Extracted | 4 |
| 9 | rym-genre-data | Extracted | 10 |
| 10 | metal-proxy | Removed (API down) | 0 |
| 11 | audio-player | Extracted ✅ | 0 |
| 12 | rym-definitions | Extracted (scraper) | 0 |

## Browser Tabs Feature Parity

| Tab | Search | Columns | Sort/Filter | o Menu | Navigation | Fully Wired |
|-----|--------|---------|-------------|--------|------------|-------------|
| Artists | F1 | Yes | Yes | o.S/o.U | g->a/g->b | Yes |
| Albums | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
| Songs | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
| Library | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
| PlaylistSearch | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
