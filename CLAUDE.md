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
- **Debug logging**: Every feature must be fully wired with logging (info/error/debug) at key decision points. No silent paths. Log input params, success/failure outcomes, and any state transitions useful for debugging. Wire to build, run, verify with logs before commit.
- **Debug-First Rule**: Every new implementation starts by creating CLI debugging tools. CLI tools make tracing changes easier than UI-only debugging. Before wiring UI features, build CLI subcommands/tools that exercise the same backend code paths. Run them to verify correctness before integrating into the UI layer.
- **Enter = speed**: Enter NEVER opens sub-menu or confirmation dialogs. Direct primary action (play, load tracks, focus). All secondary actions behind `o` context menu. No friction, no confirmations.
- **Tmux integration**: Youtui status shown via `~/.local/bin/tmux-music` script (tmpfile-based IPC), tmux window icon via `tmux-nerd-font-window-name` plugin at `~/.config/tmux/tmux-nerd-font-window-name.yml`.
- **Plain Unicode over Nerd Font**: Prefers combining Unicode characters (e.g., `♫⃠`) over Nerd Font glyphs for icons. Suckless-compatible.
- **Incremental testing**: Test one thing at a time. User validates each change before proceeding. No batch testing.
- **Compact UI**: Minimal visual noise, information-dense layouts. Footer shows `Artist - Song - Album` in single line.

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
cargo test --release -p async-callback-manager     # 14 pass (3 lib + 11 integ)
cargo test --release -p json-crawler               # 2 pass (0 lib + 2 doctests)
cargo test --release -p ytmapi-cli                 # 7 pass
```
Total: **~305/305 pass, 0 fail, 4 ignored, 0 warnings** (json-crawler count wrong in docs, actual 2 not 8)

## Warnings
`cargo build --release` -- 10 pre-existing warnings (unused imports, dead_code, deprecated SearchQuery API). Not introduced by changes.

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
| `app/server/messages.rs` | ~1350 | All backend tasks |
| `app/ui/playlist.rs` | ~2812 | Queue, playback, album splitting, visual mode |
| `app/ui/browser.rs` | ~890 | Browser routing, 5-tab dispatch |
| `app/ui/browser/draw.rs` | ~673 | All browser draw functions |
| `app/ui/browser/library.rs` | ~1107 | Library (4th tab) |
| `app/ui/browser/albumsearch.rs` | ~705 | Albums tab (refactored) |
| `config/keymap.rs` | ~2060 | All keybindings by context |
| `app/ui.rs` | ~1584 | Main window, event routing |
| `libs/metadata-provider/` | 19 tests | Metadata trait + 5 provider impls |
| `app/ui/playlist/notes_popup.rs` | ~272 | Vim-driven notes text editor |
| `docs/subsystems/notes.md` | ~100 | Notes popup architecture doc |

## Playlist Features Status
All CRUD wired: Create, Delete, Rename, Edit details, Edit privacy, Add/Remove items, Reorder (swap), Rate, Get details, Get tracks, Library playlists, Batch-merge.

Frontend: 14 handler pairs, 9 AppCallbacks, context menu (D/R/E/t/i/x/J/K/S/U/M), save popup (privacy picker), rename popup, edit popup (4 fields), details popup (loading->display), editor popup (:rename/:privacy/:rate), delete confirm. Library auto-refresh on mutation.

Album splitting: Detects full-album/EP/LP/demo/single entries (tags: full album, full ep, full lp, full demo, full single, album, demo, ep, single, singles). Triggers `ValidateMetadata` which identifies tracks → `insert_album_tracks` splits into individual entries with offsets, durations, metadata. Arc-sharing for audio data.

## Notes Popup Keybindings
`:w` Save | `:wq` Save+Quit | `:q` Quit | `Esc` Close | Enter on URL: Open | `i` Insert | `V` visual line | `C-v` visual block | `y` yank | All VTE motions (j/k/h/l/gg/G/w/b/dd/yy/p/P/u/C-r/o/O)

## Queue Keybindings (o menu)
`o.s` shuffle, `o.r`/`o.S` sort, `o.R` get related, `o.q` save, `o.L` load, `o.Q` delete, `o.m` romaji, `o.n` new playlist, `o.E` existing playlist, `o.d` delete, `o.D` delete all, `o.A` best quality, `o.c` category filter, `o.I` song info, `o.z` repeat, `o.t` like, `o.l` lyrics, `o.a` artist, `o.b` album, `o.v` album cover, `o.y`/`y` copy url.

## Enter Key Behavior (ncspot-style)
Enter NEVER opens a sub-menu. Enter ALWAYS does the primary action:
- Playlist (queue) → play selected song
- Browser songs → play song
- Browser artists → display artist albums
- Browser playlists → display playlist tracks
- Library category → focus content panel
Context menu is exclusively via `o`.

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

## Session 2026-06-23 (This Session, Not Committed)
- p# column Length(3)→Length(6) playlist, Length(4)→Length(6) albumsearch/playlistsearch/artistsearch songs_panels
- Footer format: `Artist - Song - Album` instead of `Song - Artist - Album`
- Frontend chunking removed (single CreatePlaylistWithVideos call, backend handles splitting)
- Backend chunk naming fixed: pt0/pt1/pt2 instead of `pt0 pt. 2`
- Batch size 5000→1000 for API reliability
- Enter keybinding for Library Playlist category
- ytmapi-cli: 5 new debug subcommands (delete-playlist, edit-playlist, rate-playlist, remove-items, add-to-playlist)
- CRITICAL: VL prefix stripping REVERTED in ytmapi-rs. All endpoints (delete/edit/additems/rate) now send playlistIds as-is with VL prefix, matching ytmusicapi Python behavior. Stripping VL caused delete/rename to return fake-200s (YTM returns 200 for invalid IDs) while actually doing nothing.
- CRITICAL: Library auto-refresh after playlist mutations. Added `library_playlist_mutated` flag set by HandleDeletePlaylistOk/HandleRenamePlaylistOk/HandleEditPlaylistDetailsOk/HandleRatePlaylistOk/HandleCreatePlaylistOk. Checked in app.rs `handle_effect` after mutation, triggers `library_browser.reload_category()`.
- CRITICAL: `reload_category()` bug fix. Changed `self.loading = true` → `self.loading = false` before `fetch_current_category()` call. Pre-existing bug: setting loading=true blocked fetch because `fetch_current_category()` checks loading first and returns no-op. Caused library to freeze in "Loading..." state after any mutation.
- Debug logging preference added to CLAUDE.md
- Genius lyrics: simplified slug URL fallback for parenthetical song titles. `simplify_title()` strips `(...)` and `[...]` content. Tried as intermediate step before search API.
- Debug logging added throughout genius-rs fetch pipeline (slug attempts, search API calls, hit validation).

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
- Genius hit validation: fetch_page returns final URL; find_and_fetch validates hits against query; redirects to wrong pages rejected
- Genius CLI: Add annotations subcommand (Debug-First for testing annotation extraction)
- Notes popup (`:notes`): Vim-driven text editor for URLs and notes, plain text persistence
- VTE Esc fix: Removed `cursor -= 1` on Esc from insert mode (non-standard vim behavior)
- Visual line mode in Notes: `V` + `j/k` with cyan highlight, `y` yanks to system clipboard
- Visual block mode: `C-v` for rectangular selection, `y` yanks block text
- `o`/`O` in normal mode: Open new line below/above, enter insert mode
- Library playing indicator: Green highlight on currently playing song in Liked Songs list
- Enter behavior: Playlist (queue) Enter now direct play (no sub-menu)
- Album split detection: Expanded to catch Full EP, Full LP, demo, single, singles patterns
- Docs: New `docs/subsystems/notes.md` — full arch decisions, keybinds, design rationale

## Session 2026-06-22 (Committed)
- `fix:` lyrics help text — `( ) Prev/Next Lyric | <> Prev/Next Song | [] Seek | Esc/q: Close`
- `chore:` cleanup — removed 6 stale TODOs + dead sort_column from playlist_editor_popup
- `chore:` cleanup — removed dead `AppCallback::Back` (Backspace works via BrowserAction)
- `chore:` cleanup — removed dead `GetPlaylistDetailsFromLibrary` (OpenDetailsPopup is live path)
- `chore:` cleanup — removed stale `#[allow(dead_code)]` from AddPlaylistToPlaylist struct
- `chore:` cleanup — added `library_playlist_mutated = true` to merge success handler
- Album split tags expanded: `full single`, `album` added to strip list

