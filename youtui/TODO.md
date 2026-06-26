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

#### Session 2026-06-23
- ytmapi-rs locale parameterization (language/location, builder methods, 3 tests)
- ytmapi-cli watch-playlist subcommand (Debug-First compliance)
- Metadata-provider crate extraction (19 tests, 0 warnings)
- Queue sort popup improvements (j/k nav, Enter/Esc, o.S)
- Lyrics race guard (generation: u64 counter)
- LRU lyrics cache + negative TTL (5-min error cache, cross-song)
- CRITICAL: PlaylistSearch tab fixed
- Recommendations o.r context menu (GetRelatedTracks -> GetWatchPlaylistQuery)
- NavigationController: fix albumsearch.rs GoToAlbum
- Library refresh: 4 missing playlists_fetched = false paths
- Auth test infra: 3 cookie path fallbacks
- 9 stale #[allow(dead_code)] annotations removed
- Keybinding additions: o.q/o.L/o.Q/o.m/o.n (queue), o.r (library)
- Dead code cleanup

#### Session 2026-06-24 Batches A-E
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

#### Session 2026-06-25 (ytmapi-rs Finalization)
- **62 stale TODOs removed** across 30 ytmapi-rs files (99 -> 37 remaining)
- **35 clippy warnings** fixed: vi-text-editor 18->0, metadata-provider 12->0, genius-rs 6->0
- **17 stale #[allow(dead_code)] removed**, 206 lines dead code deleted across youtui
- **Library sort order** exposed through 6 simplified API methods + CLI `--sort` flag
- **GetAlbumBrowseId resolver** (simplified_queries.rs + CLI `resolve-album`)
- **0 warnings across workspace**, all tests pass

#### Session 2026-06-25 (Annotations + Sort Order UI + Art Popup)
- **Library sort order UI**: `o.O` key in library context menu. Cycles Default->A-Z->Z-A->Recent. Title displays `[A-Z]`/`[Z-A]`/`[Recent]`. Resets fetch flags, re-fetches with new sort.
- **Album art popup pagination**: h/l cycles through all downloaded album arts in queue. Page indicator `N / M`.
- **AlbumSearchBrowser like/unlike**: `liked_playlists: HashSet<PlaylistID>` for proper toggle (was always sending Liked).
- **Sixel centering FIXED**: Root cause found - `Resize::Fit(None)` may output smaller image than target rect. No centering offset was applied. Fix: read `Protocol::area()` after `new_protocol()`, compute offset `(centered - fitted) / 2`, render Image at offset rect.
- **Annotations UI polish**: Enter copies annotation body via wl-copy. Tab/Alt+l auto-selects first annotation. Vimline C-d/C-u for half-page scroll. Absolute line numbers in lyrics.
- **GetAlbumBrowseId doc test** (no_run) + sort order cycle unit test (4 states).
- **Album art popup docs**: Created `docs/subsystems/album_art_popup.md` with full architecture.

## Test Status
- youtui: 136/136 pass, 4 ignored, 0 warnings
- metadata-provider: 47/47 pass, 0 warnings
- ytmapi-rs lib: 85/85 pass, 0 warnings
- ViTextEditor: 65/65 pass, 0 warnings
- genius-rs: 18/18 pass, 0 warnings
- ytmapi-cli: 7/7 pass, 0 warnings
- json-crawler: 2/2 pass
- async-callback-manager: 14/14 pass
- lrclib-rs: 4/4 pass
- rym-genre-data: 10/10 pass
- **Total: ~388 non-auth pass, 0 fail, 0 warnings across 11 crates**

## Remaining (Priority Order)

### P2
- FFT footer bars - real-time audio spectrum (needs rustfft + Source adapter)
- Like album to library (o.t adds to YT Music profile Albums)

### P3
- Genius annotations fallback (page scrape when no GENIUS_TOKEN)
- Genius lyrics: Musixmatch/LRCLIB integration
- Crate extraction: audio-player
- Related tracks metadata enrichment
- Album browser j/k routing when show_tracks
- Metal-API (metal-api.dev) returns 500
- Count-in-header standardization

### Skipped (Low Value)
- **GetSavedEpisodes** - podcasts not wired in UI
- **GetAccountInfo** - no UI use case
- **GetPodcast continuations** - podcasts not wired
- **GetSong (full)** - not planned by upstream
- **37 remaining ytmapi-rs TODOs** - all low-value feature gaps
- **RYM cookie proxy** - exploratory, Cloudflare-blocked
