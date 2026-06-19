# Vim Polish — Comprehensive Plan

## Phase 1: Quick Wins (~20 min, no digit conflicts)

### 1.1 Visual Mode Highlight
**File**: `playlist.rs:561-564`

Change `get_highlighted_row()` to return `visual_start` when `visual_mode` is active:
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
The current position (`cur_selected`) is already highlighted by the standard selection style. This gives visual feedback of the FULL range: start highlighted, end selected.

### 1.2 Yank in Visual Mode
**File**: `playlist.rs` — `CopySongUrl` handler

When `visual_mode` is true and `y` pressed, copy selected lines to clipboard:
```rust
PlaylistAction::CopySongUrl => {
    if self.visual_mode {
        let (start, end) = if self.visual_start <= self.cur_selected {
            (self.visual_start, self.cur_selected)
        } else {
            (self.cur_selected, self.visual_start)
        };
        let lines: Vec<String> = self.list.get_list_iter()
            .skip(start).take(end - start + 1)
            .map(|s| format!("{} - {}", s.artists..., s.title))
            .collect();
        let _ = std::process::Command::new("wl-copy").arg(lines.join("\n")).spawn();
        self.visual_mode = false;
    } else {
        // existing CopySongUrl behavior
    }
}
```

---

## Phase 2: `g` Mode Conflict Resolution

### Problem
`g` is currently used in TWO conflicting ways:
- List context: `g` = `ListAction::First` (standalone)
- Playlist/browser context: `g` = mode prefix for `g a`/`g b`
- Log context: `g` = `First`, `G` = `Last`

### Solution: Single `g` Mode
Merge all `g`-prefixed actions into ONE mode block per context:

**Playlist context** (`default_playlist_keybinds`):
```rust
'g' → Mode "Go To":
  'a' → GoToArtist
  'b' → GoToAlbum
  'g' → First        // was standalone g
  'G' → Last         // was standalone G
```

**Browser library** (`default_browser_library_keybinds`):
```rust
'g' → Mode "Go To":
  'a' → GoToArtist
  'b' → GoToAlbum
  'g' → First
  'G' → Last
```

**List context** (`default_list_keybinds`):
```rust
'g' → Mode "Go To":
  'g' → First
  'G' → Last
```

**Log context** (`default_log_keybinds`):
```rust
'g' → Mode "Go To":
  'g' → First
  'G' → Last
```

**File**: `keymap.rs` — 4 functions to modify
**Lines**: ~30

---

## Phase 3: Digit Rebinding + Count Prefix

### 3.1 Current Digit Usage

| Key | Context | Action | Keep? | Alternative |
|-----|---------|--------|-------|-------------|
| `0` | Global | ViewLogs | KEEP as `0` | Logs via `?` help or `g 0` optional |
| `1` | Global | ViewBrowser (tab 1) | KEEP | `g 1` optional |
| `2` | Global | BrowserSearch | **REMOVE** | Already duplicated by `/` |
| `3` | Song lists | Filter | KEEP in list context | Could also use `F3` |
| `4` | Song lists | Sort | KEEP in list context | Could also use `F4` |
| `5` | Global | ViewPlaylist (tab 5) | KEEP | `g 5` optional |
| `6` | Browser | ChangeSearchType | KEEP | Rarely used, context-specific |
| `7`-`9` | Unused | Nothing | FREE already | No change needed |

### 3.2 Problem: Only `2` is freed

For `10j` to work, ALL digits must be free as count prefixes when in list/playlist contexts. But `0`, `1`, `3`, `4`, `5` have important actions.

**Option A — Context-sensitive**: Digits are counts when `InputRouting` is `Content`/`List`, otherwise digit actions fire.

The problem: `1` in playlist → should it be count=1 or switch to browser tab? Different users expect different behavior.

**Option B — Move digits to F-keys** (vim-correct, most disruptive):
| Old | New | Notes |
|-----|-----|-------|
| `0`=Logs | `F1`=Logs | F-keys rarely used, good for secondary actions |
| `1`=Browser | `F2`=Browser | Tab 1 still accessible via browser context |
| `3`=Filter | `F3`=Filter | Actually natural: F3=filter in many apps |
| `4`=Sort | `F4`=Sort | Natural |
| `5`=Playlist | `F5`=Playlist | Tab 5 still accessible |
| `6`=ChangeSearch | `F6`=ChangeSearch | Rarely used |

**Option C — Hybrid**: Keep `0`, `1`, `5` as digits (rarely used as counts: `0` count is useless, `1` as count is same as no-prefix, `5` is only for tab switch). Free `2`, `3`, `4` for counts (move filter/sort to F3/F4, remove `2` since `/` exists).