## Session 2026-06-23 (This Session, Not Committed)
- **VL prefix confirmed**: ytmusicapi Python `validate_playlist_id()` at `parsers/playlists.py:270` strips `VL` from all mutation endpoints (delete, edit, add/remove items, rate). Browse/read endpoints need VL. Pattern: read=add VL, mutate=strip VL.
- **Delete/Rename/Edit fixed**: All mutation endpoints (`playlist/delete`, `browse/edit_playlist`) now strip VL prefix. 400 "invalid argument" resolved.
- **Rate playlist fixed**: `like/like` endpoint also needs VL stripped. 404 "entity not found" resolved by stripping VL prefix from rate query.
- **Playlist editor empty tracks fix**: `library.rs:1071` condition checked `playlist_data` (always populated from library listing) instead of `playlist_tracks`. Fixed: check `playlist_tracks.is_empty()` first, fetch if empty.
- **Tmux icon**: Changed to `♫⃠` (anti-music combining symbol)
- **Library auto-refresh**: Confirmed working after all playlist mutations.
- **Annotations component isolation**: Annotations panel separate component with own vim nav. Tab/l/h switch, a toggle.
- **Enter = primary action (ncspot-style)**: Never sub-menu. Direct play/load/focus.

## Notes Popup Features
- Vim-driven text editor for storing URLs, song links, personal notes
- File: `~/.config/youtui/notes.txt` — plain text, persists across sessions
- Keybindings: `:w` save, `:wq` save+quit, `:q` quit, Esc close
- Enter on URL line → opens in yt-dlp
- Full ViTextEditor support: j/k/h/l/gg/G/w/b, dd/yy/p, u/C-r, visual line/block
- System clipboard yank via `wl-copy` in visual mode
- See `docs/subsystems/notes.md` for full architecture

