# youtui (caos-obliquo fork)

## What's DONE

### 3-Day Sprint (2026-06-19 → 2026-06-22) — 108 commits

#### Day 1: Foundation
- Library browser tab (playlist tracks, context menu, visual mode)
- ViTextEditor complete (65 tests: f/F/t/T, %, w/b/e/W/B/E, 0/$, gg/G, r/~, J/., C-r, iw/aw/i(/a(/i"/a", surround)
- Lyrics popup (visual mode, hybrid line numbers, pagination)
- Album art 1920x1080 HD, decode loop guard, throttle
- Navigation hub (o→a/b, g→a/b, local search, go-to)
- Keybind standard (consistent across all tabs)
- Crossterm 0.29 migration, 15→0 warnings
- Nerd icons removed (suckless)

#### Day 2: Features
- Genius annotations (unified list, scraping, JSON API, right panel)
- PlaylistEditor (:w/:q/:wq, :rename/:privacy/:rate)
- NavigationController (:cmd parser, skip URL album split)
- Lyrics (n/p next/prev song, <> seek, ( ) nav, race guard, LRU cache)
- Queue sort (o.r cycles columns)
- Playlist popups (rename, edit 4-field, details loading→display, save privacy)
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
- **CRITICAL: PlaylistSearch tab fixed** — deprecated no-op types replaced with real dispatch
- Recommendations o.r context menu (GetRelatedTracks → GetWatchPlaylistQuery)
- NavigationController: fix albumsearch.rs GoToAlbum
- Library refresh: 4 missing playlists_fetched = false paths
- Auth test infra: 3 cookie path fallbacks
- 9 stale #[allow(dead_code)] annotations removed
- Keybinding additions: o.q/o.L/o.Q/o.m/o.n (queue), o.r (library)
- Dead code cleanup

#### Session 2026-06-24 (Uncommitted — Testing Needed)
See `CLAUDE.md` "Session 2026-06-24" section for full feature list.

**What's implemented:**
- Footer heart icon (filled 󰋑 / empty ♥)
- Library tracks Phase D: [SEARCH] indicator + selection highlight + filtered nav
- Library tracks Phase C: sort/filter popups (o.z/o.c)
- Like/subscribe/unsubscribe from album tracks view (o.t/o.S/o.U)
- Force-split (o.f) — re-validate + re-split selected song
- Playlist editor with overwrite save (o.e → :w fetch+remove+add)
- Album URL auto-detection (OLAK5uy_ via playlist?list=)
- Metadata pipeline: resolver scoring, title cleaning refactor, original_album preservation
- Discogs priority 30→8, per-track validation removed, url_added removed

**What needs testing:** All 12 items in CLAUDE.md "NEEDS TESTING" table.

## Test Status
- youtui: 103/103 pass, 4 ignored, 0 warnings
- metadata-provider: 19/19 pass, 0 warnings
- ytmapi-rs lib: 85/85 pass
- ViTextEditor: 65/65 pass
- genius-rs: 14/14 pass
- ytmapi-cli: 3/3 pass (+4 new later)
- json-crawler: 2/2 pass (0 lib + 2 doctests)
- async-callback-manager: 14/14 pass (3 lib + 11 integ)
- **Total: ~305/305 pass, 0 fail, 4 ignored, 0 warnings**

## Remaining
- Genius annotations: individual page scrape fallback (no __INITIAL_STATE__)
- Genius lyrics: Musixmatch integration
- ytmapi-cli: more fixture types, streaming tests
- Crate extraction: audio-player (deep async_rodio_sink coupling)
- Related tracks metadata enrichment (YTM API limitation — no album/year)
- Test and commit session 2026-06-24 features
- Back navigation (F7 tab cycle) push_state_snapshot fix
