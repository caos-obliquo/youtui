# Final Polish Plan — Visual Range + Search Indicator

## Issue 1: `d gg` / `d G` (Delete to Top/Bottom)

### Current state from code analysis

The `d` mode (`playlist` context, line 982) has:
```
d → Mode "Delete Mode":
  d → DeleteSelected     (= dd)
  g → DeleteToTop        (= d g)
  G → DeleteToBottom     (= d G)
```

The `list` context (line 1744) has:
```
g → Mode "Go To":
  g → First              (= gg)
  G → Last               (= gG)
G → Last                 (standalone)
```

### Analysis

`d g` (one `g`) should dispatch `DeleteToTop`:
1. `d` → key_stack=[d] → `playlist`'s `d` mode found → returns Mode
2. `g` → key_stack=[d,g] → `d` mode's sub-keys searched → `g` → `DeleteToTop`

`d G` (one `G`) should dispatch `DeleteToBottom`:
1. `d` → key_stack=[d] → `d` mode found
2. `G` → key_stack=[d,G] → `d` mode's sub-keys → `G` → `DeleteToBottom`

**However**: The user says "d gg" not "d g". In youtui, `d gg` = `d` dispatches `DeleteToTop`, then `g` enters the playlist's "Go To" mode. So `d gg` ≠ `d g`.

**Probable cause**: User expects vim-style `d gg` (operator `d` + motion `gg` = delete to top), but youtui uses `d g` (mode sub-key). The fix is to support `d gg` by allowing nested mode entry: when `d` mode is active and `g` is pressed, instead of immediately dispatching, enter a sub-mode. But the current architecture doesn't support operator+motion generically.

**Recommendation**: Document that `d g` (not `d gg`) = DeleteToTop. Or add `g` as a second level in the `d` mode: `d` → `g` → `g` = DeleteToTop, `d` → `g` → `G` = DeleteToBottom. But this conflicts with the standalone `g` mode.

Actually simpler: just add to the `d` mode:
```rust
'd' → Mode:
  'd' → DeleteSelected
  'g' → DeleteToTop       // d g = delete to top
  'G' → DeleteToBottom    // d G = delete to bottom
  'g' → Mode:             // d g g = delete to top (vim-compat)
    'g' → DeleteToTop
    'G' → DeleteToBottom
```

This way `d g` AND `d g g` both work (but `d g` dispatches immediately while `d g g` does the same after two `g`s).

### Lines: ~8 in keymap.rs

---

## Issue 2: Visual Mode Range Highlighting

### Current state
- `secondary_highlight_row: Option<usize>` only highlights ONE row
- Visual mode needs ALL rows from visual_start to cur_selected highlighted

### Implementation

**File 1**: `widgets/scrolling_table.rs`

Replace `secondary_highlight_row` with `visual_range`:
```rust
// Change field:
secondary_highlight_row: Option<usize>    →    visual_range: Option<(usize, usize)>

// Change render logic (lines 231-238):
// BEFORE:
let windowed_secondary = secondary_highlight_row.and_then(...);
let row_style = if windowed_secondary == Some(idx) { secondary_row_highlight_style } ...

// AFTER:
let row_style = visual_range.map_or(Default::default(), |(start, end)| {
    let abs_idx = offset + idx;
    if abs_idx >= start && abs_idx <= end { visual_range_style } else { Default::default() }
});
```

Note: offset is already applied (items are `skip(offset).take(visible_rows)`). The `idx` in the enumerate is 0-based within the window. Need to compare against the absolute index: `offset + idx`.

**Lines**: ~10

**File 2**: `view/draw.rs`

Change `draw_table_impl` signature:
```rust
// BEFORE:
secondary_highlighted_row: Option<usize>,
// AFTER:
visual_range: Option<(usize, usize)>,
```

