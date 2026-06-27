# Testing

## Test Suites

| Suite | Command | Count | Notes |
|-------|---------|-------|-------|
| `youtui` | Binary | `cargo test --release -p youtui --bin youtui` | 164 | 146 unit + 18 integ |
| Main app | `cargo test --release -p youtui --bin youtui` | 164 pass + 4 ignore | Unit + integration |
| ViTextEditor | `cargo test --release -p vi-text-editor` | 67 | Unit + proptests |
| ytmapi-rs (no auth) | `cargo test --release -p ytmapi-rs --lib` | 82 | All pass offline (3 locale tests removed in slimming) |
| ytmapi-rs (full) | `cargo test --release -p ytmapi-rs` | 28 pass / 52 fail | Needs browser auth |
| genius-rs | `cargo test --release -p genius-rs` | 18 | Unit tests for scraping + search + annotations |
| metadata-provider | `cargo test --release -p metadata-provider` | 48 | Unit: providers, genre_map, scoring, cache |
| async-callback-manager | `cargo test --release -p async-callback-manager` | 14 | 3 unit + 11 integration |
| json-crawler | `cargo test --release -p json-crawler` | 2 | Unit + 2 doctests |
| lrclib-rs | `cargo test --release -p lrclib-rs` | 4 | LRCLIB API lyrics provider |
| rym-genre-data | `cargo test --release -p rym-genre-data` | 10 | RYM hierarchy parser |
| audio-player | `cargo test --release -p audio-player` | 0 | Sound playback wrapper |

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
| `test_downloads` | Costly - requires network + yt-dlp |
| `test_downloading_a_song_with_ytdlp` | Network + yt-dlp required |
| `test_semaphore_limiting` | Flaky - dynamic concurrency |
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

## PR #3 Test Coverage (2026-06-26)

15 new unit tests for the perf batch (ea2fc1c):

| Area | Tests | File | What it tests |
|------|-------|------|---------------|
| PlayDebouncer | 5 | `app.rs` | allow/deny/cooldown/reset/multiple rapid events |
| Protocol cache | 3 | `app/ui.rs` | invalidate_protocol_cache(), None/Some invalidation |
| Download cancel | 3 | `app/ui/playlist.rs` | cancel_all_downloads() with active/cancelled/mixed tokens |
| Library lazy iterator | 4 | `app/ui/browser/library.rs` | get_filtered_items() returns lazy iterators for all 4 categories + tracks view |

## Test Structure

```
youtui/src/
├── tests.rs                      - Integration tests
├── app/
│   ├── server/providers/
│   │   ├── discogs.rs (inline)   - Parse tests
│   │   ├── genius.rs (inline)    - Metadata parse tests
│   │   ├── lastfm_album.rs (inline)
│   │   ├── lastfm_track.rs (inline)
│   │   ├── musicbrainz.rs (inline)
│   │   └── util.rs (inline)      - norm_for_lfm tests
│   ├── ui/
│   │   ├── browser/
│   │   │   ├── artistsearch/ (inline) - Search/submit behavior
│   │   │   ├── playlistsearch/ (inline)
│   │   │   └── shared_components.rs (inline) - Search suggestions, list columns
│   │   ├── playlist/
│   │   │   ├── tests.rs          - Album splitting, ARC sharing, progress
│   │   │   └── browser.rs (inline) - Keybinding validation
│   │   └── action.rs (via actionhandler.rs) - Key stack resolution
│   ├── view/ (inline)            - Filter constraint tests
│   ├── queue_persistence/ (inline) - Serialization roundtrip
│   ├── component/actionhandler.rs (inline) - Key stack parsing
│   └── structures/ (inline)      - Fuzzy matching tests
├── config/
│   ├── keymap.rs (inline)        - Keybinding parsing
│   └── mod.rs (inline)           - Config IR roundtrip, deserialization
├── core.rs (inline)              - File management, temp cleanup
├── drawutils.rs (inline)         - Rect boundary checks
├── keybind.rs (inline)           - Key parsing
├── widgets/
│   ├── scrolling_list.rs (inline) - Scrolling behavior
│   ├── scrolling_table.rs (inline) - Scrolling + grapheme handling
│   ├── tab_grid.rs (inline)      - Grid layout
│   └── mod.rs (inline)           - Split point tests
└── youtube_downloader/ (inline)  - yt-dlp argument generation
```

## Testing Latest Updates (2026-06-23)

### VL Prefix Regression Test (historical - ytmapi-cli removed from workspace)

Mutation endpoints must strip `VL` from playlist IDs. Browse endpoints must keep `VL`.

Manual test via youtui binary (ytmapi-cli removed in PR #27):
```bash
# Delete (strips VL) - use playlist editor or o.D context menu
# Edit/rename (strips VL) - use o.R context menu
# Rate (strips VL) - use o.t context menu
# Read/browse (needs VL) - Enter on playlist
# Add to playlist (strips VL) - use o.a/o.A context menu
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

## Scrobbler Tests (5 tests, 2026-06-26)

In `youtui/src/app/scrobbler.rs`:

| Test | What it covers |
|------|---------------|
| `scrobble_state_timing` | Duration threshold (>=30s for scrobble) |
| `session_key_used_for_signing` | Session_key from config passed to signer |
| `signature_sorted_alphabetically` | params.sort_by() before HMAC signing |
| `rate_limiting` | Debounce consecutive scrobbles (rate_limit_duration) |
| `error_handling_bad_auth` | Invalid session_key returns ApiError |

## CLI Test Tool

```bash
# Direct scrobble submission (bypasses UI)
youtui test-scrobble --artist "Artist" --title "Song" --album "Album" --duration 180

# Test metadata validation pipeline (bypasses UI)
youtui test-validate-metadata "Artist" "Title" ["Album hint"]
```

- `test-scrobble`: Tests full scrobble pipeline: session_key → HMAC signing → Last.fm API.
- `test-validate-metadata`: Tests MetadataRegistry against all providers (Last.fm, MusicBrainz, Discogs, MetalArchives, Genius). Prints resolved artist/album/year/tracklist.

## Persistent Scrobble Cache

- File: `~/.config/youtui/scrobble_cache.json`
- Failed scrobbles saved with retry_count field
- Retried on next youtui startup via `retry_failed_scrobbles()`
- Max 3 retries per entry (dropped after 3 failures)

## Ignored Tests (existing 4)

### Annotations/Lyrics Panel Focus Test

1. Open lyrics popup (`o.l`)
2. Press `a` to toggle annotations on
3. Verify view splits 55/45 lyrics/annotations
4. Press `Tab`/`l` to focus annotations panel
5. Verify j/k/gg/G scroll annotations, seek commands `( ) < > [ ]` still work (global controls)
6. Press `Tab`/`h` to return focus to lyrics
7. Verify j/k/gg/G scroll lyrics, seek commands still work
8. Press `a` to toggle annotations off - focus returns to lyrics full width
