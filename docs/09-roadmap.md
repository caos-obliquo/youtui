# Roadmap

## Completed (2026-06-22 -- Day 1 + Day 2, 76 commits)

| # | Feature | Files |
|---|---------|-------|
| 1 | Library browser tab (playlist tracks, context menu, visual mode) | `library.rs`, `playlist.rs`, `keymap.rs` |
| 2 | ViTextEditor complete (67 tests: f/F/t/T/;/,/%, w/b/e/W/B/E, 0/$/gg/G/^, r/~/J/./C-r, / ? search, text objects, surround) | `libs/vi-text-editor/` |
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
| youtui | 164 | 4 |
| metadata-provider | 48 | 0 |
| vi-text-editor | 67 | 0 |
| ytmapi-rs (lib) | 82 | 0 |
| genius-rs | 18 | 0 |
| async-callback-manager | 14 | 0 |
| json-crawler | 2 | 0 |
| lrclib-rs | 4 | 0 |
| rym-genre-data | 10 | 0 |
| audio-player | 0 | 0 |
| **Total** | **409** | **4** |

0 failures, 0 build warnings across 10 workspace crates.

## Completed 2026-06-26 - Scrobbler + Suckless + PR #3 Perf

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

### PR #7 - Background scrobble retry + rate limit handling
| # | Feature | Files |
|---|---------|-------|
| 92 | ScrobbleResult enum (Success/Failure/RateLimited) | `scrobbler.rs` |
| 93 | Rate limit stops retry loop (error 29) | `scrobbler.rs` |
| 94 | Background 5-min retry loop in main event loop | `app.rs` |
| 95 | 2s delay between retries | `scrobbler.rs` |
| 96 | Max cache size: 200 entries (oldest evicted) | `scrobbler.rs` |
| 97 | 5 cache unit tests (roundtrip, max size, retry increment, drop expired, legacy default) | `scrobbler.rs` |

### PR #8 - Protocol cache chunk dimension tracking + debug logging
| # | Feature | Files |
|---|---------|-------|
| 98 | Chunk dimension tracking prevents 8-bit fallback on terminal resize | `footer.rs`, `ui.rs` |
| 99 | Debug logging for terminal Picker detection | `ui.rs` |

### PR #9 - o.v zero-pixel image guard
| # | Feature | Files |
|---|---------|-------|
| 100 | Zero-pixel in_mem_image guard shows 'No image data' fallback | `playlist.rs` |

### PR #10 - Doc hygiene (35+ stale refs fixed)
| # | Feature | Files |
|---|---------|-------|
| 101 | Test counts, line counts, scrobbler doc fixed across 7 files | 7 doc files |

### PR #11 - Album scrobble consistency
| # | Feature | Files |
|---|---------|-------|
| 102 | Album-mode scrobble: hardcoded None → reads current song album | `playlist.rs` |
| 103 | ValidateMetadata enrichment gate removed (runs even when album=None) | `messages.rs` |
| 104 | ScrobbleState refreshes album every 100ms progress check | `playlist.rs` |
| 105 | 5s album wait before scrobble for async metadata arrival | `playlist.rs` |
| 106 | submit_now_playing() on song start | `scrobbler.rs`, `playlist.rs` |

### PR #12 - Liked songs column + version bump
| # | Feature | Files |
|---|---------|-------|
| 107 | LikeStatus in ListSongDisplayableField (new variant) | `structures.rs` |
| 108 | Parsed from YTM search results (SearchResultSong.like_status) | `search.rs` |
| 109 | Heart column shown in all 5 browser tabs, sortable | 5 browser files |
| 110 | Version bump 0.0.37 → 1.0.0, CHANGELOG | `main.rs` |

### PR #13 - CLI sort flags
| # | Feature | Files |
|---|---------|-------|
| 111 | --sort arg for 9 library/upload CLI commands (closes 9 TODOs) | `main.rs`, `cli/` |

### PR #14-#15 - Liked column layout + UTF-8 crash fix + queue liked column
| # | Feature | Files |
|---|---------|-------|
| 112 | get_layout() constraint added for Liked column in all browser tabs | 5 browser files |
| 113 | Queue (playlist) gets Liked column | `playlist.rs` |
| 114 | UTF-8 crash fix: 6 cursor+=1 bugs fixed with len_utf8() | `vi-text-editor/` |
| 115 | Full heart icon in liked column | `structures.rs` |

### PR #16 - Audio cache (repeat Enter re-download fix)
| # | Feature | Files |
|---|---------|-------|
| 116 | HashMap<video_id, Arc<InMemSong>> survives reset() | `playlist.rs` |
| 117 | Max 50 entries, full clear on overflow | `playlist.rs` |