## Genius API
- Token: `5e4pF3nYzWG-xHFdpQpmX-nkjfLjZODc4PUBIQrphwHnbnCkjmS3x0pewYHY33Sq` in config
- CLI: `GENIUS_TOKEN=... genius-rs annotations "Artist" "Song"`
- Search: `genius-rs search "Artist" "Song"` → returns real song ID (1063 etc.)
- Hit validation: `find_and_fetch` rejects results where final URL redirects to non-Genius page
- Bearer search prioritized over slug URL when token available (gives real song ID)

## Known Issues
- **Genius annotations w/o token**: `__INITIAL_STATE__` scraping fails on most pages. Need `GENIUS_TOKEN`.
- **Genius lyrics**: `find_and_fetch` slug URL fails for songs with parenthetical/bracketed title extras (e.g., "(Japanese Bonus Track)"). Simplified slug fallback added but may not match all cases.
- **Auth tests**: 52 ytmapi-rs integration tests need cookie file.
- **Playlist editor**: 2-Enter flow: first fetches tracks, second opens editor. Auto-open after fetch not yet implemented.
- **Album art popup**: Sixel fullscreen may fail on small terminals (width/height < image cells). Needs graceful fallback.
- **Playlist merge into self**: `AddPlaylistToPlaylist` with identical source/target IDs causes 400 error. UI should prevent selecting same playlist. `effect_handlers_playlist.rs:1029`
- **Cursor style in editor popups**: Notes popup cursor matches search box (`▎` character). ConfigEditorPopup and other editor popups lack cursor style.

## Remaining
### P0 — User-visible bugs
- Playlist editor auto-open after track fetch (2-Enter flow)
- Merge into self guard (selecting same playlist → 400)
- Album art sixel fallback on small terminals
- Cursor style in ConfigEditorPopup etc. (notes popup works)

### P1 — Missing features (wired backend, need frontend)
- **Like status in details popup**: `GetPlaylistDetails.like_status` already parsed, just not rendered. Details popup `playlist_details_popup.rs:draw()` — add 1 line showing rating.
- **Back navigation (remaining gap)**: `BrowserAction::Back` + `state_stack` works for `Navigate()` but `handle_change_search_type()` (F7 tab cycle) doesn't push snapshots. Stack can restore wrong tab.
- **Playlist overwrite mode**: `app.rs:481` — fetch current tracks + remove before add
- **Annotations integration**: verify Tab/l/h switching is fully wired

### P2 — Polish
- FFT footer bars (roadmap)
- Remove remaining 25 dead_code items (see prior audit)
- Fix test counts in docs (json-crawler 8→2, async-callback-manager 15→14)

### P3 — Tech debt
- Genius annotations fallback (page scrape)
- Genius lyrics: Musixmatch integration
- ytmapi-cli: more fixture types
- Crate extraction: audio-player
- Count-in-header standardization
- Album browser j/k routing when show_tracks
- ytmapi-rs: 150 pre-existing TODOs (parse/search.rs, parse/artist.rs type safety)
