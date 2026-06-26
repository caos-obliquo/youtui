# Suckless Refactoring — Complete ✅

Branch: `refactor/suckless`
Goal: Clean, minimal, robust codebase aligned with suckless philosophy
Baseline: 34.5k LOC, 69 files, 62 deps (youtui crate), 11 workspace members
Results: **-630 lines**, 0 warnings, 136/136 tests pass

## Done

| Batch | Item | Δ Lines | Commit |
|---|---|---|---|
| 1 | Fix 6 panic paths (expect/unwrap → proper error) | -0 | `48c7eaa` |
| 2 | Delete dead crates (metal-proxy, rym-definitions) | -606 | `19f4e46` |
| 3 | Extract boilerplate (7 CRUD macro pairs, conversion fn, thumbnail fn) | -24 | `7fc6252` |
| 4a | Subdivide MetadataEffect::apply (180→40 lines) | -0 | `35bf646` |
| 4b | Extract clean_title_for_metadata into 4 named helpers | -0 | `35bf646` |
| 4d | Extract handle_force_split from apply_action (75→1 line arm) | -0 | `096fa0f` |
| **Total** | | **-630** | |

## Not Done (low value)

| Skipped | Reason |
|---|---|
| Batch 4c: handle_callback split (460 lines) | Most arms are 1-3 lines, splitting adds indirection |
| Batch 4e: api.rs retry dedup | Complexity too high for 15-line savings |
| Batch 4f: keymap.rs dead bindings | No automated dead binding detection |
| Batch 5: error swallows | Sixel writes are intentional no-ops (terminal disappear) |

## Batch Details

### Batch 1: Fix Panic Paths
Replace `.expect()` and `.unwrap()` that can panic at runtime:
- `api.rs:168,226`: `refresh_token()?.expect(...)` → `ok_or_else` + propagate
- `playlist.rs:844`: `get_song_from_idx().expect("BUG")` → `if let Some` + last_error
- `shared_components.rs:268`: `suggestions_cur.expect(...)` → proper state handling
- `keybind.rs:33`: `partial_cmp.expect(...)` → fall back to `Ordering::Equal`
- `structures.rs:177`: `chars.next().unwrap()` → `unwrap_or("?")`
- `core.rs:218`: `FromStr::from_str(value).unwrap()` → propagate error

### Batch 2: Kill Dead Crates
- `libs/metal-proxy/` (Cargo.toml + src/) — -317 lines
- `libs/rym-definitions/` (Cargo.toml + src/) — -289 lines

### Batch 3: Extract Boilerplate
- 7 CRUD OK handlers + 8 error handlers → shared macro
- PlaylistSong→ListSong conversion → `convert_playlist_songs()` helper
- Thumbnail collection → `collect_thumbnail_tasks()` helper (used by push_song_list + insert_next_song_list)
- Skipped: api.rs retry dedup (complexity too high for ~15-line savings)

### Batch 4a: Subdivide MetadataEffect::apply (180→40 lines)
- Extract `apply_metadata_fields()` — applies artist/album/year/genres/styles to song
- Extract `handle_album_split()` — duration ratio check, tracklist validation, insert tracks, fetch album art
- Main fn drops from 183 to ~40 lines of routing

### Batch 4b: Split clean_title_for_metadata (130→10 lines)
- Extract 4 named helpers: `strip_artist_prefix`, `strip_youtube_noise`, `strip_album_metadata_tags`, `strip_year_from_title`
- Main fn now chains 4 calls in <10 lines
- Each pass independently testable

### Batch 4d: Extract handle_force_split from apply_action (75→1 line)
- Move inline ForceSplitAlbum arm (~75 lines) into dedicated `Playlist::handle_force_split()` method
- apply_action drops 74 lines of inline logic

## Verification
- `cargo build --release` — 0 warnings across workspace (all 11 crates)
- `cargo test --release -p youtui --bin youtui` — 136 pass, 4 ignored
- Suckless refactoring adds 0 tests (refactors existing code only)
