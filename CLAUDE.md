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
| `ytmapi-cli` | CLI debug tool | 7 |
| `metal-proxy` | Metallum Chrome proxy | 0 |

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
| `app/ui/browser/library.rs` | ~1269 | Library (4th tab) with inline tracks view |
| `app/ui/browser/albumsearch.rs` | ~705 | Albums tab (refactored) |
| `config/keymap.rs` | ~2079 | All keybindings by context |
| `app/ui.rs` | ~1584 | Main window, event routing |
| `libs/metadata-provider/` | 19 tests | Metadata trait + 5 provider impls |
| `app/ui/playlist/notes_popup.rs` | ~272 | Vim-driven notes text editor |
| `app/ui/playlist/playlist_editor_popup.rs` | ~484 | Playlist editor (nvim-driven) |
| `app/ui/playlist/album_art_popup.rs` | ~41 | Album art sixel popup |
| `app/ui/playlist/config_editor_popup.rs` | ~146 | Config file editor |
| `docs/subsystems/notes.md` | ~100 | Notes popup architecture doc |

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
- **Album art popup**: `o.v` opens full-screen centered image via `centered_rect_fixed(90,90)` + `Resize::Fit(None)`. Issues with sixel persistence/centering documented as known bug.
- **Footer 2-line metadata**: Artist-Song on line 1, Album indented gray on line 2. Overflow truncation with `...`. Fallback album art position fixed (was using hardcoded `Rect { x:0, y:0 }`).
- **Playlist editor nvim-driven**: undo stack (100-level), yank/paste (yy/p/P), visual mode (V->j/k->d/y), D=dG, Y=yy, o/O insert blank line, count prefix for all motions/ops, delete/yank operator modes. 4-block capacity bar (`Tracks: N/5000 [■■■■]...`).
- **Library playlist tracks inline**: uses `draw_advanced_table` with proper columns (#, Artist, Album, Song, Duration, Year). Left category panel hidden. Enter plays song, Esc goes back. Tracks sortable/filterable.
- **Vim motions in library tracks**: V visual mode, dd/dg/dG delete, D dismiss tracks.
- **Copy Album URL**: `o.Y` / global `Y` copies `https://music.youtube.com/browse/{album_id}`.
- **P0 bugs fixed**: merge-into-self guard (source==target silent no-op), album art sixel min-size guard, ConfigEditorPopup cursor style (teal marker via Line+Span).
- **Warnings**: 0 across workspace.
- **Title cleaning**: strips `(Official Audio)`, `(Official Video)`, `c legenda`, `Legendado`, `subtitle` etc. from titles before metadata lookup. Also strips bare artist prefix when no ` - ` separator.
- **Artist normalization**: `normalize_artist_name()` capitalizes first letter. Applied in `From<ParsedSongArtist>`, `MetadataEffect::Validated`, and `insert_album_tracks`.
- **Discogs artist fix**: was returning `artist: None`, now extracts `artists[0].name` from Discogs Master API response.
- **Metal API provider**: new `metal_api.rs` in metadata-provider crate. Queries `https://metal-api.dev/` (approved Metal Archives API) at priority 5. Returns band name, album, year, tracklist. Currently API returns 500 errors (backend crash).
- **Year fallback**: extract 4-digit year from album name when metadata providers return `None`.
- **CLI debug tool**: `ytmapi debug resolve <artist> <title>` tests full metadata pipeline (title cleaning -> normalization -> provider cascade).

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
- **Metal-API (metal-api.dev)**: Approved REST API for Metal Archives. Currently returns 500 errors (backend crash). Provider code is written and tested with CLI tool, but API must be back online for production use.
- **Year metadata**: Some tracks still show `None` for year when no metadata provider returns a year and album name has no year string. Fallback extracts from album name `(YYYY)`.

## Remaining
### P1 — Missing features (wired backend, need frontend)
- **Like status in details popup**: `GetPlaylistDetails.like_status` already parsed, just not rendered. Details popup `playlist_details_popup.rs:draw()` — add 1 line showing rating.
- **Back navigation (remaining gap)**: `BrowserAction::Back` + `state_stack` works for `Navigate()` but `handle_change_search_type()` (F7 tab cycle) doesn't push snapshots. Stack can restore wrong tab.
- **Playlist overwrite mode**: `app.rs:481` — fetch current tracks + remove before add
- **Annotations integration**: verify Tab/l/h switching is fully wired

### P2 — Polish
- FFT footer bars (roadmap)
- Fix test counts in docs (json-crawler 8→2, async-callback-manager 15→14)
- Footer: upgrade indicator icons (shuffle/repeat/scrobble) to monochrome, show explicitly in footer space
- Album art popup: fix sixel centering and reuse (known bug)

### P3 — Tech debt
- Genius annotations fallback (page scrape)
- Genius lyrics: Musixmatch integration
- ytmapi-cli: more fixture types
- Crate extraction: audio-player
- Count-in-header standardization
- Album browser j/k routing when show_tracks
- ytmapi-rs: 150 pre-existing TODOs (parse/search.rs, parse/artist.rs type safety)
- **Metadata pipeline**: Improve year coverage - add more fallback sources for year extraction. Metal-API provider needs backend fix at metal-api.dev.
