# Youtui — Project Knowledge

## GOLDEN RULE
One feature at a time. Implement -> test (user validates) -> commit -> next. Never batch changes.
If things break, rollback and re-apply one-by-one.

## Workflow (User-Defined)
- **One feat per time**: user tests, validates, then proceeds. No batching.
- **User chooses priority**: items listed, user picks. Always one.
- **Test before commit**: user must confirm working before commit.
- **Debug-First Rule**: CLI debug tool before UI wiring for any new backend path.

## User Preferences (Strict)
- **No sudo** without explicit permission.
- **No AUR.** Only official repos + local compilation.
- **Suckless.** Minimal deps, focused scope, ASCII-only words, no bloat.
- **Rust only.** No shell plugins, no non-Rust dependencies.
- **Subagent stack**: `rustacean` for Rust code review, `akita` for architecture/tooling decisions.
- **WHITESPACE** (critical): Keep cursor/indentation whitespace in preferences block exactly as-is. Rendered verbatim.
- **Consistency across windows**: Every browser tab (Artists, Songs, Albums, Library, Playlist) must share same UI patterns: search (F1), advanced table columns with sort/filter, o-mode context menu, j/k/gg/G navigation. No tab second-class.
- **No em-dashes**: Never use `--` (em-dash) in code. Use `-` (hyphen) for all display strings, log messages, comments, docs. Bad practice, avoid entirely.
- **Priority: Playlist features most important.** All browser entities fully wired backend->UI->API.
- **Mail**: `caos_obliquo@outlook.com`

## Build
- Workspace root: `/home/caos/builds/youtui/`
- Rust nightly (1.97.0)
- Binary: `cargo build --release` -> `target/release/youtui`
- Dependencies: yt-dlp, ffmpeg, alsa-lib (system packages via pacman)

## Tests
```bash
cargo test --release -p youtui --bin youtui       # 103 pass, 4 ignore
cargo test --release -p metadata-provider          # 19 pass
cargo test --release -p vi-text-editor             # 65 pass
cargo test --release -p ytmapi-rs --lib            # 85 pass (no auth)
cargo test --release -p ytmapi-rs                  # 28/52 auth (needs cookie)
cargo test --release -p genius-rs                  # 14 pass
cargo test --release -p async-callback-manager     # 15 pass
cargo test --release -p json-crawler               # 8 pass
cargo test --release -p ytmapi-cli                 # 3 pass
```
Total: **312/312 pass, 0 fail, 4 ignored, 0 warnings**

## Warnings
`cargo build --release` -- 0 warnings across workspace (youtui + 6 lib crates + metadata-provider).

## Arch (3-layer async callback)
```
Frontend (UI) -> TaskManager -> Backend (Server)
```
See `docs/` for full reference (5.4k lines, 20 files).

## 8 Workspace Crates (49k+ LOC)
| Crate | Status | Tests |
|---|---|---|
| `youtui` | Main binary | 103 |
| `ytmapi-rs` | YT Music API client | 85 lib + 28/52 auth |
| `vi-text-editor` | Vim text editor widget | 65 |
| `metadata-provider` | Metadata trait + impls | 19 |
| `genius-rs` | Genius lyrics/annotations | 14 |
| `async-callback-manager` | Async task dispatch | 15 |
| `json-crawler` | JSON path parser | 8 |
| `ytmapi-cli` | CLI debug tool | 3 |

## 5 Browser Tabs Fully Wired
| Tab | Search | Table | Sort/Filter | o Menu | Nav | Status |
|---|---|---|---|---|---|---|
| Artists | F1 | Detailed | Y | Y | ga/gb | OK |
| Albums | F1 | Detailed | Y | Y | ga/gb | OK (refactored to AdvancedTableView) |
| Songs | F1 | Detailed | Y | Y | ga/gb | OK |
| Library | F1 | Detailed | Y | Y | ga/gb | OK |
| PlaylistSearch | F1 | Detailed | Y | Y | ga/gb | **FIXED** (was dead, now live) |

## Key Files
| File | Lines | Purpose |
|---|---|---|
| `app/server/messages.rs` | ~1368 | All backend tasks |
| `app/ui/playlist.rs` | ~2815 | Queue, playback, visual mode |
| `app/ui/browser.rs` | ~885 | Browser routing, 5-tab dispatch |
| `app/ui/browser/draw.rs` | ~695 | All browser draw functions |
| `app/ui/browser/library.rs` | ~1080 | Library (4th tab) |
| `app/ui/browser/albumsearch.rs` | ~720 | Albums tab (refactored) |
| `config/keymap.rs` | ~2079 | All keybindings by context |
| `app/ui.rs` | ~1530 | Main window, event routing |
| `libs/metadata-provider/` | 19 tests | Metadata trait + 5 provider impls |

## Playlist Features Status
All CRUD wired: Create, Delete, Rename, Edit details, Edit privacy, Add/Remove items, Reorder (swap), Rate, Get details, Get tracks, Library playlists, Batch-merge.