This is the most pragmatic. With `2`, `3`, `4` free, users can do `2j`, `3k`, `4dd`. The common counts 2-9 are covered. Larger counts like `10j` would need `C-b`/`C-f` (page up/down) which already exist.

### Recommendation: Option C (Hybrid)

| Digit | Count Use | Action Use |
|-------|-----------|------------|
| `0` | ❌ (useless as count) | ViewLogs |
| `1` | ❌ (`1j` = same as `j`) | ViewBrowser (tab 1) |
| `2` | ✅ `2j`, `2k`, `2dd` | REMOVED, `/` covers search |
| `3` | ✅ `3j`, `3dd` | MOVED to `F3`=Filter |
| `4` | ✅ `4j`, `4dd` | MOVED to `F4`=Sort |
| `5` | ❌ (keep tab switch) | ViewPlaylist (tab 5) |
| `6` | ✅ | Keep in browser only |
| `7`-`9` | ✅ | Already free |

### 3.3 Count Prefix Implementation

**New file**: `app/count_prefix.rs`

```rust
/// Extract count prefix from key stack.
/// Returns (count, remaining_keys).
/// count=1 means no count prefix (default).
pub fn extract_count(keys: &[KeyEvent]) -> (usize, &[KeyEvent]) {
    let mut count = 0u32;
    let mut i = 0;
    for key in keys {
        if let KeyCode::Char(c) = key.code {
            if let Some(d) = c.to_digit(10) {
                count = count * 10 + d;
                i += 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    if count == 0 { (1, keys) } else { (count as usize, &keys[i..]) }
}
```

**Modified**: `app/ui.rs:global_handle_key_stack()`

Before dispatching action via `handle_key_stack`:
1. Extract count prefix from `self.key_stack`
2. If count > 1, pass count alongside the action
3. Apply count to the motion/action

The count is stored in a temporary field on `YoutuiWindow`:
```rust
pub count_prefix: usize,  // 1 = default (no prefix)
```

After action dispatch, reset to 1.

**Modified**: Key routes that handle list motions

In the playlist's `increment_list`, browser's `increment_list`, etc., multiply the amount by `count_prefix`:
```rust
// In playlist.rs:increment_list
fn increment_list(&mut self, amount: isize) {
    let amount = amount * self.count_prefix as isize;
    // ... rest of existing logic
}
```

Actually, the count can't be stored on Playlist since it's a component with no access to YoutuiWindow. Alternative: pass count through the existing `ListAction` system.

**Better approach**: Overload `ListAction::Down`/`Up` behavior using a global count tracker:

```rust
// In app.rs or app/component/actionhandler.rs
thread_local! {
    static COUNT_PREFIX: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
}
```

When count is extracted, store it. When a motion action fires, read it:
```rust
let count = COUNT_PREFIX.get();
for _ in 0..count { self.increment_list(1); }
```

This avoids changing any action enums.

### 3.4 Files Changed

| File | Change | Lines |
|------|--------|-------|
| `app/count_prefix.rs` (NEW) | `extract_count()` function | 20 |
| `app/ui.rs` | Hook into key processing, call `extract_count` + apply | 15 |
| `app/component/actionhandler.rs` | `COUNT_PREFIX` thread_local + apply helper | 15 |
| `config/keymap.rs` | Remove `2`=Search, move `3`/`4` to `F3`/`F4` | 10 |
| Config test file (if needed) | Update test expectations | 5 |

---

## Phase 4: Build + Test

```bash
cargo build --release -p youtui --bin youtui
cargo test --release -p youtui --bin youtui
git add -A && git commit -m "feat: vim polish — visual highlight, yank, count prefix"
```

---

## Summary

| Phase | Item | Est. | Depends on |
|-------|------|------|-----------|
| 1.1 | Visual mode highlight | 5min | — |
| 1.2 | Yank in visual mode | 10min | 1.1 |
| 2 | `g` mode conflict resolution | 10min | — |
| 3.2 | Free digits (remove `2`, move `3`/`4` to F3/F4) | 5min | — |
| 3.3 | Count prefix implementation | 30min | 3.2 |
| | Build + commit | 10min | All |
| | **Total** | **~1h 10min** | |

## Open Decisions for You

1. **F-keys**: OK moving `3`(Filter)→`F3`, `4`(Sort)→`F4`? Or prefer other keys?

2. **`2` (Search) removal**: `/` already searches in all contexts. `2` duplicate — sure to remove?

3. **`g` mode merge**: OK with `gg` = First, `gG` = Last instead of standalone `g`/`G`?

4. **Yank format**: Copy selected tracks as `"title - artist"` lines, or URLs, or something else?
