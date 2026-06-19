# Visual Highlight Fix — Range Highlighting for Visual Mode

## Problem

`secondary_highlight_row: Option<usize>` only highlights ONE row. Visual mode needs ALL rows from `visual_start` to `cur_selected` highlighted (vim-style full range).

## Fix

### 1. Change widget to support range (`scrolling_table.rs`)

Replace `secondary_highlight_row: Option<usize>` with:

```rust
/// Visual selection range: (start, end) inclusive. Both ends get secondary style.
visual_range: Option<(usize, usize)>,
```

Remove `secondary_highlight_row` field. Remove `secondary_row_highlight_style` field.
Add `visual_range_style: Style`.

**Rendering logic change** (lines ~225-235): instead of checking `windowed_secondary`, check if each row index falls within `visual_range`:

```rust
// For each row being rendered:
let in_visual_range = visual_range.map_or(false, |(start, end)| {
    i >= start && i <= end
});
if in_visual_range {
    // apply visual_range_style
} else if i == windowed_selected {
    // apply row_highlight_style
}
```

### 2. Update `draw_table_impl` to pass visual range (`view/draw.rs`)

Change the `secondary_highlighted_row: Option<usize>` parameter to:

```rust
visual_range: Option<(usize, usize)>,
```

Pass it to `ScrollingTable::new().visual_range(visual_range)`.

### 3. Wire visual range from playlist (`playlist.rs:TableView`)

In the playlist's `TableView` impl, when `visual_mode` is true, return the range `(min, max)` as the visual range:

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

A new method on the `TableView` trait or pass it differently.

### 4. Files changed

| File | Change |
|------|--------|
| `widgets/scrolling_table.rs` | `secondary_highlight_row` → `visual_range: Option<(usize, usize)>` |
| `view/draw.rs` | Update `draw_table_impl` signature + call site |
| `view.rs` | Add `get_visual_range()` to `TableView` trait or similar |
| `playlist.rs` | Implement visual range logic |
| Browser song panels (3 files) | Update `get_visual_range()` to return None |

### Lines: ~40 total across 5 files
