# Youtui — Project Knowledge

## GOLDEN RULE
One feature at a time. Implement -> test (user validates) -> commit -> next. Never batch changes.
If things break, rollback and re-apply one-by-one.

**`/` = global fuzzy search**: `/` everywhere triggers local fuzzy filter across visible items. Identical behavior in queue, all 5 browser tabs, and any list view. Never dead/incomplete. Fuzzy mathches across title/artist/album. Shows `[SEARCH: text (N/M)]`. No exceptions. If `/` exists in a context, it must filter. If it doesn't filter, don't bind it.

## Workflow (User-Defined)
- **One feat per time**: user tests, validates, then proceeds. No batching.
- **User chooses priority**: items listed, user picks. Always one.
- **Test before commit**: user must confirm working before commit.
- **Debug-First Rule**: CLI debug tool before UI wiring for any new backend path.
- **Investigate before change**: trace root cause fully before proposing fix. Present findings, then fix.
- **Small commits, clean diffs**: each commit = one logical change. No mixed concerns. No drive-by refactors.

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
- **Plain Unicode over Nerd Font**: Prefers combining Unicode characters (e.g., `♫⃠`) over Nerd Font glyphs for icons. Suckless-compatible. Exception: Nerd Font MDI icons for footer status (repeat/shuffle/heart) for visual clarity.
- **Incremental testing**: Test one thing at a time. User validates each change before proceeding. No batch testing.
- **Compact UI**: Minimal visual noise, information-dense layouts. Footer shows `Artist - Song - Album` in single line.
- **Terminal**: foot (Wayland native). Sixel graphics support but DCS clear is unreliable. Design fallbacks.
- **Docs are code**: CLAUDE.md, TODO.md, docs/ must stay current with every commit. Stale docs = bug.
- **Dead code is liability**: Remove unused structs, methods, annotations on sight. Keep only what compiles and is wired.
- **Prefer foreground over background**: Subtle styling (green text, not green highlight) for playing indicators. Less visual noise.
- **Prioritize root cause over workaround**: Trace the chain before patching. If the fix is in a dependency, document upstream.
- **Catalog before implement**: New features get a TODO entry with scope, files, and estimate before coding starts.

## Build
- Workspace root: `/home/caos/builds/youtui/`
- Rust nightly (1.97.0)
- Binary: `cargo build --release` -> `target/release/youtui`
- Dependencies: yt-dlp, ffmpeg, alsa-lib (system packages via pacman)

## Tests
```bash
cargo test --release -p youtui --bin youtui       # 124 pass, 4 ignore
cargo test --release -p metadata-provider          # 19 pass
cargo test --release -p vi-text-editor             # 65 pass
cargo test --release -p ytmapi-rs --lib            # 85 pass (no auth)
cargo test --release -p ytmapi-rs                  # 28/52 auth (needs cookie)
cargo test --release -p genius-rs                  # 14 pass
cargo test --release -p async-callback-manager     # 14 pass (3 lib + 11 integ)
cargo test --release -p json-crawler               # 2 pass (0 lib + 2 doctests)
cargo test --release -p ytmapi-cli                 # 7 pass
```
Total: **~326/326 pass, 0 fail, 4 ignored, 0 warnings** (youtui 124 + 19 + 65 + 85 + 14 + 14 + 2 + 7 = 330 metadata-provider included separately)

## Warnings
`cargo build --release` -- 1 pre-existing warning (ytmapi-rs `unused_mut` in playlist.rs). Not introduced by changes. 2 pre-existing ytmapi-cli deprecation warnings (SearchQuery). youtui crate: 0 warnings.

## Arch (3-layer async callback)
```
Frontend (UI) -> TaskManager -> Backend (Server)
```
See `docs/` for full reference (5.4k lines, 20 files).

