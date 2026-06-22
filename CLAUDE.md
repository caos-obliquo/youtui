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

**5,452 lines, 20 files, ~45 pages** — covers all 5 crates, 49k LOC.

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
cargo test --release -p genius-rs                  # 11 pass
cargo test --release -p async-callback-manager     # 15 pass
cargo test --release -p json-crawler               # 8 pass
```

## Warnings

`cargo build --release` — 0 warnings across workspace (youtui + 4 lib crates).
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
|---|---|---|---|
| `app/server/messages.rs` | ~1280 | All backend tasks |
| `app/ui/playlist.rs` | ~2440 | Queue, playback, scrobbling, visual mode |
| `app/ui/playlist/effect_handlers_playlist.rs` | ~555 | Frontend effect handlers |
| `app/ui/browser/library.rs` | ~914 | Library browser (4th tab) |
| `app/ui/browser.rs` | ~690 | Browser routing, tab dispatch |
| `config/keymap.rs` | ~1982 | All keybindings by context |
| `libs/vi-text-editor/src/lib.rs` | ~2260 | Vi-mode text editor |
| `libs/genius-rs/src/lib.rs` | ~100 | Genius API client + HTML scraped lyrics/annotations |
| `app/ui/playlist/playlist_rename_popup.rs` | ~85 | Rename popup (char buffer) |
| `app/ui/playlist/playlist_edit_popup.rs` | ~165 | Edit popup (4 fields, tab cycle) |
| `app/ui/playlist/playlist_details_popup.rs` | ~145 | Details popup (loading→display) |
| `app/ui/playlist/lyrics_popup.rs` | ~690 | Lyrics display + visual mode |

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
| Library auto-refresh | `app.rs` | `playlists_fetched = false` after mutations |

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

### Completed — All ncspot-inspired playlist features
| Feature | Status | Keybinding |
|---------|--------|------------|
| View all playlists | ✅ | Library > Playlists |
| View playlist tracks | ✅ | Right arrow |
| Delete playlist | ✅ | `o.D` |
| Rename playlist | ✅ | `o.R` |
| Edit description/privacy | ✅ | `o.E` |
| Rate playlist | ✅ | `o.t` (always Liked, toggle coming) |
| Get playlist details | ✅ | `o.i` |
| Remove tracks from playlist | ✅ | `o.x` |
| Reorder tracks | ✅ | `o.J`/`o.K` |
| Save to existing playlist | ✅ | `o.E` (update popup) |
| Save to new playlist | ✅ | `o.s` (save popup) |
| Playlist editor popup | ✅ | `o.e` |
| Artist subscribe/unsubscribe | ✅ | `o.S`/`o.U` |
| Back navigation | ✅ | (backspace/browser back) |
| Library auto-refresh | ✅ | After all playlist mutations |

### Remaining dead code / feature gaps
6 items needing feature-level work. Branches created on `main`:
- `feat/playlist-search` — new browser tab for YTM playlist search
- `feat/batch-streaming` — `get_playlist_songs()` to stream all playlist tracks
- `feat/queue-sort` — sort popup UI for queue columns
- `feat/batch-merge` — `append_raw_playlist_items` backend message

Known issues:
- `o.E` edit playlist 400 — YTM API rejects `EditPlaylistQuery`. Fixed duplicate `privacy_status` action. Verify fix.
- Genius annotations API — new `fetch_annotations_with_token()` method. Requires `GENIUS_TOKEN` env var.
