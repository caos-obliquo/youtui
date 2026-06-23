# Testing

## Test Suites

| Suite | Command | Count | Notes |
|-------|---------|-------|-------|
| Main app | `cargo test --release -p youtui --bin youtui` | 103 pass + 4 ignore | Unit + integration |
| ViTextEditor | `cargo test --release -p vi-text-editor` | 65 | Unit + proptests |
| ytmapi-rs (no auth) | `cargo test --release -p ytmapi-rs --lib` | 85 | All pass offline |
| ytmapi-rs (full) | `cargo test --release -p ytmapi-rs` | 28 pass / 52 fail | Needs browser auth |
| genius-rs | `cargo test --release -p genius-rs` | 14 | Unit tests for scraping + search + annotations |
| metadata-provider | `cargo test --release -p metadata-provider` | 19 | Unit: discogs, genius, lastfm, musicbrainz parsers |
| ytmapi-cli | `cargo test --release -p ytmapi-cli` | 7 | Fixture parsing, CLI usage |
| async-callback-manager | `cargo test --release -p async-callback-manager` | 14 | 3 unit + 11 integration |
| json-crawler | `cargo test --release -p json-crawler` | 8 | Unit + 2 doctests |

## Running Tests

```bash
# All workspace tests
cargo test --release --workspace

# Single crate
cargo test --release -p vi-text-editor

# Single test by name
cargo test --release -p vi-text-editor -- test_delete_char

# Proptest invariants (randomized)
cargo test --release -p vi-text-editor -- invariants

# Run with environment variable to control proptest iterations
QUICKCHECK_TESTS=1000 cargo test --release -p vi-text-editor -- invariants
```

## Ignored Tests

4 tests in youtui are ignored by default:

| Test | Reason |
|------|--------|
| `test_downloads` | Costly — requires network + yt-dlp |
| `test_downloading_a_song_with_ytdlp` | Network + yt-dlp required |
| `test_semaphore_limiting` | Flaky — dynamic concurrency |
| `test_default_config_equals_deserialized_config` | Config drifts from defaults |

Run ignored tests explicitly: `cargo test --release -- --ignored`

## ViTextEditor Proptests

3 property-based tests in `libs/vi-text-editor/src/lib.rs`:

```rust
cursor_never_exceeds_buffer  // Random text → operations → cursor <= len
undo_redo_roundtrip           // x → undo → redo → undo → original state
paste_does_not_corrupt        // Paste at any position → invariants pass
```

Uses `proptest` crate with `QuickCheck`-style randomized input generation.

## Test Structure

```
youtui/src/
├── tests.rs                      — Integration tests
├── app/
│   ├── server/providers/
│   │   ├── discogs.rs (inline)   — Parse tests
│   │   ├── genius.rs (inline)    — Metadata parse tests
│   │   ├── lastfm_album.rs (inline)
│   │   ├── lastfm_track.rs (inline)
│   │   ├── musicbrainz.rs (inline)
│   │   └── util.rs (inline)      — norm_for_lfm tests
│   ├── ui/
│   │   ├── browser/
│   │   │   ├── artistsearch/ (inline) — Search/submit behavior
│   │   │   ├── playlistsearch/ (inline)
│   │   │   └── shared_components.rs (inline) — Search suggestions, list columns
│   │   ├── playlist/
│   │   │   ├── tests.rs          — Album splitting, ARC sharing, progress
│   │   │   └── browser.rs (inline) — Keybinding validation
│   │   └── action.rs (via actionhandler.rs) — Key stack resolution
│   ├── view/ (inline)            — Filter constraint tests
│   ├── queue_persistence/ (inline) — Serialization roundtrip
│   ├── component/actionhandler.rs (inline) — Key stack parsing
│   └── structures/ (inline)      — Fuzzy matching tests
├── config/
│   ├── keymap.rs (inline)        — Keybinding parsing
│   └── mod.rs (inline)           — Config IR roundtrip, deserialization
├── core.rs (inline)              — File management, temp cleanup
├── drawutils.rs (inline)         — Rect boundary checks
├── keybind.rs (inline)           — Key parsing
├── widgets/
│   ├── scrolling_list.rs (inline) — Scrolling behavior
│   ├── scrolling_table.rs (inline) — Scrolling + grapheme handling
│   ├── tab_grid.rs (inline)      — Grid layout
│   └── mod.rs (inline)           — Split point tests
└── youtube_downloader/ (inline)  — yt-dlp argument generation
```

## Testing Latest Updates (2026-06-23)

### VL Prefix Regression Test

Mutation endpoints must strip `VL` from playlist IDs. Browse endpoints must keep `VL`.

Manual test via ytmapi-cli:
```bash
# Delete (strips VL)
cargo run --release -p ytmapi-cli -- delete-playlist VLPL...

# Edit/rename (strips VL)
cargo run --release -p ytmapi-cli -- edit-playlist VLPL... --title "new"

# Rate (strips VL)
cargo run --release -p ytmapi-cli -- rate-playlist VLPL... like

# Read/browse (needs VL)
cargo run --release -p ytmapi-cli -- playlist-songs VLPL...

# Add to playlist (strips VL)
cargo run --release -p ytmapi-cli -- add-to-playlist VLPL... <videoId>
```

Expected: mutations return 200 (not 400/404), reads return playlist data.

### Library Auto-Refresh Test

Every playlist mutation (delete, rename, edit, rate) must trigger library playlist count refresh:
1. Open Library → Playlists tab
2. Perform mutation via `o` menu (`o.D` delete, `o.R` rename, `o.t` rate, `o.E` edit)
3. Verify playlist count updates within 2-3 seconds
4. Verify mutated playlist reflects changes (new name, removed from list on delete)

### Playlist Editor Test

1. Open Library → Playlists
2. Press `Enter` on a playlist → tracks load (first Enter fetches)
3. Press `Enter` again → editor popup opens with tracks
4. Test `j/k` navigation, `dd` delete (with `d` confirm), `J`/`K` reorder
5. Test `:w` save, `:wq` save+quit, `:q` quit
6. Test `:rename`, `:privacy`, `:rate` commands

### Annotations/Lyrics Component Isolation Test

1. Open lyrics popup (`o.l`)
2. Press `a` to toggle annotations on
3. Verify view splits 55/45 lyrics/annotations
4. Press `Tab`/`l` to focus annotations panel
5. Verify lyrics seek commands (`(`/`)`/`<`/`>`) do nothing while annotations focused
6. Press `Tab`/`h` to return focus to lyrics
7. Verify seek commands work again
8. Press `a` to toggle annotations off — focus returns to lyrics full width