## 9 Workspace Crates (50k+ LOC)
| Crate | Status | Tests |
|---|---|---|
| `youtui` | Main binary | 103 |
| `ytmapi-rs` | YT Music API client | 85 lib + 28/52 auth |
| `vi-text-editor` | Vim text editor widget | 65 |
| `metadata-provider` | Metadata trait + impls | 19 |
| `genius-rs` | Genius lyrics/annotations | 14 |
| `async-callback-manager` | Async task dispatch | 15 |
| `json-crawler` | JSON path parser | 8 |
| `ytmapi-cli` | CLI debug tool | 7 |
| `metal-proxy` | Metal Archives direct proxy | 0 |
| `libs/metal-proxy/` | ~250 | Chromium-free background proxy (cookie-based)

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
| `app/ui/playlist.rs` | ~2835 | Queue, playback, album splitting, visual mode |
| `app/ui/browser.rs` | ~932 | Browser routing, 5-tab dispatch |
| `app/ui/browser/draw.rs` | ~657 | All browser draw functions |
| `app/ui/browser/library.rs` | ~1500 | Library (4th tab) with inline tracks view |
| `app/ui/browser/albumsearch.rs` | ~720 | Albums tab (refactored, like/subscribe/audio_playlist_id) |
| `config/keymap.rs` | ~2130 | All keybindings by context |
| `app/ui.rs` | ~1591 | Main window, event routing |
| `libs/metadata-provider/` | 19 tests | Metadata trait + 5 provider impls |
| `app/ui/playlist/notes_popup.rs` | ~272 | Vim-driven notes text editor |
| `app/ui/playlist/playlist_editor_popup.rs` | ~484 | Playlist editor (nvim-driven, overwrite save) |
| `app/ui/playlist/album_art_popup.rs` | ~26 | Album art sixel popup |
| `app/ui/playlist/config_editor_popup.rs` | ~146 | Config file editor |
| `app/ui/browser/footer.rs` | ~257 | Footer: progress, metadata, heart icon, album art |
| `app/ui/playlist/effect_handlers_playlist.rs` | ~650 | ValidateMetadata, overwrite save chain handlers |
| `libs/metal-proxy/src/main.rs` | ~275 | Metal Archives cookie-based proxy |

## Playlist Features Status
All CRUD wired: Create, Delete, Rename, Edit details, Edit privacy, Add/Remove items, Reorder (swap), Rate, Get details, Get tracks, Library playlists, Batch-merge.

Frontend: 14 handler pairs, 9 AppCallbacks, context menu (D/R/E/t/i/x/J/K/S/U/M), save popup (privacy picker), rename popup, edit popup (4 fields), details popup (loading->display), editor popup (:rename/:privacy/:rate), delete confirm. Library auto-refresh on mutation.

Album splitting: Detects full-album/EP/LP/demo/single entries (tags: full album, full ep, full lp, full demo, full single, album, demo, ep, single, singles). Triggers `ValidateMetadata` which identifies tracks → `insert_album_tracks` splits into individual entries with offsets, durations, metadata. Arc-sharing for audio data.

## Playlist Editor Keybindings (nvim-driven, line-based list)
See `playlist_editor_popup.rs` for implementation.

### Motions
- `j`/`k` — move down/up (with `Nj`/`Nk` count prefix)
- `g`/`gg` — go to first line (or `Ng` to line N)
- `G` — go to last line (or `NG` to line N)

### Delete (d operator)
- `dd`/`Ndd` — delete N lines
- `dN`+`j` — delete N lines down
- `dN`+`k` — delete N lines up
- `dg` — delete to top
- `dG`/`D` — delete to end

### Yank (y operator)
- `yy`/`Nyy` — yank N lines
- `yj` — yank line below
- `yk` — yank line above
- `ygg` — yank to top
- `yG` — yank to end
- `Y` — yank current line

### Paste
- `p` — paste below cursor
- `P` — paste above cursor

### Visual mode
- `V` — toggle visual line selection
- `j`/`k` — extend selection
- `d`/`x` — delete selection
- `y` — yank selection
- `p`/`P` — paste over selection

### Undo/Redo
- `u` — undo (100-level stack)
- `C-r` — redo slot (unbound yet)

### Insert/Reorder
- `o`/`O` — insert blank line below/above
- `J`/`K` — move line down/up (swap, with undo)

### Other
- `:` — command mode (`:w` save, `:wq` save+quit, `:q` quit, `:q!` force quit, `:d N` delete, `:m N M` move, `:rename`, `:privacy`, `:rate`)
- `q`/`Esc` — close
- `E` — save to existing playlist
- Capacity bar at top: `Tracks: N/5000 [■■■■] [□□□□] [□□□□] [□□□□]` (4 blocks × 1250)
- Pending count shows in mode indicator: `[5]`, `[DELETE 3]`, `[V]`