Frontend: 14 handler pairs, 9 AppCallbacks, context menu (D/R/E/t/i/x/J/K/S/U/M), save popup (privacy picker), rename popup, edit popup (4 fields), details popup (loading->display), editor popup (:rename/:privacy/:rate), delete confirm. Library auto-refresh on mutation.

## Queue Keybindings (o menu)
`o.s` shuffle, `o.r`/`o.S` sort, `o.R` get related, `o.q` save, `o.L` load, `o.Q` delete, `o.m` romaji, `o.n` new playlist, `o.E` existing playlist, `o.d` delete, `o.D` delete all, `o.A` best quality, `o.c` category filter, `o.I` song info, `o.z` repeat, `o.t` like, `o.l` lyrics, `o.a` artist, `o.b` album, `o.v` album cover, `o.y`/`y` copy url.

## Session 2026-06-23 (Committed)
- metadata-provider crate extraction (19 tests, 0 warnings)
- CRITICAL: PlaylistSearch tab fixed (deprecated no-op types removed, dispatch wired, keybindings populated)
- Albums AdvancedTableView refactor (Enter loads tracks, draw_advanced_table matching other browsers)
- ytmapi-cli watch-playlist subcommand (Debug-First compliance)
- ytmapi-rs locale parameterization (language/location builders, 3 tests)
- 9 stale #[allow(dead_code)] removed
- Keybinding additions: o.q/o.L/o.Q/o.m/o.n (queue), o.r (library)
- Lyrics race guard + LRU cache with negative TTL
- Queue sort popup improvements (j/k, Enter/Esc, o.S)
- NavigationController: fix albumsearch GoToAlbum
- Auth test infra: 3 cookie path fallbacks
- Library refresh: 4 missing playlists_fetched = false paths

## Session 2026-06-23 (This Session, Committed)
- Footer album format: Single-line `Title - Artist - Album_ [s]` instead of separate album line
- Sixel tmux vanish fix: Post-draw flush_sixel() re-sends sixel data with cursor positioning, gated behind TERM=tmux* env
- Sixel `:` command: Clear stale sixel data in draw_app; blanking sequence when footer hidden
- `[I]` mode indicator leak: Songs InputRouting default List; Artists ArtistInputRouting default List; BrowserSearchAction::Close resets search_popped
- Browser tab order: Artists/Albums/Songs/Playlists/Library (Library last)
- Album global YTM search: handle_text_entry_action(Submit) calls search_albums_query; removed live-search-on-every-keystroke bug
- `o.v` album art popup: ViewAlbumCover resolves thumbnail from current song or last_album_art; uses Resize::Scale for fullscreen
- Albums right panel: Always shows draw_advanced_table headers instead of hint
- PlaylistSearch right panel: Always shows draw_advanced_table headers
- Config cleanup: Strip defaults, keep only overrides; docs updated
- Header dedup: Remove duplicate F1, colon, y/C-y CopySongUrl global keybinds
- ytmapi-cli: Add search-playlists, playlist-songs subcommands (Debug-First)
- dead_code cleanup: Remove stale annotations from SearchPlaylists, GetPlaylistSongs, api.rs methods (all now wired)
- Genius hit validation: fetch_page returns final URL; find_and_fetch validates hits against query; redirects to wrong pages rejected. Song without Genius page shows clean "No lyrics found"
- Genius CLI: Add annotations subcommand (Debug-First for testing annotation extraction)

## Known Issues
- **Genius annotations w/o token**: `__INITIAL_STATE__` scraping fails on most pages. Need `GENIUS_TOKEN`.
- **Auth tests**: 52 ytmapi-rs integration tests need cookie file.
- **`AppCallback::Back`**: `#[allow(dead_code)]` at app.rs:158 -- TODO: Wire back navigation.
- **`AppCallback::GetPlaylistDetailsFromLibrary`**: `#[allow(dead_code)]` at app.rs:177 -- rate toggle pending.
- **`SearchPlaylists`/`GetPlaylistSongs`**: `#[allow(dead_code)]` at messages.rs:54,62 -- batch streaming (now wired, stale annotation removed).
- **Album art popup**: Sixel fullscreen may fail on small terminals (width/height < image cells). Needs graceful fallback.
- **Playlist merge into self**: `AddPlaylistToPlaylist` with identical source/target IDs causes 400 error. UI should prevent selecting same playlist. `effect_handlers_playlist.rs:1029`
- **Cursor style in editor popups**: Notes popup cursor style implemented (`SetCursorStyle`). ConfigEditorPopup and other editor popups lack cursor style — they always show default terminal cursor.

## Remaining
- Genius annotations: page scrape fallback (no `__INITIAL_STATE__`)
- Genius lyrics: Musixmatch integration
- ytmapi-cli: more fixture types, streaming tests
- Crate extraction: audio-player (deep async_rodio_sink coupling)
- Count-in-header standardization across all browser views