Update the call to `ScrollingTable::new(): 
```rust
// BEFORE:
.secondary_highlight_row(secondary_highlighted_row)
// AFTER:
.visual_range(visual_range)
```

Update all callers of `draw_table_impl` (2 callers at lines 238, 301) to pass `None` for visual_range.

**Lines**: ~10

**File 3**: `playlist.rs`

Add a new method to `TableView` trait or a separate method:
```rust
fn get_visual_range(&self) -> Option<(usize, usize)> {
    if self.visual_mode {
        let start = self.visual_start.min(self.cur_selected);
        let end = self.visual_start.max(self.cur_selected);
        Some((start, end))
    } else {
        None
    }
}
```

But this isn't in the `TableView` trait. The `draw_table` function at `view/draw.rs:244` passes `None` as secondary highlight. I need to either:
- Add `get_visual_range()` to `TableView` trait, or
- Pass it through the existing `draw_table` → `draw_table_impl` chain

Actually, looking at `draw_table` (line 244), it calls `draw_table_impl` with `None`. The callers of `draw_table` are in the individual components (playlist, browser panels). Each component calls `draw_table` directly.

So I can:
1. Change `draw_table_impl` to take `visual_range` instead of `secondary_highlighted_row`
2. Change `draw_table` to take `visual_range` as well
3. Pass it from each component

The chain is: `Component::draw` → `draw_table(table, ...)` → `draw_table_impl(..., visual_range)`.

I can pass the visual range from each component:
```rust
// In playlist's draw:
let visual_range = if self.visual_mode {
    Some((self.visual_start.min(self.cur_selected), self.visual_start.max(self.cur_selected)))
} else {
    None
};
draw_table(f, self, chunk, cur_tick, visual_range)
```

Or add it to the `TableView` trait. Adding to the trait is cleaner:
```rust
trait TableView {
    // ... existing methods ...
    fn get_visual_range(&self) -> Option<(usize, usize)> { None }  // default impl
}
```

Then override in `Playlist`:
```rust
fn get_visual_range(&self) -> Option<(usize, usize)> {
    if self.visual_mode {
        Some((self.visual_start.min(self.cur_selected), self.visual_start.max(self.cur_selected)))
    } else {
        None
    }
}
```

**Lines**: ~15 across 3 files

### Files summary for Issue 2

| File | Lines | Change |
|------|-------|--------|
| `widgets/scrolling_table.rs` | 10 | `secondary_highlight_row`→`visual_range` + render logic |
| `view.rs` | 3 | Add `get_visual_range()` to `TableView` trait |
| `view/draw.rs` | 5 | Update `draw_table_impl` + `draw_table` signatures |
| `playlist.rs` | 5 | Implement `get_visual_range()` |
| 3 browser panels | 3 | Default impl returns None (no change needed if trait has default) |

---

## Issue 3: Search Indicator in Browser Song Panel

### Current state
Browser song search opens a popup in the LEFT panel (artist list). The right panel (songs) shows results without a search indicator like `[SEARCH: query (n/m)]` seen in the playlist.

### Fix
In the song panel title, when search results are shown, display `[SEARCH: query]` or the result count.

For `SongSearchBrowser` — the title is set in `view.rs` or `draw.rs`. The `get_title` method returns the panel title. Add the search query to it.

**Lines**: ~5 in songsearch.rs (or the draw function)

---

## Execution Order

| # | Item | Est. |
|---|------|------|
| 1 | `d` mode: add `g g` as nested mode (vim-compat) | 5min |
| 2 | `scrolling_table.rs` — `secondary_highlight_row`→`visual_range` | 10min |
| 3 | `view.rs` — add `get_visual_range()` to `TableView` | 3min |
| 4 | `view/draw.rs` — update signatures + pass visual range | 5min |
| 5 | `playlist.rs` — implement `get_visual_range()` | 5min |
| 6 | Search indicator in browser song panel | 5min |
| | **Build + test + commit** | 10min |
| | **Total** | **~45min** |
