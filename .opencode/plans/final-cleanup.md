# Final Warning Cleanup + Esc Revert Plan

## Part 1: Revert Esc behavior (double Esc)

Revert the `return true` in `handle_insert` for Esc at `vi_text_editor.rs:188`. This restores the original behavior:
- **First Esc** (Insert mode) → switches to Normal mode
- **Second Esc** (Normal mode) → closes search

**File**: `vi_text_editor.rs:188` — remove `return true;`
**Lines**: 1

## Part 2: Fix 15 warnings (~12 min)

| # | Warning | File:Line | Fix |
|---|---------|-----------|-----|
| 1 | `album_art_fetch_throttle` never read | `playlist.rs:122` | Remove unused field + its init + declare in struct |
| 2 | `filtered_playlists` never used | `playlist_update_popup.rs:155` | Remove entire method (replaced by `refresh_filter`) |
| 3 | `load_selected_playlist`, `search_text`, `is_search_active` never used | `library.rs:485` | Add `#[allow(dead_code)]` on each |
| 4 | `unused import: RateSong` | `effect_handlers_playlist.rs:4` | Remove `RateSong` from the import |
| 5 | `unused import: SearchArtists` | `browser.rs:534` | Remove the `use` line inside `navigate_to` |
| 6 | `unreachable pattern` (2x) | `library.rs:716,740` | Add `#[allow(unreachable_patterns)]` before the match |
| 7 | `unused variable: this` (2x) | `effect_handlers_playlist.rs:77,90` | `\|this, ...\|` → `\|_this, ...\|` |
| 8 | `variable does not need to be mutable` (3x) | `playlist.rs:1943,1962`, `ui.rs:1029` | `let mut` → `let` |
| 9 | `variant Back` never constructed | `app.rs:127` | Add `#[allow(dead_code)]` |
| 10 | `RenamePlaylist`, `RemovePlaylistItems` never constructed | `messages.rs:93,99` | Add `#[allow(dead_code)]` |

**Total**: ~14 changes, ~13 min

## Execution Order

```
1. Revert Esc (1 line, 1 min)
2. Fix all 15 warnings (12 min)
3. Build + test + commit (5 min)
Total: ~18 min
```
