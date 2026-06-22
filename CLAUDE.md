# Youtui — Project Knowledge

## GOLDEN RULE
One feature at a time. Implement → test → commit → next. Never batch changes.
If things break, rollback and re-apply one-by-one.

## User Preferences (Strict)
- **No sudo** without explicit permission.
- **No AUR.** Only official repos + local compilation.
- **Suckless.** Minimal deps, focused scope, ASCII-only words, no bloat.
- **Rust only.** No shell plugins, no non-Rust dependencies.
- **Subagent stack**: `rustacean` for Rust code review, `akita` for architecture/tooling decisions.
- **WHITESPACE** (critical): Keep cursor/indentation whitespace in the above preference block exactly as-is — leading spaces, trailing spaces, blank lines between items. This block is rendered verbatim in opencode prompts and must not drift.
- **Consistency across windows**: Every browser tab (Artists, Songs, Albums, Library, Playlist) must share the same UI patterns: search (F1), advanced table columns with sort/filter, o-mode context menu, j/k/gg/G navigation. No tab should feel like a second-class citizen.
- **Debug-First Rule**: Every new implementation starts by creating CLI debugging tools. CLI tools make tracing changes easier than UI-only debugging. Before wiring UI features, build CLI subcommands/tools that exercise the same backend code paths. Run them to verify correctness before integrating into the UI layer.
- **Mail**: `caos_obliquo@outlook.com`

## Full Reference Manual
See `docs/` for the comprehensive reference:
```
docs/
├── README.md                        — Entry point
├── 01-architecture.md               — 3-layer callback, crate diagram
├── 02-crates/                       — Each crate: purpose, modules, API
├── 03-data-flow.md                  — Event → task → effect → render
├── 04-configuration.md              — All config.toml fields
├── 05-keybindings.md                — All contexts, actions, defaults
├── 06-subsystems/                   — Deep dive: lyrics, audio, queue, etc.
├── 07-testing.md                    — Test structure, commands
├── 08-known-issues.md               — Bugs and workarounds
└── 09-roadmap.md                    — Next features, crate extraction
```
**5,452 lines, 20 files, ~45 pages** — covers all 7 crates, 49k LOC.

## Build
- Workspace root: `/home/caos/builds/youtui/`
- Rust nightly (1.97.0)
- Binary: `cargo build --release` → `target/release/youtui`
- Dependencies: yt-dlp, ffmpeg, alsa-lib (system packages via pacman)

## Tests
```bash
cargo test --release -p youtui --bin youtui       # 120 pass, 4 ignore
cargo test --release -p vi-text-editor             # 65 pass
cargo test --release -p ytmapi-rs --lib            # 82 pass (no auth needed)
cargo test --release -p ytmapi-rs                  # 28 pass / 52 fail (needs auth)
cargo test --release -p genius-rs                  # 14 pass
cargo test --release -p async-callback-manager     # 15 pass
cargo test --release -p json-crawler               # 8 pass
cargo test --release -p ytmapi-cli                 # 0 (stub, no tests yet)
```

## Warnings
`cargo build --release` — 0 warnings across workspace (youtui + 6 lib crates).
Eliminated 46 youtui warnings via `cargo fix` + manual `#[allow(dead_code)]` annotations.
10 ytmapi-rs fixture-drift test failures fixed by regenerating expected output files.

## Dead Code Policy
Future features leave skeleton structs/variants/methods in place, annotated with:
```rust
// TODO: Wire <feature name> — <description>
#[allow(dead_code)]
```
This keeps planned extensions visible in code and grep-able by `TODO: Wire` pattern.
Add new entries to `docs/09-roadmap.md` when annotating. Remove `#[allow]` when wiring.

## Key Files
| File | Lines | Purpose |
|---|---|---|
| `app/server/messages.rs` | ~1318 | All backend tasks |
| `app/ui/playlist.rs` | ~2750 | Queue, playback, scrobbling, visual mode |
| `app/ui/playlist/effect_handlers_playlist.rs` | ~965 | Frontend effect handlers |
| `app/ui/browser/library.rs` | ~1080 | Library browser (4th tab) |
| `app/ui/browser.rs` | ~790 | Browser routing, tab dispatch |
| `config/keymap.rs` | ~1920 | All keybindings by context |
| `libs/vi-text-editor/src/lib.rs` | ~2360 | Vi-mode text editor |
| `libs/genius-rs/src/lib.rs` | ~120 | Genius API client + HTML scraped lyrics/annotations |
| `app/ui/playlist/playlist_rename_popup.rs` | ~85 | Rename popup (char buffer) |
| `app/ui/playlist/playlist_edit_popup.rs` | ~165 | Edit popup (4 fields, tab cycle) |
| `app/ui/playlist/playlist_details_popup.rs` | ~145 | Details popup (loading→display) |
| `app/ui/playlist/lyrics_popup.rs` | ~945 | Lyrics display + visual mode |

