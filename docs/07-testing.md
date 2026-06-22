# Testing

## Test Suites

| Suite | Command | Count | Notes |
|-------|---------|-------|-------|
| Main app | `cargo test --release -p youtui --bin youtui` | 120 + 4 ignore | Unit + integration |
| ViTextEditor | `cargo test --release -p vi-text-editor` | 65 | Unit + proptests |
| ytmapi-rs (no auth) | `cargo test --release -p ytmapi-rs --lib` | 82 | All pass offline |
| ytmapi-rs (full) | `cargo test --release -p ytmapi-rs` | 28 pass / 52 fail | Needs browser auth |
| async-callback-manager | `cargo test --release -p async-callback-manager` | 15 | Unit |
| json-crawler | `cargo test --release -p json-crawler` | 8 | Unit |

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
