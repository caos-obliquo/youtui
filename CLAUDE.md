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
- **PR-only merges to main**: feature branch → PR → review → merge. No direct pushes to `main`. Pre-push hook enforces this; `git push origin HEAD:feature-branch` for new branches.

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
- **Compact UI**: Minimal visual noise, information-dense layouts. Footer shows 2 lines: line 1 = playing indicator + `Artist - Song`, line 2 = album + status icons.
- **Terminal**: foot (Wayland native). Sixel graphics support but DCS clear is unreliable. Design fallbacks.
- **Docs are code**: CLAUDE.md, TODO.md, docs/ must stay current with every commit. Stale docs = bug.
- **Dead code is liability**: Remove unused structs, methods, annotations on sight. Keep only what compiles and is wired.
- **Doc hygiene is hard rule**: Every doc change MUST cross-reference ALL related docs (CLAUDE.md, TODO.md, docs/*.md) and update stale info. No orphan doc updates. Verify test counts, file paths, line counts, and feature status after every edit. Stale docs = bug.
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
cargo test --release -p youtui --bin youtui       # 161 pass, 4 ignore
cargo test --release -p metadata-provider          # 47 pass
cargo test --release -p vi-text-editor             # 65 pass
cargo test --release -p ytmapi-rs --lib            # 85 pass (no auth)
cargo test --release -p ytmapi-rs                  # 29/51 auth (needs cookie)
cargo test --release -p genius-rs                  # 18 pass
cargo test --release -p async-callback-manager     # 14 pass (3 lib + 11 integ)
cargo test --release -p json-crawler               # 2 pass (0 lib + 2 doctests)
cargo test --release -p ytmapi-cli                 # 7 pass
cargo test --release -p lrclib-rs                  # 4 pass
cargo test --release -p rym-genre-data             # 10 pass
```
Total: **~413/413 pass, 0 fail, 4 ignored, 0 warnings** (161 + 47 + 65 + 85 + 18 + 14 + 2 + 7 + 4 + 10 = 413)

## Warnings
`cargo build --release` — **0 warnings across workspace** (all 11 crates clean).

## Performance (PR #3 — perf/enter-cancel-render)
Batch of 6 perf fixes merged 2026-06-26, ea2fc1c:
- **Render throttle**: `needs_redraw` bool + 33ms `tokio::time::Interval` max ~30fps. No 1000fps on key spam.
- **Stale download cancel**: `cancel_all_downloads()` calls `.cancel()` on each `CancellationToken` before `clear()`. Was leaking tokens.
- **Enter-spam guard**: `PlayDebouncer` struct — 300ms cooldown on `AddSongsToPlaylistAndPlay` via `Instant` check in `handle_callback`.
- **Library lazy iterator**: `get_filtered_items()` returns `Box<dyn Iterator>` instead of eager `.collect()` into `Vec<Vec<Cow>>`. Eliminates O(n) heap alloc per frame.
- **Footer protocol cache**: `cached_album_protocol: Option<Protocol>` in `YoutuiWindow`. Skips CPU-heavy `new_protocol()` re-encode when `Rc::ptr_eq` shows album art unchanged. `invalidate_protocol_cache()` method added.
- **Help menu single-pass**: Collects `DisplayableKeyAction` into owned `[String; 3]` once, reuses for widths + table render. Was calling `get_help_list_items()` twice per draw.

Tests: 15 new unit tests added (5 PlayDebouncer, 3 protocol cache, 3 download cancel, 4 library lazy iterator).

## Arch (3-layer async callback)
```
Frontend (UI) -> TaskManager -> Backend (Server)
```
See `docs/` for full reference (4.1k lines, 31 files).

## 11 Workspace Crates (50k+ LOC)
| Crate | Status | Tests |
|---|---|---|
| `youtui` | Main binary | 161 |
| `ytmapi-rs` | YT Music API client | 85 lib + 29/51 auth |
| `vi-text-editor` | Vim text editor widget | 65 |
| `metadata-provider` | Metadata trait + impls | 47 |
| `genius-rs` | Genius lyrics/annotations | 18 |
| `async-callback-manager` | Async task dispatch | 14 |
| `json-crawler` | JSON path parser | 2 |
| `ytmapi-cli` | CLI debug tool | 7 |
| `lrclib-rs` | LRCLIB lyrics provider | 4 |
| `rym-genre-data` | RYM genre/descriptor hierarchy | 10 |
| `audio-player` | Async rodio-based audio player | 0 |

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
| `youtui/src/app/server/messages.rs` | ~1598 | All backend tasks |
| `youtui/src/app/ui/playlist.rs` | ~3104 | Queue, playback, album splitting, visual mode |
| `youtui/src/app/ui/browser.rs` | ~1012 | Browser routing, 5-tab dispatch |
| `youtui/src/app/ui/browser/draw.rs` | ~517 | All browser draw functions |
| `youtui/src/app/ui/browser/library.rs` | ~2123 | Library (4th tab) with inline tracks view |
| `youtui/src/app/ui/browser/albumsearch.rs` | ~731 | Albums tab (refactored, like/subscribe/audio_playlist_id) |
| `youtui/src/config/keymap.rs` | ~2142 | All keybindings by context |
| `youtui/src/app/ui.rs` | ~1779 | Main window, event routing |
| `libs/metadata-provider/` | 47 tests | Metadata trait + 6 provider impls + genre_map |
| `youtui/src/app/ui/playlist/notes_popup.rs` | ~254 | Vim-driven notes text editor |
| `youtui/src/app/ui/playlist/playlist_editor_popup.rs` | ~748 | Playlist editor (nvim-driven, overwrite save) |
| `youtui/src/app/ui/playlist/album_art_popup.rs` | ~54 | Album art sixel popup w/ pagination |
| `youtui/src/app/ui/playlist/config_editor_popup.rs` | ~153 | Config file editor |
| `youtui/src/app/ui/playlist/lyrics_popup.rs` | ~1210 | Lyrics + annotations display |
| `youtui/src/app/ui/footer.rs` | ~275 | Footer: progress, metadata, heart icon, album art |
| `youtui/src/app/ui/playlist/effect_handlers_playlist.rs` | ~1174 | ValidateMetadata, overwrite save chain handlers |

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
`:w` Save | `:wq` Save+Quit | `:q` Quit | Enter on URL: Open | `i` Insert | `V` visual line | `C-v` visual block | `y` yank | All VTE motions (j/k/h/l/gg/G/w/b/dd/yy/p/P/u/C-r/o/O)
- Starts in Normal mode (navigate with j/k, edit with i)
- `scroll_offset` keeps cursor visible in long files
- Esc exits Insert/Visual mode to Normal (never closes popup)

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

See `docs/09-roadmap.md` for detailed session history.

## Notes Popup Features
- Vim-driven text editor for storing URLs, song links, personal notes
- File: `~/.config/youtui/notes.txt` — plain text, persists across sessions
- Keybindings: `:w` save, `:wq` save+quit, `:q` quit
- Enter on URL line → opens in yt-dlp
- Full ViTextEditor support: j/k/h/l/gg/G/w/b, dd/yy/p, u/C-r, visual line/block
- Starts in Normal mode (navigate with j/k, edit with i)
- `scroll_offset` keeps cursor visible in long files
- Esc exits Insert/Visual mode to Normal (never closes popup)
- System clipboard yank via `wl-copy` in visual mode
- See `docs/subsystems/notes.md` for full architecture

## Genius API
- Token: `5e4pF3nYzWG-xHFdpQpmX-nkjfLjZODc4PUBIQrphwHnbnCkjmS3x0pewYHY33Sq` in config
- CLI: `GENIUS_TOKEN=... genius-rs annotations "Artist" "Song"`
- Search: `genius-rs search "Artist" "Song"` → returns real song ID (1063 etc.)
- Hit validation: `find_and_fetch` rejects results where final URL redirects to non-Genius page
- Bearer search prioritized over slug URL when token available (gives real song ID)

## Known Issues
- **Genius lyrics**: `find_and_fetch` slug URL fails for songs with parenthetical/bracketed title extras (e.g., "(Japanese Bonus Track)"). Simplified slug fallback added but may not match all cases.
- **Auth tests**: 51 ytmapi-rs integration tests need cookie file.
- **Metal-API (metal-api.dev)**: Approved REST API for Metal Archives. Currently returns 500 errors (backend crash). Provider code is written but API must be back online.
- **Year metadata**: Some tracks still show `None` for year when no metadata provider returns a year and album name has no year string. Fallback extracts from album name `(YYYY)`.
- **MA_COOKIE**: `cf_clearance` cookie from Metal Archives expires ~30 min. Must be refreshed manually via browser DevTools > Application > Cookies. The `metal-proxy` crate has been removed from workspace (backend API returns 500).
- **Album `audio_playlist_id`**: May be `None` for some album types (singles/EPs). `o.t` shows feedback message now.
- **Playlist editor modified check**: `Esc`/`:q` warns on unsaved changes. `:q!` force-quits.
- **Sixel album art**: Belt-and-suspenders clear on close fixed in af0acb8. Sixel cleared via `\x1bP0p\x1b\\` DCS clear at start of every draw, plus offset tracking via `sixel_rect` for proper area management.
- **Scrobbler rate limit**: Rescrobbled systemd service double-submits scrobbles. Must stop/disable rescrobbled before using native scrobbler. `sudo systemctl stop --user rescrobbled && sudo systemctl disable --user rescrobbled`.
- **Scrobble cache**: Persistent retry file at `~/.config/youtui/scrobble_cache.json`. Failed scrobbles saved to disk with retry count (max 3). Retried on startup + background 5-min loop. Rate limit stops retries to avoid hammering.
- **Protocol cache (chunk dimensions)**: `cached_album_chunk` tracks image chunk dimensions in footer. `chunk_changed` comparison prevents 8-bit fallback on terminal resize (PR #8).
- **o.v zero-pixel guard**: Zero-width/height `in_mem_image` shows 'No image data' instead of attempting to render empty sixel (PR #9).

## Scrobbler Integration

### Fixes in `fix/scrobbler-signature` branch
- **Signature sort**: Last.fm API requires params sorted alphabetically before signing (`params.sort_by()` added before HMAC signing)
- **Album track scrobbling**: Removed `should_scrobble()` guard on album track submission — all album split tracks now scrobble
- **scrobble_pending guard**: `self.scrobble_pending = false` cleared in `play_song_id()` and `stop()` to prevent stale state
- **Rescrobbled spawn removed**: `extend_rescrobbled_process_keepalive` dropped — no duplicate scrobbles from systemd+embedded scrobbler
- **5 scrobbler tests**: Unit tests covering scrobble state timing, session_key usage, signature sorting, rate limiting, error handling

### Persistent Scrobble Cache
- File: `~/.config/youtui/scrobble_cache.json` — JSON array of failed scrobble payloads
- `save_failed_scrobble()` — writes failed submission to disk with retry_count field
- `retry_failed_scrobbles()` — called on YoutuiWindow::new() startup; retries cached failures
- `remove_cached_scrobble()` — removes entry after successful retry (`#[allow(dead_code)]`, used only in tests)
- Max retries: 3 per entry (incremented each attempt, entries dropped after 3 failures)
- Max cache size: 200 entries (oldest evicted when exceeded)
- `ScrobbleResult` enum: `Success`, `Failure(String)`, `RateLimited` — rate limit stops retry loop
- Background retry: 5-minute interval loop in main event loop, retries cached failures continuously until cleared or rate limited

### CLI Scrobble Test Tool
- `youtui test-scrobble` — direct scrobble submission command
- Usage: `youtui test-scrobble --artist "Artist" --title "Song" --album "Album" --duration 180`
- Prints full params + API response + timing info
- Tests the full scrobble pipeline: session_key retrieval, HMAC signing, Last.fm API submission



## Remaining Items (Detailed)
### Blocked: Cross-platform clipboard
**Problem**: Clipboard yank uses Wayland-only `wl-copy`. No X11/macOS fallback.

### P3: ytmapi-rs ~68 remaining TODOs
**Problem**: ~37 legitimate TODOs remaining (artist categories, i18n, continuations, unfulfilled feature fields). All LOW value for youtui.

### Ann: Annotation wrapping — fixed
**Problem**: Last annotation entry partially cut off with very long explanation text. **Fixed**: Wrapping-aware line counting added, accounts for Paragraph widget line-wrapping of long explanation lines.

### Feature: Liked songs in browser tables
**Problem**: `LikeStatus` only visible on currently playing track (footer heart icon). Not shown in Songs/Albums/Library browser tables.
**Plan**: Parse `like_status` from YTM search response (`SearchResultSong`), add "Liked" column to `AdvancedTableView`. `AlbumSong` already has `like_status` field available. Medium effort.

## Phase Tracking (from m0094 — updated 2026-06-25)

### Phase 1 ✅ — Small UI fixes
1. Annotation panel last entry cut-off (`lyrics_popup.rs`)
2. Force-split visual feedback (`playlist.rs`, `effect_handlers_playlist.rs`)
3. Album `audio_playlist_id` None guard (`albumsearch.rs`)

### Phase 1.5 ✅ — Scroll-centering + early Library fixes
1. Vim centered-scrolling (all table views: `scrolling_table.rs`, `draw.rs`)
2. Library Albums format (`Artist - Album`)
3. Browser Albums auto-load removed (`albumsearch.rs`, `browser.rs`)
4. GoToArtist in Library (`library.rs`)

### Library Page Revision ✅ — Complete overhaul
1. Context menu per-category filtering (`songsearch.rs`, `ui.rs`, `browser.rs`)
2. GoToAlbum→AlbumOpen direct tracks (`library.rs`, `app.rs`, `albumsearch.rs`, `browser.rs`, `ytmapi-rs`)
3. Enter: Artists→channel, Albums→AlbumOpen
4. F1 search all categories (`library.rs`)
5. `/` filter all 4 categories (`draw.rs`)
6. `/` filter guard rail — zero command bleeding (`ui.rs`, `browser.rs`)
7. Subscribe single toggle S key (`songsearch.rs`, `library.rs`, `keymap.rs`)
8. Plays column preserved from YTM (`structures.rs`)
9. Lowercase artist names (`structures.rs`)
10. Album art vanish fix — DCS clear only in popup block (`draw.rs`)
11. RatePlaylist for Library Albums (`library.rs`)
12. Removed hardcoded "No albums/playlists found" (`draw.rs`)

### Metadata Pipeline Fixes ✅
1. Fallback split guard — requires `video_dur` OR album tags (`effect_handlers_playlist.rs:655`)
2. Album override guard — keep YTM when present (`effect_handlers_playlist.rs:578`)
3. Track-presence check — reject wrong album split (`effect_handlers_playlist.rs:652`)
4. DiscogsProvider track validation (`discogs.rs:102-111`)
5. AlbumSearchProvider track validation (`lastfm_album.rs:98-111`)
6. TrackSearchProvider title matching (`lastfm_track.rs:55-59`)
7. normalize_artist_name — ALL-CAPS preserved (`structures.rs:176-181`)
8. 6-provider metadata audit completed (MetalApi, MusicBrainz, Discogs, Last.fm Album/Track, Genius)

### Phase 2 ✅ — Genius fallback + Year coverage + Scoring
1. Genius annotations fallback — `__PRELOADED_STATE__ = JSON.parse('...')` extraction ✅
2. FFT footer bars — cancelled (cosmetic, user skipped) 
3. Year coverage gaps — Library LikedSongs + title parenthetical fallbacks ✅
4. Metadata scoring review (+100 tracklist bias gated) ✅

### Phase 3 ✅ — Musixmatch/LRCLIB + RYM genre data
1. LRCLIB lyrics crate (`libs/lrclib-rs/`) with CLI debug tool ✅
2. RYM genre/descriptor data from pre-scraped GitHub (`libs/rym-genre-data/`) ✅
3. RYM genre descriptions in song info popup (`song_info_popup.rs`) ✅

### Phase 4 ✅ — audio-player crate extraction
- `libs/audio-player/` extracted from `async_rodio_sink.rs` ✅
- 7 files' import paths updated; old file deleted ✅

### Phase 5 ✅ — Related tracks metadata enrichment
- yt-dlp per-video bounded concurrent (max 30, 5 semaphore) ✅
- `EnrichRelatedTracks` task + `HandleEnrichRelatedTracksOk`/`Err` handlers ✅

### Phase 6 🔴 — Cross-platform clipboard
- Wayland-only `wl-copy`. No X11/macOS fallback.

## Suckless Refactoring (refactor/suckless branch)
Goal: Clean, minimal, robust codebase. 5-batch plan in `docs/refactor-suckless.md`.

### Done
| Batch | Item | Δ Lines | Status |
|---|---|---|---|
| 1 | Replace 6 panic paths with proper error handling | -0 | `48c7eaa` |
| 2 | Delete dead crates (metal-proxy, rym-definitions) | -606 | `19f4e46` |
| 3 | Extract boilerplate (7 CRUD macro pairs, conversion fn, thumbnail fn) | -24 | `7fc6252` |
| 4a | Subdivide MetadataEffect::apply (180→40 lines) | -0 | `35bf646` |
| 4b | Extract clean_title_for_metadata into 4 named helpers | -0 | `35bf646` |
| 4d | Extract handle_force_split from apply_action (75→1 line arm) | -0 | `096fa0f` |
| **Total** | | **-630** | |

### Not Done (low value)
| Skipped | Reason |
|---|---|
| Batch 4c: handle_callback split | Most arms are 1-3 lines, splitting adds indirection |
| Batch 4e: api.rs retry dedup | Complexity too high for 15-line savings |
| Batch 4f: keymap.rs dead bindings | No automated dead binding detection |
| Batch 5: error swallows | Sixel writes are intentional no-ops (terminal disappear) |

### Verification
- 161/161 pass, 4 ignored, 0 warnings across workspace
- Suckless refactoring adds 0 tests (refactors existing code only)