### PR #17 - Batch playlist streaming
| # | Feature | Files |
|---|---------|-------|
| 118 | get_playlist_songs() uses stream_api_with_retry_n instead of single page | `api.rs` |
| 119 | max_pages from max_results/100 (clamped 1-50) | `api.rs` |

### PR #18 - drawutils cleanup
| # | Feature | Files |
|---|---------|-------|
| 120 | bottom_of_rect: saturating arith prevents underflow panic | `drawutils.rs` |
| 121 | below_left_rect: clamp x/y to max_bounds | `drawutils.rs` |
| 122 | 3 new tests (basic/narrow/zero-width) | `drawutils.rs` |

### PR #19 - View-indices sort refactor
| # | Feature | Files |
|---|---------|-------|
| 123 | view_indices: Vec<usize> maintains sort order separate from backing list | `songsearch.rs`, `playlistsearch/`, `artistsearch/` |
| 124 | clear_sort_commands() resets to identity (restores fetch order without re-fetch) | 3 files |
| 125 | 3 TODO comments removed | 3 files |

### PR #27 - ytmapi-rs regression fix (5 regressions from working tree slimming)
| # | Feature | Files |
|---|---------|-------|
| 131 | **Auth fix**: restored `parse_netscape_cookies()` - Netscape cookie format from yt-dlp needs parsing before reqwest Cookie header | `auth/browser.rs` |
| 132 | **EP/singles fix**: case-insensitive `contains()` matching for carousel section titles - `categorize_top_release()` was deleted, Singles/EPs never processed | `parse/artist.rs` |
| 133 | **reqwest 0.13.3 → 0.11**: TLS broken in 0.13.3, reverted | `Cargo.toml` |
| 134 | **VL prefix stripping restored**: 5 mutation files had stripping removed - all mutation ops on VL playlists would fail 400/404 | 5 query files |
| 135 | **RemovePlaylistItems endpoint fixed**: `browse/edit_playlist` → `playlist/edit` | `query/playlist.rs` |
| 136 | **ytmapi-rs slimming**: +804/-2107 lines across 60 files. ytmapi-cli removed from workspace. Simplified queries reduced. Auth consolidated. Test fixtures regenerated. | ytmapi-rs/ |
| 137 | **ytmapi-rs lib tests**: 85→82 (3 locale `with_language`/`with_location` tests removed) | test files |

### PR #20 - ytmapi-rs artist categories
| # | Feature | Files |
|---|---------|-------|
| 126 | ArtistTopReleaseCategory made pub enum (was private) | `parse/artist.rs` |
| 127 | GetArtistAlbumsAlbum.category: Option<String> → Option<ArtistTopReleaseCategory> | `parse/artist.rs` |
| 128 | Videos/Related/Playlists carousel arms wired | `parse/artist.rs` |
| 129 | GetArtistTopReleases.playlists: new field | `parse/artist.rs` |
| 130 | ytmapi-rs lib tests: 76/85 → 85/85 (+9) | test output files |

## Medium Term

| # | Feature | Est | Notes |
|---|---------|-----|-------|
| 1 | Cross-platform clipboard | med | ✅ DONE - fallback chain + cookie_browser config + Windows block |
| 4 | Batch reorder (not just swap) in ytmapi-rs | large | `ytmapi-rs/` |
| 5 | View-only struct refactor for browser tabs | low | DONE (PR #19) |
| 6 | ytmapi-rs artist categories (5 TODOs) | med | DONE (PR #20) |


## Crate Extraction Status

| # | Crate | Status | Tests |
|---|-------|--------|-------|
| 1 | ytmapi-rs | Extracted | 82 lib |
| 2 | json-crawler | Extracted | 2 |
| 3 | async-callback-manager | Extracted | 14 |
| 4 | vi-text-editor | Extracted | 67 |
| 5 | genius-rs | Extracted | 18 |
| 6 | metadata-provider | Extracted | 48 |
| 7 | lrclib-rs | Extracted | 4 |
| 8 | rym-genre-data | Extracted | 10 |
| 9 | metal-proxy | Removed (API down) | 0 |
| 10 | audio-player | Extracted ✅ | 0 |
| 11 | rym-definitions | Removed (merged into rym-genre-data) | 0 |

## Browser Tabs Feature Parity

| Tab | Search | Columns | Sort/Filter | o Menu | Navigation | Fully Wired |
|-----|--------|---------|-------------|--------|------------|-------------|
| Artists | F1 | Yes | Yes | o.S/o.U | g->a/g->b | Yes |
| Albums | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
| Songs | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
| Library | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
| PlaylistSearch | F1 | Yes | Yes | all actions | g->a/g->b | Yes |