### Architecture
- `save_state()` pushes full track snapshot to `undo_stack` before every mutation
- `yank_buffer: Vec<ListSong>` stores copied lines
- `delete_mode`/`yank_mode` are operator-mode flags (like vim's d/y waiting for motion)
- `visual_mode` + `visual_start` for visual line selection

## Notes Popup Keybindings
`:w` Save | `:wq` Save+Quit | `:q` Quit | `Esc` Close | Enter on URL: Open | `i` Insert | `V` visual line | `C-v` visual block | `y` yank | All VTE motions (j/k/h/l/gg/G/w/b/dd/yy/p/P/u/C-r/o/O)

## Queue Keybindings (o menu)
`o.s` shuffle, `o.r`/`o.S` sort, `o.R` get related, `o.q` save, `o.L` load, `o.Q` delete, `o.m` romaji, `o.n` new playlist, `o.E` existing playlist, `o.d` delete, `o.D` delete all, `o.A` best quality, `o.c` category filter, `o.I` song info, `o.z` repeat, `o.t` like, `o.l` lyrics, `o.a` artist, `o.b` album, `o.v` album cover, `o.y`/`y` copy url, `o.Y`/`Y` copy album url.

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
- **Album art popup**: `o.v` opens full-screen centered image via `centered_rect_fixed(90,90)` + `Resize::Fit(None)`. Early return in `draw_app` skips main window (no sixel corruption). Sixel clear at start of every draw. Known bug: centering not perfect, sixel persistence after close.
- **Footer 2-line metadata**: Artist-Song on line 1, Album indented gray on line 2. Truncation with `...`. Fallback album art position fixed (was `Rect { x:0, y:0 }`).
- **Playlist editor nvim-driven**: undo stack (100-level), yank/paste (yy/p/P), visual mode (V->j/k->d/y), D=dG, Y=yy, o/O insert blank line, count prefix for all motions/ops, delete/yank operator modes. 4-block capacity bar (`Tracks: N/5000 [■■■■]...`).
- **Library playlist tracks inline**: uses `draw_advanced_table` with proper columns (#, Artist, Album, Song, Duration, Year). Left category panel hidden when showing tracks. Enter plays song, Esc goes back (DismissTracks action + keybinding). Visual mode, dd/dg/dG delete.
- **Copy Album URL**: `o.Y` / global `Y` copies `https://music.youtube.com/browse/{album_id}`.
- **P0 bugs fixed**: merge-into-self guard (source==target silent no-op), album art sixel min-size guard, ConfigEditorPopup cursor style (teal marker via Line+Span).
- **Warnings**: 0 across workspace (were 15).
- **Title cleaning**: strips `(Official Audio)`, `(Official Video)`, `c legenda`, `Legendado`, `subtitle` etc. from titles before metadata lookup. Strips bare artist prefix when no ` - ` separator. Fixes dangling paren after strip.
- **Artist normalization**: `normalize_artist_name()` capitalizes first letter. Applied in `From<ParsedSongArtist>`, `MetadataEffect::Validated`, and `insert_album_tracks`.
- **Discogs artist fix**: was returning `artist: None`, now extracts `artists[0].name` from Discogs Master API response.
- **Metal API provider**: queries `https://metal-api.dev/` (approved MA REST API) at priority 5. Returns band name, album, year, tracklist. API returns 500 (backend crash). Falls back to local proxy.
- **MA_COOKIE direct access** (Cookie-based Metallum access — ONLY working path):
  - Reads `MA_COOKIE` env var, then `~/.config/youtui/ma_cookie` file
  - Makes direct HTTP requests to Metal Archives AJAX API (bypasses Cloudflare)
  - Returns artist, album, year (from `<!-- 2024 -->` comments), full tracklist, genre (from band page)
  - Cookie auto-saved to config file for persistence
  - Expires ~30 min, needs periodic refresh
  - Proven working: 91 Megadeth albums returned, genres extracted from band page
- **metal-proxy** (`libs/metal-proxy/`):
  - Pure background HTTP server on port 5000
  - No headless browser, no window, no Python — Rust-only
  - Reads saved cookie, serves MA data via direct HTTP
  - Background task refreshes cookie from running Chromium via CDP (every 15 min)
  - `--get-cookie` flag: launches Chromium with `--remote-debugging-port=9222`, tries headless=new first, falls back to visible window
  - Your metadata provider's `try_local_proxy` connects automatically
- **Genre aliasing**: 3,713 genres from MusicBee hierarchy (MusicBrainz + Discogs + RYM + Wikidata). `genre_map::normalize_genre()` normalizes provider genres. Integrated into `MetadataRegistry.resolve()`. 26 tests pass.
- **Year fallback**: extract 4-digit year from album name when providers return `None`.
- **CLI debug tool**: `ytmapi debug resolve <artist> <title>` tests full pipeline.
- `ytmapi debug genre <genre>` / `genre-list [filter]` test genre normalization.
- **Chromium headless** blocked by Cloudflare (confirmed). No viable browser-automation path.
- **enmet Python lib** (github.com/lukjak/enmet) accesses MA — not used (Rust-only rule).

## Session 2026-06-23 (This Session, Committed)
### Core UI
- **Album art popup**: `o.v` opens full-screen centered image via `centered_rect_fixed(90,90)` + `Resize::Fit(None)`. Early return in `draw_app` skips main window (no sixel corruption). Sixel clear at start of every draw. Known bug: centering not perfect, sixel persistence after close.
- **Footer 2-line metadata**: Artist-Song on line 1, Album indented gray on line 2. Truncation with `...`. Fallback album art position fixed (was `Rect { x:0, y:0 }`).
- **Playlist editor nvim-driven**: undo stack (100-level), yank/paste (yy/p/P), visual mode (V->j/k->d/y), D=dG, Y=yy, o/O insert blank line, count prefix for all motions/ops, delete/yank operator modes. 4-block capacity bar (`Tracks: N/5000 [■■■■]...`). Visual selection color changed from teal to cyan.
- **Library playlist tracks inline**: uses `draw_advanced_table` with proper columns (#, Artist, Album, Song, Duration, Year). Left category panel hidden when showing tracks. Enter plays song, Esc goes back (DismissTracks action + keybinding). Visual mode, dd/dg/dG delete.
- **Copy Album URL**: `o.Y` / global `Y` copies `https://music.youtube.com/browse/{album_id}`.
- **P0 bugs fixed**: merge-into-self guard (source==target silent no-op), album art sixel min-size guard, ConfigEditorPopup cursor style (teal marker via Line+Span).

### Library Tracks Refactor (Phase A+B)
- **Delete re-routed** to LibraryBrowser (was routing to Playlist/queue, zero feedback). `HandleLibraryRemoveItemsOk`/`Err` created targeting LibraryBrowser.
- **Filtered/sorted indices** — all delete handlers (`RemoveTrackFromPlaylist`, `DeleteSelected`, `DeleteToTop`, `DeleteToBottom`) now use `get_tracks_filtered_list_iter()` for correct track selection when sort/filter active.
- **Local removal** — deleted tracks removed from `playlist_tracks` via `video_id` match (not raw index) for immediate feedback.
- **Visual mode range** — uses filtered list for correct visual range when sorted/filtered.
- **DismissTracks** resets `tracks_visual_mode`, `tracks_visual_start`.
- **MoveTrackUp/Down** — uses filtered indices, swaps locally for immediate visual feedback. Re-routed to LibraryBrowser handlers.
- **Reorder re-routed** to LibraryBrowser (`HandleLibraryReorderItemsOk`/`Err`).

### Metadata Pipeline
- **Title cleaning**: strips `(Official Audio)`, `(Official Video)`, `c legenda`, `Legendado`, `subtitle` etc. from titles before metadata lookup. Strips bare artist prefix when no ` - ` separator. Strips extracted years `(2000)` from title before ValidateMetadata.
- **Artist normalization**: `normalize_artist_name()` capitalizes first letter. Applied in `From<ParsedSongArtist>`, `MetadataEffect::Validated`, and `insert_album_tracks`.
- **Discogs artist fix**: was returning `artist: None`, now extracts `artists[0].name` from Discogs Master API response.
- **Discogs search fix**: was using broken `artist=&album=` structured search (ignored album param, returned random albums). Changed to `q=` combined search which matches both terms.
- **Discogs fallback**: when exact `q=artist+album` search returns nothing, falls back to `q=artist` artist-only search to ensure obscure/underground albums still split.
- **CRITICAL: url_added removed** — `play_yt_url()` set `url_added = true` which caused `MetadataEffect::Validated` to skip album splitting for URL-added songs. Removed `url_added` field entirely. URL-added songs now split correctly.
- **Metal API provider**: queries `https://metal-api.dev/` (approved MA REST API) at priority 5. Returns band name, album, year, tracklist. API returns 500 (backend crash). Falls back to local proxy + MA_COOKIE.
- **MA_COOKIE direct access** (Cookie-based Metallum access):
  - Reads `MA_COOKIE` env var, then `~/.config/youtui/ma_cookie` file
  - Makes direct HTTP requests to Metal Archives AJAX API (bypasses Cloudflare)
  - Returns artist, album, year (from `<!-- 2024 -->` comments), full tracklist, genre (from band page)
  - Cookie auto-saved to config file for persistence. Expires ~30 min, refresh via `--get-cookie`
- **metal-proxy** (`libs/metal-proxy/`):
  - Pure background HTTP server on port 5000. No headless browser, no window, no Python — Rust-only.
  - Reads saved cookie, serves MA data via direct HTTP.
  - Background task refreshes cookie from running Chromium via CDP (every 15 min).
  - `--get-cookie` flag: launches Chromium with debug port, tries headless=new first, falls back to visible.
  - Optional: configured via `MA_COOKIE` env var or `~/.config/youtui/ma_cookie` file.
- **Genre aliasing**: 3,713 genres from MusicBee hierarchy (MusicBrainz + Discogs + RYM + Wikidata). `genre_map::normalize_genre()` normalizes provider genres. Integrated into `MetadataRegistry.resolve()`. 26 tests pass.
- **Year fallback**: extract 4-digit year from album name when providers return `None`.
- **CLI debug tool**: `ytmapi debug resolve <artist> <title>` tests full pipeline. `ytmapi debug genre <genre>` / `genre-list [filter]` test genre normalization.

### Bug Fixes
- **Log viewer toggle**: F11 -> ViewLogs now correctly toggles off (was always entering logs, couldn't exit). Esc restore works.
- **Discogs provider**: was returning wrong albums for all queries due to broken `artist=&album=` API parameters. Changed to `q=` combined search.
- **Playlist editor**: Esc and `:q` now warn when modified. `:q!` force quits. Visual selection color fixed from teal to cyan.
- **VL prefix**: `RemovePlaylistItemsQuery` was missing VL prefix stripping (other mutation queries had it). Added.
- **setVideoId**: library tracks now track `SetVideoID` from API response for correct track removal. Falls back to `video_id` when empty.
- **RemovePlaylistItems endpoint**: changed from `browse/edit_playlist` (metadata edits) to `playlist/edit` (content mutations).

### Known Issues
- **Album art popup**: Sixel centering not perfect, sixel persistence after close. Known bug.
- **MA cookie**: `cf_clearance` expires ~30 min. Refresh via `cargo run --release -p metal-proxy -- --get-cookie`.
- **Sort/filter popups**: Column sort and filter popups not wired for library tracks view (Phase C).
- **[SEARCH] indicator**: Missing for library tracks `/` filter (Phase D).

## Session 2026-06-22 (Committed)
- `fix:` lyrics help text — `( ) Prev/Next Lyric | <> Prev/Next Song | [] Seek | Esc/q: Close`
- `chore:` cleanup — removed 6 stale TODOs + dead sort_column from playlist_editor_popup
- `chore:` cleanup — removed dead `AppCallback::Back` (Backspace works via BrowserAction)
- `chore:` cleanup — removed dead `GetPlaylistDetailsFromLibrary` (OpenDetailsPopup is live path)
- `chore:` cleanup — removed stale `#[allow(dead_code)]` from AddPlaylistToPlaylist struct
- `chore:` cleanup — added `library_playlist_mutated = true` to merge success handler
- Album split tags expanded: `full single`, `album` added to strip list

## Session 2026-06-24 (Committed)

### Footer Restructure
- **5-line footer** with `Block::default().borders(Borders::ALL)` title "Status" / right-aligned "Youtui"
- **Album art 7-char wide** (`ALBUM_ART_WIDTH = 7`) with 1-char gap. `Resize::Fit(None)` scales image.
- **Layout**: 3 inner lines — line1 = artist-song, line2 = album (gray) + status icons (default), line3 = progress bar `< [ ] >`.
- **Status icons**: repeat `󰑖`/`󰑗`/`󰑘`, shuffle `󰒝`, scrobble, heart `󰋑` (MDI Nerd Font set). Heart only red.
- `footer.rs`: extracted `like_icon()` as public fn (3 tests).
- `like_icon()`: returns `" 󰋑"` for Liked, `" ♥"` otherwise.
- Footer reserve 6→5 in lyrics_popup.

### Lyrics Popup
- **Space pauses**: `KeyCode::Char(' ')` → `AppCallback::TogglePlayPause`. `lyrics_popup.rs:777-780`.
- **Hint text cleaned**: `"( ) Lyric | <> Song | [] Seek | Space Pause | Esc/q Close"`.
- **Footer reserve**: `top_anchored_rect` reserve updated to 5 lines to match 5-line footer.

### Green Lettering (Playing Indicator)
- **All browser tabs now show green bold text** on currently playing song: Songs, Artists, Albums, Library, PlaylistSearch.
- `view/draw.rs`: changed `secondary_row_highlight_style` from `bg(PLAYING_COLOUR)` to `fg(PLAYING_COLOUR)`.
- `cur_playing_video_id` field added to every browser widget struct.
- `Browser::set_cur_playing_video_id()` propagates playing ID to all widgets.
- `get_highlighted_row()` in each browser returns index matching `cur_playing_video_id`.

### Album Art Popup (o.v)
- Draw logic moved from `album_art_popup.rs` into `draw.rs` for sixel data tracking.
- 95% centered rect (was 90%).
- Stores sixel data in `w.sixel_data` on draw for proper cleanup on close.
- `AppCallback::ViewAlbumCover`: saves `prev_context` before opening.
- `AppCallback::ClosePopup`: handles album art popup as overlay (no context change).
- Dead second draw removed from `draw.rs`.

### Library Tracks Sort/Filter + SEARCH
- `o.z` sort popup, `o.c` filter popup wired for library tracks view.
- `HasTitle::get_title()` returns `Playlist Tracks [SEARCH: text (N/M)]`.
- `get_selected_item()` maps raw selection to filtered index when sort/filter active.
- `get_highlighted_row()` now also checks `cur_playing_video_id`.

### Like/Subscribe from Album Tracks
- `o.t` likes album via `audio_playlist_id` (RatePlaylistFromLibrary).
- `o.S` subscribes to artist, `o.U` unsubscribes (SubscribeToArtistFromLibrary/UnsubscribeFromArtistFromLibrary).

### Force-Split (o.f)
- `PlaylistAction::ForceSplitAlbum`: re-validates metadata and re-splits selected track.
- Handles both cases: parent exists (re-split) and parent removed (use track 1 as new parent).
- Triggers `ValidateMetadata` + `download_upcoming_from_id`.

### Album URL Auto-Detect
- `:` command with `playlist?list=OLAK5uy_...` loads all tracks via `GetPlaylistTracks`.
- Extracted `extract_playlist_id()` and `extract_video_id()` as free functions (11 tests).

### Playlist Editor Overwrite Save
- `o.e` → editor → `:w` → `AppCallback::OverwritePlaylistTracks`.
- Chain: `HandleOverwriteGetTracks` → `HandleOverwriteRemoveDone` → `HandleAddSongsOk`.
- `close_popup()` called, library refreshes on save.

### AppCallback
- New variant `AppCallback::TogglePlayPause` → calls `self.window_state.pauseplay()`. `app.rs:107,688-691`.
- New variant `AppCallback::OverwritePlaylistTracks`.

### New Tests
- **29 new tests**: `like_icon()` (3), `extract_playlist_id()`/`extract_video_id()` (11), `normalize_artist_name()` (6), `score_result()` (9).
- LikeStatus persistence: `CompactSongRef` gains `like_status` field with `#[serde(default)]` backward compat.
- Total youtui tests: 124 (was 103).

### Keybinding Additions
| Key | Action | View |
|---|---|---|
| `o.f` | ForceSplitAlbum | Queue |
| `o.e` | OpenPlaylistEditor | Library tracks |
| `o.t` | RatePlaylist (like album) | Album tracks |
| `o.S` | SubscribeToArtist | Album tracks |
| `o.U` | UnsubscribeFromArtist | Album tracks |
| `o.z` | SortAction::Close | Library tracks |
| `o.c` | FilterAction::Close | Library tracks |

### Metadata Pipeline (Committed This Session)
- **Scoring system**: `MetadataRegistry::score_result()` — +50 tracklist, +20 album match, +10 artist match, +10 year present.
- **Discogs `q=` search**: combined artist+album search (was broken structured `artist=&album=`).
- **Discogs fallback**: when exact match fails, retry with `q=artist` only.
- **url_added removed**: URL-added songs now split correctly.
- **Per-track validation removed**: corrupted correct split-track metadata.
- **MA_COOKIE direct access**: cookie-based Metal Archives bypass (ENV var + config file).
- **metal-proxy**: pure Rust background server on port 5000, cookie-based, no browser.
- **Genre aliasing**: 3,713 genres normalized, 26 tests.

### Fixed Bugs
- Log viewer toggle (F11) now exits properly.
- Year stripping unused variable warning removed.
- Discogs structured search → combined search.
- `url_added` removed (blocked URL song splitting).
- Per-track validation removed (corrupted split-track metadata).
- o.v album art popup: separate ClosePopup handler (no context corruption).
- o.v: sixel data stored in `w.sixel_data` for cleanup.
- **F7 tab cycle**: `handle_change_search_type()` now calls `push_snapshot()` before switching variant. Back-stack no longer corrupted.
- **C-r redo**: config_editor_popup now passes ctrl modifier to ViTextEditor. Notes editor already worked.

### Dead Code Removed
- `yt_dlp.rs`: removed unused `YtDlpDownloader` struct + `YtDlpDownloaderError` enum (app uses rusty_ytdl)
- `effect_handlers_playlist.rs`: removed 4 dead handler structs + impls (HandleReorderPlaylistItem*, HandleRemovePlaylistItems*)
- `library.rs`: removed 3 unused methods (`load_selected_playlist`, `search_text`, `is_search_active`)
- `albumsearch.rs`: removed 5 unused methods (`get_search_suggestions`, `revert_routing`, `apply_all_sort_commands`, `apply_filter`, `clear_filter`)
- `actionhandler.rs`: removed unused `MouseHandler` trait
- `notes_popup.rs`: removed unused `NotesAction` enum + `ActionHandler` impl (Esc/q handled directly)
- `scrolling_list.rs`, `tab_grid.rs`: removed unused methods
- `#[allow(dead_code)]` annotation removed from `WindowContext::Notes` (intentional, re-added)
- Remaining `#[allow(dead_code)]`: notes_popup (intentional dead variant), albumsearch (constructor used from tests)

## Session 2026-06-24 (Batch B — Annotations + Colon + Metadata Pipeline)

### Annotations & Colon Key
- `:` key routing in lyrics popup: intercept before popup handler, route to YoutuiWindow command mode.
- `AppCallback::TogglePlayPause`: space in lyrics popup pauses/resumes playback.
- Genius `find_song` reorder: Bearer search FIRST (real id/path), slug URL fallback only.
- Genius annotations pagination: `per_page=50` → full set (was 10).
- Annotations UI: right-side absolute numbers, Tab/Alt+l/Alt+h focus switch, R romaji guard (has_japanese), conditional hints, visual mode yank (fragment+explanation via wl-copy).
- Notes popup: `close_popup` saves parent popup (lyrics/etc.) when notes stacked on top.
- `:notes` command: sets `prev_context` + `context = WindowContext::Notes`.

### Metadata Pipeline Fixes
- `score_result`: artist exact match +50 (was +3), contains +10 (was +1), penalty -500 for clear artist mismatch.
- Discogs `find_artist_result`: filter search results by artist name before selecting master.
- `normalize_artist_name`: strip " - Topic", Discogs "(N)" suffix, bracket prefix `[hate5six] Artist`, all-caps → proper case.
- `add_raw_search_result_song`: normalize artist name on creation.
- Album name cleanup: strip "YouTube: " prefix, " - Topic" suffix, bracket prefix.
- Year fallback: extract 4-digit year from song title when metadata returns None.
- Tags added: self-titled, self titled, s/t to album metadata stripping.

### Album Split Fixes (Browser Play Path)
- `push_song_list`: spawns `ValidateMetadata` for first added song (browser play path, was missing).
- `clean_title_for_metadata` helper: extracted from `add_yt_video` inline, used by both paths.
- Album name priority in `insert_album_tracks`: metadata-discovered album > YouTube uploader name.
- `MetadataEffect::Validated`: year-from-title fallback after album fallback.

### CLI Debug Tool
- `ytmapi-cli genius`: search/annotations/lyrics/all subcommands for end-to-end testing.

### Keybinding Changes
| Key | Action | View |
|---|---|---|
| `Alt+l` | Focus annotations panel | Lyrics popup (annotations) |
| `Alt+h` | Focus lyrics panel | Lyrics popup (annotations) |
| `Tab`/`BackTab` | Focus switch (unchanged) | Lyrics popup |

### Test Updates
- `norm_uppercase`: `"METALLICA"` → `"Metallica"` (new all-caps normalization).
- All 6 scoring tests updated for new weights.

### Previous Session Features (Unchanged)
- Metadata pipeline (providers, Discogs, MA_COOKIE, genre aliasing).
- Library tracks Phase A+B (delete re-route, filtered indices).
- Heart icon persisted across sessions via queue save/load.
- Force split (`o.f`), Playlist editor (vim-driven, overwrite save).
- Album URL auto-detect (playlist-based URLs).
- Like/subscribe from album tracks.

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
- **Album art popup**: Sixel centering/sizing not fully correct. Image may appear small or off-center. Sixel persistence after popup close can corrupt main window. Known bug - needs dedicated sixel layer management.
- **Playlist merge into self**: Guard added against identical source/target in `playlist_update_popup.rs`.
- **Cursor style**: Notes popup + ConfigEditorPopup now render cursor with teal background via line-by-line `Span` approach.
- **Metal-API (metal-api.dev)**: Approved REST API for Metal Archives. Currently returns 500 errors (backend crash). Provider code is written but API must be back online.
- **Year metadata**: Some tracks still show `None` for year when no metadata provider returns a year and album name has no year string. Fallback extracts from album name `(YYYY)`.
- **MA_COOKIE**: `cf_clearance` cookie from Metal Archives expires ~30 min. Must be refreshed periodically via `cargo run --release -p metal-proxy -- --get-cookie` or manual browser extraction.
- **Album `audio_playlist_id`**: May be `None` for some album types (singles, EPs). `o.t` silently no-ops.
- **Related tracks metadata**: YTM watch-playlist API returns no album/year. Artist extracted from channel name only.
- **Album URL tracks bypass metadata pipeline**: `GetPlaylistTracks` loads songs without `ValidateMetadata`. No album splitting for these.
- **Force-split visual feedback**: No toast/notification on success/failure. Check logs.
- **Playlist editor modified check**: `Esc`/`:q` warns on unsaved changes. `:q!` force-quits.
- **Sixel album art**: May persist after popup close. Temp workaround: resize terminal.

## Remaining Items (Detailed)
### Recommended Order
1. **Annotations integration** (P1)
2. **Sixel album art persistence** (P2)
3. **P2/P3 items** (polish, no data-loss)

### P1: ~~Back navigation (F7 cycle) — FIXED.~~

### P1: Annotations integration + `:` command in lyrics
**Problem**: Lyrics popup has Tab/l/h for switching between lyrics/annotations modes. `:` command (OpenUrl) doesn't work inside lyrics popup — popup intercepts keys before global handler. Annotations display needs end-to-end verification with GENIUS_TOKEN.

**Files**: `youtui/src/app/ui/playlist/lyrics_popup.rs`, `youtui/src/app/ui/playlist/annotations_popup.rs`, `app.rs`

### P2: Visual mode color — all highlighted lines cyan
**Problem**: In queue visual mode, first line shows green lettering (even when not playing). Rest highlighted purple. All highlighted lines should use consistent cyan bg.

**Files**: `youtui/src/app/ui/playlist.rs`, `youtui/src/app/view/draw.rs`

### P2: FFT footer bars (low priority)
**Problem**: No FFT frequency bars in footer (roadmap feature, not wired yet).

### P2: Sixel album art persistence
**Problem**: Sixel centering/sizing not fully correct. Sixel persistence after popup close can corrupt main window. Partially fixed: sixel data stored in `w.sixel_data` for cleanup. Root cause: `\x1bP0p\x1b\\` DCS clear not supported on all terminals (foot may handle it intermittently). Fix: render blank sixel over popup area on close.

**Files**: `youtui/src/app/ui/draw.rs`

### P3: Like album to library
**Problem**: `o.t` rates audio_playlist_id (likes the playlist) but doesn't add album to user's library/Albums section in YT Music profile. Needs new API endpoint for "add to library" for albums.

### P3: Genius annotations fallback (page scrape)
**Problem**: Without `GENIUS_TOKEN`, `__INITIAL_STATE__` scraping fails on most pages. Need a fallback web-scraping path.

**Files**: `genius-rs/src/annotations.rs`

### P3: Genius lyrics: Musixmatch integration
**Problem**: Genius lyrics only. No Musixmatch/LRCLIB fallback for songs without Genius entries.

### P3: ytmapi-cli more fixture types
**Problem**: CLI debug tool needs more fixture types (browse/search endpoints).

### P3: Crate extraction: audio-player
**Problem**: Audio player logic embedded in youtui binary. Should extract to separate crate.

### P3: Count-in-header standardization
**Problem**: Some browser tables show "N results", others don't. Standardize.

### P3: Album browser j/k routing when show_tracks
**Problem**: When album tracks are shown inline in AlbumsBrowser, j/k navigation doesn't move through tracks.

### P3: ytmapi-rs 150 TODOs
**Problem**: ~150 pre-existing TODO comments in `ytmapi-rs/src/parse/search.rs` and `parse/artist.rs` for type safety improvements.

### P3: ytmapi-rs 150 TODOs
**Problem**: ~150 pre-existing TODO comments in `ytmapi-rs/src/parse/search.rs` and `parse/artist.rs` for type safety improvements.

### P3: Metadata pipeline year coverage
**Problem**: Some tracks show `None` for year. Need more fallback sources.

### P3: RYM cookie proxy
**Problem**: RateYourMusic has genre/descriptor data. Cloudflare-blocked, no public API. Could use same MA_COOKIE pattern (RYM session cookie + reverse-engineered internal API). Exploratory.