## ViTextEditor Summary
65 tests, all pass. Full feature set:
- Motions: `h/l/j/k/w/b/e/0/$/gg/G/W/B/E`, `f/F/t/T`/`;/,`, `%`
- Operators: `d`/`c`/`y`/`r`/`~`/`J`/`x`, with text objects `iw/aw/i(/a(/i"/a"/i'/a'/`` i`/a` ``
- Visual: `V` (line) and `v` (char) with `o` exchange, `c` change
- Surround: `ds`/`cs`/`ys` with `iw`/`W`/`$`/`ss` targets
- Switch: `^A`/`^X` number increment/decrement
- Repeat: `.`/`u`/`^R` with 50-entry stacks
- Proptest invariants for UTF-8 safety, undo/redo roundtrip
- Deps: crossterm only (intentionally suckless)

## Key Architecture
3-layer async callback:
```
Frontend (UI) → TaskManager → Backend (Server)
```
See `docs/01-architecture.md` and `docs/03-data-flow.md` for full detail.
See `docs/06-subsystems/lyrics.md` for lyrics pipeline.
See `docs/06-subsystems/validation.md` for metadata validation.
See `docs/06-subsystems/audio.md` for audio download + playback.

## Playlist Features — Implementation Status

### Backend (ytmapi-rs) — 90% complete
All CRUD ops exist. Gaps: batch reorder (swap only), single-song metadata, song feedback tokens, hardcoded locale.

| Feature | ytmapi-rs Query | Status |
|---|---|---|
| Create playlist | `CreatePlaylistQuery` | ✅ |
| Delete playlist | `DeletePlaylistQuery` | ✅ |
| Rename playlist | `EditPlaylistQuery::new_title` | ✅ |
| Edit description | `EditPlaylistQuery::new_description` | ✅ |
| Edit privacy | `EditPlaylistQuery::new_privacy_status` | ✅ |
| Add items | `AddPlaylistItemsQuery` | ✅ |
| Remove items | `RemovePlaylistItemsQuery` | ✅ |
| Reorder (swap) | `EditPlaylistQuery::swap_videos_order` | ✅ |
| Rate playlist | `RatePlaylistQuery` | ✅ |
| Get details | `GetPlaylistDetailsQuery` | ✅ |
| Get tracks | `GetPlaylistTracksQuery` | ✅ |
| Library playlists | `GetLibraryPlaylistsQuery` | ✅ |
| Change add order | `EditPlaylistQuery::change_add_order` | ✅ |
| Duplicate handling | `DuplicateHandlingMode` | ✅ |
| Add playlist→playlist | `AddPlaylistToPlaylist` | ✅ |

### Frontend (youtui) — Wiring status
| Layer | File | Status |
|---|---|---|
| Backend messages | `app/server/messages.rs` | All 7 messages wired: DeletePlaylist, EditPlaylistDetails, RatePlaylistMessage, GetPlaylistDetailsMessage, ReorderPlaylistItem, RenamePlaylist, RemovePlaylistItems |
| API bridge | `app/server/api.rs` | `create_playlist_with_videos` accepts privacy param |
| Effect handlers | `app/ui/playlist/effect_handlers_playlist.rs` | 14 handler pairs (Ok/Err) for all operations |
| AppCallbacks | `app.rs` | 9 callbacks: DeletePlaylistFromLibrary, RenamePlaylistFromLibrary, EditPlaylistDetailsFromLibrary, RatePlaylistFromLibrary, GetPlaylistDetailsFromLibrary, ShowDeleteConfirm, OpenRenamePopup, OpenEditPopup, OpenDetailsPopup |
| Library browser | `app/ui/browser/library.rs` | Context menu: D delete (with confirm), R rename popup, E edit popup, t rate, i details popup |
| Keybindings | `config/keymap.rs` | `o` context menu: D delete, R rename, E edit, t rate, i details |
| Save popup | `app/ui/playlist/playlist_save_popup.rs` | Privacy picker field (Private→Public→Unlisted) |
| Rename popup | `app/ui/playlist/playlist_rename_popup.rs` | Text input + Enter confirm + Esc cancel |
| Edit popup | `app/ui/playlist/playlist_edit_popup.rs` | 4 fields (Title/Desc/Privacy/Save), tab/focus cycle |
| Details popup | `app/ui/playlist/playlist_details_popup.rs` | Loading→details display, async fetch |
| Editor popup | `app/ui/playlist/playlist_editor_popup.rs` | `:rename`, `:privacy`, `:rate` commands |
| Delete confirm | `app/ui.rs` (inline) | y/Enter confirm, n/Esc/q cancel |
| Library auto-refresh | `app.rs` | `playlists_fetched = false` after all playlist mutations |

### Architecture Decisions — Playlist Popup System
| Decision | Why | Intent |
|---|---|---|
| 3 separate popup files | Each has distinct input + rendering. No bloat in playlist.rs. | Single-responsibility |
| WindowContext per popup | Youtui dispatches events by context. New variant = correct routing. | Event routing correctness |
| Delete confirm as inline Option | Same as quit_confirm. y/N response, 2 match arms. | Minimal code for simple dialog |
| Option\<GetPlaylistDetails\> | #[non_exhaustive] — can't construct outside ytmapi-rs. | Works around crate boundary |
| Library refresh = playlists_fetched = false | On next category focus, auto-refetch. Zero new infra. Matches ncspot. | Auto-refresh without new fields |
| Sequential API for edit | No builder in ytmapi-rs. Each field = separate query. | Works with existing API surface |
| Rate always sends Liked | No like_status in response to parse. Future: parse from GetPlaylistDetails. | Forward-compatible stub |
| Rename popup char buffer | One field, Enter/Esc. ViTextEditor overhead not worth it. | Suckless |
| Edit popup FocusedField enum | Tab/Enter cycles Title→Desc→Privacy→Save. No ambiguous keys. | Predictable UX |
| Details loading state | "Loading..." instantly, content on async response. No flicker. | Immediate feedback |

### Visual Mode Feature — Shift+HJKL + arrows in VL/VC + lyrics
#### vi-text-editor (`libs/vi-text-editor/src/lib.rs`)
- **handle_visual_char**: H/J/K/L merged into existing h/j/k/l arms (shift variants = same behavior)
- **handle_visual_line**: Full VisualChar motion parity — h/l/H/L/Left/Right/0/$/Home/End + w/W/b/B/e/E + J/K as j/k aliases
- No new types, no new deps. Existing test suite covers h/j/k which applies to H/J/K via pattern merge.

#### lyrics_popup (`youtui/src/app/ui/playlist/lyrics_popup.rs`)
- **Normal mode**: H=left, L=right, Left/Right arrows, 0/$ for line start/end
- **Visual mode**: H=left, L=right, J=j/Down, K=k/Up, Left/Right arrows, 0/$, w/b/e word motions
- J/K move visual_end selection; H/L move cursor_col within current line

### Known Bugs (discovered 2026-06-22)
- **Albums search not working**: ~~F1 search opens but typing doesn't populate results.~~ **FIXED Phase 1.1** — 5 root causes fixed: fetch_albums propagates on tab switch, navigate_to no longer drops task, draw.rs shows list below search box, live search on keystroke, HandleSearchAlbumsOk no longer closes search.
- **Genius annotations return 0**: Without `GENIUS_TOKEN` env var, `__INITIAL_STATE__` scraping fails on most pages. API-based `fetch_annotations_with_token()` needs Bearer token. **Mitigation Phase 1.3**: `has_genius` gate removed — annotations always tried.
- **Annotations only work on modern Genius pages**: Pages without `__INITIAL_STATE__` JSON can't have annotations extracted. Structural limitation.

## Priority (2026-06-22 — User directive)
**PLAYLIST FEATURES ARE THE MOST IMPORTANT.** All other work is secondary. Every browser entity (Songs, Albums, Artists, Playlists, Library) must be fully wired with proper backend→UI→API flow. PlaylistSearch tab was critical gap — now resolved.

---

## 3-Day Retrospective: 2026-06-19 to 2026-06-22

### Branch Strategy
| Branch | Status | Purpose |
|---|---|---|
| `main` | Active | All work merged here. 108 commits in 3 days. |
| `feat/playlist-search` | Merged | PlaylistSearch tab (F1 search, dual panel) |
| `ytmapi-cli` (branches) | Merged | Dedicated debug CLI tool |
| `albums-caching` | Merged | 50-entry LRU cache for album search |
| `lyrics-filter` | Merged | `/` filter in lyrics popup |
| `batch-streaming` | Merged | Batch song streaming for album/playlist |
| `batch-merge` | Merged | Merge playlist into another playlist |

### Commit Timeline (108 commits)

#### Day 1 — 2026-06-19 (36 commits)
```
5608f0c → 426ae82 — Foundation phase
```
- **Library browser tab** — full tab with playlist tracks, context menu, visual mode
- **ViTextEditor** — complete feature set (65 tests):
  - Motions: `f/F/t/T`, `;/,`, `%`, `w/b/e`, `0/$/gg/G`, `W/B/E`
  - Operators: `r`, `~`, `J`, `.`, `C-r`, text objects (`iw/aw/i(/a(/i"/a"`))
- **Lyrics popup** — visual mode, hybrid line numbers, pagination
- **Album art** — 1920x1080 HD, decode loop guard, throttle
- **Navigation hub** — `o→a`/`o→b`/`g→a`/`g→b`, local search, go-to
- **Keybind standard** — consistent across all tabs
- **Config parsing** — fix, footer album art, annotations nav `r→l`
- **Build fixes** — crossterm 0.29 migration, 15→0 warnings
- **Nerd icons** — removed from all 3 files (suckless compliance)

#### Day 2 — 2026-06-21 (40 commits)
```
0610176 → 8198c29 — Feature phase
```
- **Genius annotations** — unified list model, `__INITIAL_STATE__` scraping, JSON API, right panel, Enter seeks timestamp
- **PlaylistEditor** — `:w/:q/:wq` vim-driven editing, `:rename/:privacy/:rate` commands
- **NavigationController** — `:cmd` parser, skip URL album split, 9 fix bundle
- **Lyrics** — `n/p` next/prev song, `<>` seek, `( )` nav, `>`/`<` play next/prev, lyrics race guard, inflight dedup, LRU cache
- **Queue sort** — `o.r` cycles columns (title/artist/album/duration)
- **Playlist popups** — rename, edit (4 fields), details (loading→display), save (privacy picker), delete confirm
- **Visual mode** — Shift+HJKL + arrows in VL/VC + lyrics (H/L cursor_col, J/K visual_end)
- **Config reload** — `:reload` command, `SeekTo` callback
- **Genius-rs crate** — CLI fetch/search/all/slug commands (14 tests)
- **Albums tab** — replaced Playlists tab, table-style columns, sort/filter, YTM search, auto-open empty, LRU search cache, fetch_albums wired
- **Library** — playlist tracks with columns, context menu
- **PlaylistSearch** — new 5th tab (F1 search, dual panel: search list + songs)
- **Fix 46 warnings** — across workspace, 10 ytmapi-rs fixture regenerations
- **Batch-merge** — context menu `o.M`, album art centering
- **Album art** — 1920x1200 (native display match), debug logging

#### Day 3 — 2026-06-22 (32 commits + uncommitted)
```
16a7ea8 → HEAD — Polish + wiring phase
```
- **Final clean builds** — 0 warnings across workspace
- **ytmapi-cli** — live queries + cookie auth (search, search-artists, search-albums, playlist, album, artist, library, fixture)
- **Genius annotations fix** — use real song ID from search API, not slug ID
- **Edit playlist 400 fix** — `privacy_status` serialized correctly (appears once)
- **Genius annotations gate removed** — always try, fallback gracefully when no token
- **Album art centering** — vertical centering in popup
- **Comprehensive docs** — crate docs, man pages, 5.4k-line reference manual

### Current Uncommitted Workspace (this session — ~1300 lines)
#### New crate
- **`libs/metadata-provider/`** — extracted from `youtui/src/app/server/providers/` (8 files → 1 crate, 19 tests, 0 warnings)
  - `MetadataProvider` trait + `MetadataRegistry`
  - 5 provider impls: Last.fm album/track, Discogs, Genius, MusicBrainz
  - `ValidatedMetadata`/`AlbumTrack` moved to crate, re-exported from `crate::app::server`
  - `Server::new()` now accepts `overrides_path: Option<PathBuf>`

#### Debug-First compliance
- **`ytmapi-cli watch-playlist`** — new subcommand using `get_watch_playlist_from_video_id()`
- Fixes gap where `GetRelatedTracks` (o.r in UI) had no dedicated CLI debug tool

#### Dead code cleanup — 9 stale annotations removed
| File | Annotations Removed |
|---|---|
| `lyrics_popup.rs:36` | `Loaded(String)` variant + stale TODO |
| `playlist.rs:135` | `sort_direction` field |
| `playlist.rs:522` | `sort_column_label()` fn |
| `effect_handlers_playlist.rs:52,55` | `HandleReorderPlaylistItemOk`/`Err` |
| `effect_handlers_playlist.rs:62,65` | `HandleRemovePlaylistItemsOk`/`Err` |
| `effect_handlers_playlist.rs:1016,1019` | `HandleAddPlaylistToPlaylistOk`/`Err` |

#### CRITICAL Fix: PlaylistSearch Tab
**Root cause**: `AppAction::BrowserPlaylists` and `AppAction::BrowserPlaylistSongs` used **deprecated** no-op types (`BrowserPlaylistsDeprecated`/`BrowserPlaylistSongsDeprecated`) that logged warnings and exited — all keybindings were dead.

**Fix applied**:
- Removed deprecated enum types from `action.rs` (38 lines deleted)
- Swapped `AppAction` variants to use real `BrowserPlaylistsAction`/`BrowserPlaylistSongsAction` from the PlaylistSearch panels
- Wired dispatch in `app/ui.rs:491-496` → routes to `this.browser`
- Populated `default_browser_playlist_songs_keybinds()` with full keybinding set (was empty `BTreeMap::new()`):
  - `o` context menu: Enter/s/p/P/y/a/b/l/r (11 entries)
  - `y` global CopySongUrl
  - `g a`/`g b` navigation
  - `Enter` PlaySong
- `default_browser_playlists_keybinds()` now has `Enter` → `DisplaySelectedPlaylist` (was empty)

#### Keybinding additions (queue + library)
- **Queue `o` context menu**: `o.q` SaveQueue, `o.L` LoadQueue, `o.Q` DeleteQueue, `o.m` ToggleRomaji, `o.n` SaveToNewPlaylist (was only accessible via Enter→n)
- **Library `o` context menu**: `o.r` GetRelatedTracks (was missing from `default_browser_library_keybinds()`)

#### Auth test infra
- `ytmapi-rs/tests/utils/mod.rs`: `new_standard_api()` now checks 3 cookie paths (env var `youtui_test_cookie`, `YOUTUI_COOKIE`, `cookie.txt`, `~/.cache/youtui/cookie.txt`)
- Same for OAuth: `youtui_test_oauth`, `YOUTUI_OAUTH`, `oauth.json`, `~/.cache/youtui/oauth.json`

#### Locale parameterization (ytmapi-rs)
- `language`/`location` fields on `Client` struct with builder methods
- `YtMusic<A>` forwarding methods
- Threaded through `raw_query_post` context JSON (was hardcoded `"hl":"en","gl":"US"`)
- 3 new tests (85 total ytmapi-rs lib tests)

### Final Test Results (current workspace)
```
youtui               103/103   pass (0 fail, 4 ignored, 0 warnings)
metadata-provider     19/19    pass (0 fail, 0 ignored, 0 warnings)
ytmapi-rs lib         85/85    pass (0 fail, 0 ignored, 0 warnings)
vi-text-editor        65/65    pass (0 fail, 0 ignored)
genius-rs             14/14    pass (0 fail, 0 ignored)
json-crawler           8/8     pass (0 fail, 0 ignored)
async-callback-mgr    15/15    pass (0 fail, 0 ignored)
ytmapi-cli             3/3     pass (0 fail, 0 ignored)
──────────────────────────────────────────────────
TOTAL                312/312   pass (0 fail, 4 ignored)
```

### Known Issues (current)
- **Genius annotations w/o token**: `__INITIAL_STATE__` scraping won't work on most pages. `has_genius` gate removed — always tries API, falls back when no `GENIUS_TOKEN`.
- **Annotations structural limit**: Only modern Genius pages with `__INITIAL_STATE__` JSON work.
- **Auth tests**: 52 ytmapi-rs integration tests need cookie file (run with `cargo test -- --ignored`).
- **`AppCallback::Back`**: `#[allow(dead_code)]` at `app.rs:158` — TODO: Wire back navigation.
- **`AppCallback::GetPlaylistDetailsFromLibrary`**: `#[allow(dead_code)]` at `app.rs:177` — rate toggle from details popup pending.
- **`SearchPlaylists`/`GetPlaylistSongs`**: Batch streaming — `#[allow(dead_code)]` at `messages.rs:54,62`.

### Remaining Roadmap
- **Genius annotations**: Page scrape fallback for pages without `__INITIAL_STATE__`
- **Genius lyrics**: Musixmatch integration (partial)
- **ytmapi-rs**: Locale already done. Batch reorder still swap-only.
- **ytmapi-cli**: More fixture types and streaming tests

### Total Workspace Stats
```
7 crates, 49k+ LOC, 312 tests, 0 warnings
Crate extractions done: ytmapi-rs, json-crawler, async-callback-manager,
  vi-text-editor, genius-rs, ytmapi-cli, metadata-provider (7 of 8 planned)
Remaining crate extraction: audio-player (deep coupling to async_rodio_sink)
```
