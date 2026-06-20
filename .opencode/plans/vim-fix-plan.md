# Vim Fix Plan — Final

## P1: `d G` Not Working

### Analysis
The `d` mode at `keymap.rs:995-1022` has:
- `d` `d` = DeleteSelected ✅
- `d` `G` = DeleteToBottom (direct key in mode) ✅
- `d` `g` = Nested mode "Delete To" wait for `g` or `G`
- `d` `g` `g` = DeleteToTop ✅ (should work as nested)

`d G` should dispatch immediately because `G` is a direct `Key`, not a `Mode`. If it's not working, the likely culprit is your **`config.toml`** overriding the default keybinds. The config is **merged**, not replaced — old saved keybinds shadow new defaults.

### Fix
```bash
# Clean ALL old keybinds from config to let defaults take over
sed -i '/^\[keybinds\]/,/^\[/ s/^\([0-9].*\)$/# \1/' ~/.config/youtui/config.toml
```
Or just delete the config:
```bash
rm ~/.config/youtui/config.toml
```

## P2: `d g` vs `d g g`

### Problem
Before: `d g` (2 keys) = DeleteToTop immediately.
After my change: `d g` enters sub-mode, `d g g` (3 keys) = DeleteToTop.

The user expects `d g g` like vim's `dgg`.

### Fix: Revert `g` to DIRECT key AND keep nested

Change the `d` mode to:
```
'd' → Mode:
  'd' → DeleteSelected      (dd)
  'g' → DeleteToTop          (dg) — direct, immediate
  'G' → DeleteToBottom       (dG) — direct, immediate
  'g' → Mode "Delete To":    (dg → sub-mode for vim compat)
    'g' → DeleteToTop        (dgg — also works)
    'G' → DeleteToBottom     (dgG — also works)
```

But BTreeMap only allows ONE entry per key. `'g'` can be EITHER a direct Key OR a Mode, not both.

**Solution**: Keep `'g'` as the direct key (DeleteToTop), and add a SEPARATE key for the nested mode:
```
'd' → Mode:
  'd' → DeleteSelected
  'g' → DeleteToTop          (dg = immediate)
  'G' → DeleteToBottom       (dG = immediate)
  'z' → Mode "Delete To":   (dz = sub-mode)
    'g' → DeleteToTop        (dzg — same result)
    'G' → DeleteToBottom     (dzG — same result)
```

This way `d g` still works like before (immediate), AND `d z g` also works as a nested alternative. Vim-style `dgg` is approximated by `d g` (which is what it was before my change).

## P3: Visual Range Not Visible

### Problem
`visual_range_style` is `Style::default().bold().italic()` — nearly invisible on most terminals. The user can't see the selection range.

### Fix
Change to a visible background color at `view/draw.rs:199`:
```rust
.visual_range_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR).add_modifier(Modifier::DIM))
```

This uses the same background color as the regular row highlight but with DIM modifier, making the range clearly visible without confusing it with the selected row.

## P4: Esc Doesn't Close Browser Search

### Analysis
`vi_text_editor.rs:188` — `handle_insert()` returns `false` for ALL keys, including Esc. The `SearchBlock::handle_text_event_impl` checks `changed` (which is `false`), so it returns `Some(no_op)` instead of `None`. The Esc key never reaches `BrowserSearchAction::Close`.

### Fix
At `vi_text_editor.rs:188`, after `self.cursor -= 1;`, add:
```rust
return true;
```
This signals that the key was "consumed" (mode changed to Normal), allowing the second Esc dispatch to close the search.

## P5: `5` in Logs Should Be `F5`

### Fix
`keymap.rs:1629` — one line change:
```rust
// BEFORE:
Keybind::new_unmodified(crossterm::event::KeyCode::Char('5')),
// AFTER:
Keybind::new_unmodified(crossterm::event::KeyCode::F(5)),
```

## P6: `E` Popup Still 400

### Investigation needed
The log shows adding 1 video to a VL-prefixed playlist fails with 400. Other parts of the codebase strip `VL` prefix when using playlist IDs (e.g., `playlist_id.get_raw().strip_prefix("VL")`). The `AddPlaylistItemsQuery` might expect the non-VL-prefixed ID.

Check how `CreatePlaylistWithVideos` handles IDs vs how `AddPlaylistItems` handles them. If `CreatePlaylist` strips `VL` and `AddPlaylistItems` doesn't, that's the bug.

## Execution Order

| # | Item | File | Lines | Est. |
|---|------|------|-------|------|
| 1 | Revert `d g` to direct key + add `d z` sub-mode | `keymap.rs:1007-1022` | 5 | 2min |
| 2 | Visual range visible style | `view/draw.rs:199` | 1 | 1min |
| 3 | Esc close search fix | `vi_text_editor.rs:188` | 1 | 1min |
| 4 | Logs `5` → `F5` | `keymap.rs:1629` | 1 | 1min |
| 5 | Investigate `E` 400 (VL prefix) | `api.rs` | 5 | 15min |
| 6 | Build + test + commit | — | — | 5min |
| 7 | User cleans config | `~/.config/youtui/config.toml` | — | 1min |
| | **Total** | | | **~25min** |
