# Vim Polish — Final Plan (Yazi-Inspired)

## Core Insight from Yazi

**Yazi proves: vim-driven TUI = `0`-`9` are ALWAYS count prefixes, NEVER action keys.**

Youtui's biggest gap is that `0`-`6` are bound to actions. Yazi uses `tab_switch 1`/`tab_switch 2` with the tab number as an argument, not a keypress. We follow this pattern.

---

## Phase 0: Free All Digits (keymap.rs)

Move ALL digit-bound actions to alternatives:

| Current | Action | New | Rationale |
|---------|--------|-----|-----------|
| `0` | ViewLogs | `g l` ("go to logs") | Consistent with `g` prefix |
| `1` | ViewBrowser (tab 1) | Already have `5` for browser/playlist toggle. Remove `1`. | Browser accessible via `5` (playlist) or tab switching |
| `2` | BrowserSearch | Already duplicated by `/` — REMOVE | `/` is standard vim search |
| `3` | Filter | REMOVE from global. Keep as `3` only when in a song list (filter context) | Actually, Yazi doesn't have a "filter" keybind number. We keep filter accessible only via its context |
| `4` | Sort | Same as filter — context-specific | |
| `5` | ViewPlaylist | KEEP as `5` (tab 5 = playlist) | Tab number exception — like Yazi's `tab_switch` |
| `6` | ChangeSearchType | REMOVE | Rarely used, accessible via other means |

**Net result**: Digits `0`, `1`, `2`, `3`, `4`, `6` freed for count prefix. Only `5` remains as an action key.

### `g l` for ViewLogs

Add to `default_global_keybinds`:
```rust
(
    Keybind::new_unmodified(crossterm::event::KeyCode::Char('g')),
    KeyActionTree::new_mode(
        [
            (
                Keybind::new_unmodified(crossterm::event::KeyCode::Char('l')),
                KeyActionTree::new_key_with_visibility(
                    AppAction::Log(LoggerAction::ViewBrowser),
                    KeyActionVisibility::Global,
                ),
            ),
        ],
        "Go To".into(),
    ),
),
```

Wait, `g` is already bound in multiple contexts. Need to add `g l` to each `g` mode or create a global fallback.

Actually simpler: just add `F1` for ViewLogs and be done. Yazi doesn't overthink this.

**Simplest approach**: Replace `0`=ViewLogs with `F1`=ViewLogs. Replace `1`=ViewBrowser with nothing (browser/playlist toggle via `5`). Remove `2`=Search (`/` covers it). Remove `6`=ChangeSearchType.

---

## Phase 1: Visual Mode Highlight + Yank (15 min)

### 1a: Fix `get_highlighted_row()` — `playlist.rs:561`
```rust
fn get_highlighted_row(&self) -> Option<usize> {
    if self.visual_mode {
        Some(self.visual_start)
    } else {
        self.get_cur_playing_index()
            .and_then(|idx| self.actual_to_visual_index(idx))
    }
}
```

### 1b: Yank in visual mode — `playlist.rs` CopySongUrl handler
When `visual_mode` is true, `y` copies selected lines as `"artist - title"`.

---

## Phase 2: Merge `g` Mode — `gg`=First, `gG`=Last (10 min)

Replace standalone `g`/`G` keybinds with mode entries in:
- `default_list_keybinds`: add `g g`=First, `g G`=Last
- `default_playlist_keybinds`: already has `g` mode, just add `g`=First, `G`=Last
- `default_browser_library_keybinds`: already has `g` mode, add `g`=First, `G`=Last

---

## Phase 3: Count Prefix for Motions (30 min)

### Architecture (modeled on Yazi's `arrow [steps]`)

**New file**: `app/count_prefix.rs`
```rust
/// Extract count prefix from key stack.
/// Returns (count, remaining_key_stack).
/// If no digits prefix, returns (1, original_keys).
pub fn extract_count(keys: &[KeyEvent]) -> (usize, &[KeyEvent]) {
    let mut count = 0u32;
    let consumed = keys.iter().take_while(|k| {
        if let KeyCode::Char(c) = k.code {
            c.to_digit(10).map(|d| { count = count * 10 + d; true }).unwrap_or(false)
        } else {
            false
        }
    }).count();
    if consumed == 0 { (1, keys) } else { (count as usize, &keys[consumed..]) }
}
```

**Modified**: `app/component/actionhandler.rs` — Add `COUNT_PREFIX` thread_local + apply helper:
```rust
thread_local! {
    pub static COUNT_PREFIX: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
}
```

**Modified**: `app/ui.rs:handle_key_event` — Before dispatching, extract count:
```rust
// In the key processing pipeline, before actiosn are dispatched:
let (count, remaining) = crate::app::count_prefix::extract_count(&self.key_stack);
COUNT_PREFIX.set(count);
// Continue processing remaining keys...
```

**Modified**: All `increment_list` implementations — multiply amount by count:
```rust
fn increment_list(&mut self, amount: isize) {
    let count = COUNT_PREFIX.get() as isize;
    let actual_amount = amount * count;
    // ... existing logic with actual_amount
}
```

---

## Phase 4: Rebuild `g` mode with all Go-To keys (10 min)

After Phase 2, consolidate `g` modes across all contexts:

| Key | Action | Available in |
|-----|--------|-------------|
| `g g` | Go to first | All list contexts |
| `g G` | Go to last | All list contexts |
| `g a` | Go to Artist | Song list contexts |
| `g b` | Go to Album | Song list contexts |
| `g l` | Go to Logs | Global context |

---

## Execution Order

| Phase | Item | Est. | Files |
|-------|------|------|-------|
| 0 | Free digits: remove `0`/`1`/`2`/`6`, keep `5` | 10min | `keymap.rs` |
| 0a | Add `F1`=ViewLogs, add `g l`=ViewLogs | 5min | `keymap.rs` |
| 1a | Visual mode highlight | 5min | `playlist.rs` |
| 1b | Yank in visual mode | 10min | `playlist.rs` |
| 2 | Merge `g` mode (`gg`=First, `gG`=Last) | 10min | `keymap.rs` |
| 3 | Count prefix: `extract_count` + wiring | 30min | `count_prefix.rs` (NEW), `actionhandler.rs`, `ui.rs`, all `increment_list` |
| 4 | Consolidate `g` modes across contexts | 10min | `keymap.rs` |
| | Build + test + commit | 10min | |
| | **Total** | **~1.5h** | |

---

## Open Questions

1. **`F1` for logs acceptable?** Or prefer something else?
2. **Do we keep `5` as ViewPlaylist?** (exception to digit-free rule)
3. **Count reset behavior**: After a counted motion, should subsequent motions reuse the same count? (Vim: no, count is one-shot)
