# youtui (caos-obliquo fork)

## What's DONE

### 3-Day Sprint (2026-06-19 → 2026-06-22) — 108 commits

#### Day 1: Foundation
- Library browser tab (playlist tracks, context menu, visual mode)
- ViTextEditor complete (65 tests: f/F/t/T, %, w/b/e/W/B/E, 0/$, gg/G, r/~, J/., C-r, iw/aw/i(/a(/i"/a", surround)
- Lyrics popup (visual mode, hybrid line numbers, pagination)
- Album art 1920x1080 HD, decode loop guard, throttle
- Navigation hub (o->a/b, g->a/b, local search, go-to)
- Keybind standard (consistent across all tabs)
- Crossterm 0.29 migration, 15->0 warnings
- Nerd icons removed (suckless)

#### Day 2: Features
- Genius annotations (unified list, scraping, JSON API, right panel)
- PlaylistEditor (:w/:q/:wq, :rename/:privacy/:rate)
- NavigationController (:cmd parser, skip URL album split)
- Lyrics (n/p next/prev song, <> seek, ( ) nav, race guard, LRU cache)
- Queue sort (o.r cycles columns)
- Playlist popups (rename, edit 4-field, details loading->display, save privacy)
- Visual mode Shift+HJKL + arrows
- Config reload (:reload), SeekTo callback
- Genius-rs crate (CLI fetch/search/all/slug, 14 tests)
- Albums tab (replaced Playlists, table columns, sort/filter, YTM search, LRU cache)
- PlaylistSearch tab (F1 search, dual panel: search list + songs)
- 46 warnings eliminated, 10 ytmapi-rs fixtures regenerated
- Batch-merge o.M context menu

#### Day 3: Polish + Wiring
- ytmapi-cli (live queries: search, search-artists, search-albums, playlist, album, artist, library, fixture)
- Edit playlist 400 fix (privacy_status serialization)
- Genius annotations gate removed (always try)
- Album art centering
- Comprehensive docs (5.4k-line reference manual, man pages)

#### Session 2026-06-23 (Committed)
- ytmapi-rs locale parameterization (language/location, builder methods, 3 tests)
- ytmapi-cli watch-playlist subcommand (Debug-First compliance)
- Metadata-provider crate extraction (19 tests, 0 warnings)
- Queue sort popup improvements (j/k nav, Enter/Esc, o.S)
- Lyrics race guard (generation: u64 counter)
- LRU lyrics cache + negative TTL (5-min error cache, cross-song)
- **CRITICAL: PlaylistSearch tab fixed** - deprecated no-op types replaced with real dispatch
- Recommendations o.r context menu (GetRelatedTracks -> GetWatchPlaylistQuery)
- NavigationController: fix albumsearch.rs GoToAlbum
- Library refresh: 4 missing playlists_fetched = false paths
- Auth test infra: 3 cookie path fallbacks
- 9 stale #[allow(dead_code)] annotations removed
- Keybinding additions: o.q/o.L/o.Q/o.m/o.n (queue), o.r (library)
- Dead code cleanup

#### Session 2026-06-24 Batches A-E (Committed)
- Footer: 5-line Status block, album art 7-char, heart icon, MDI Nerd Font icons
- Library tracks Phase C+D: sort/filter popups, [SEARCH] indicator
- Like/subscribe/unsubscribe from album tracks view (o.t/o.S/o.U)
- Force-split (o.f) + playlist editor overwrite save (vim-driven, 100-level undo)
- Album URL auto-detection (OLAK5uy_ via playlist?list=)
- Green lettering for playing song across ALL browser tabs
- Album art popup (o.v): 95% centered, sixel stored for cleanup
- Metadata pipeline: resolver scoring, Discogs fix, url_added removed, per-track validation removed
- Annotations integration: Tab/Alt+l/Alt+h focus, Bearer search first, 50/page pagination
- `:` command routing in lyrics popup + notes context restore
- Visual mode: yank/paste (p/P), Esc clears visual mode, VISUAL_MODE_COLOUR cyan
- Consistent cyan highlight in queue + annotations + lyrics panels
- Sixel persistence: belt-and-suspenders clear (DCS + CSI 2J)
- Heart icon spacing (2 spaces), like_icon() public fn
- 29 new tests (youtui: 103->124)
- F7 tab cycle back-nav fix (push_snapshot before variant switch)
- Config editor C-r redo (ctrl modifier wired to ViTextEditor)
- 15 dead code items removed, 0 warnings workspace-wide
- Metadata pipeline: title/artist/album normalization, year fallback, per-track validation removed

#### Session 2026-06-24 (ytmapi-cli Full Wiring)
- ytmapi-rs Phase 0: Fixed 3 `todo!()` panics in search.rs TopResultType parsing
- ytmapi-rs Phase 0: Fixed 2 deprecated SearchQuery::new calls in ytmapi-cli
- ytmapi-cli: Full 44-command coverage (was 16) across all ytmapi-rs endpoints
- 7/7 tests pass, 0 warnings

#### Session 2026-06-24 (ytmapi-rs Polish, uncommitted)
- **62 stale TODOs removed** across 30 files (99->37 remaining)
- **0 warnings across workspace** (fixed 35 clippy warnings in 3 dep crates)
- Library sort order exposed through 6 simplified API methods + ytmapi-cli `--sort` flag
- GetAlbumBrowseId resolver (`resolve_album_browse_id()` fn)
- `#[allow(dead_code)]` cleanup: 7 proposital kept, partial stale removal
- Clippy: vi-text-editor 18→0, metadata-provider 12→0, genius-rs 6→0

## Test Status
- youtui: 124/124 pass, 4 ignored, 0 warnings
- metadata-provider: 19/19 pass, 0 warnings
- ytmapi-rs lib: 85/85 pass, 0 warnings
- ViTextEditor: 65/65 pass, 0 warnings
- genius-rs: 14/14 pass, 0 warnings
- ytmapi-cli: 7/7 pass, 0 warnings
- json-crawler: 2/2 pass
- async-callback-manager: 14/14 pass
- **Total: ~330 non-auth pass, 0 fail, 0 warnings across workspace**

## Remaining (Priority Order)

### P1 (This Session)
- Finish dead_code cleanup: remove 17 stale annotations + truly dead methods
- Wire library sort order in youtui UI
- Update docs: CLAUDE.md, TODO.md, roadmap

### P2
- FFT footer bars - real-time audio spectrum (needs rustfft + Source adapter)
- Sixel album art centering/persistence fix
- Like album to library (o.t adds to YT Music profile Albums)

### P3
- Genius annotations fallback (page scrape when no GENIUS_TOKEN)
- Genius lyrics: Musixmatch/LRCLIB integration
- Crate extraction: audio-player (deep async_rodio_sink coupling)
- Related tracks metadata enrichment
- Album browser j/k routing when show_tracks
- Metal-API (metal-api.dev) returns 500

### Skipped (Low Value)
- **GetSavedEpisodes** — podcasts not wired in UI
- **GetAccountInfo** — no UI use case
- **GetPodcast continuations** — podcasts not wired
- **GetSong (full)** — not planned by upstream
- **37 remaining ytmapi-rs TODOs** — all low-value feature gaps (artist categories, i18n, VL prefix, consolidation)
