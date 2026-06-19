# Vim Enship — Yazi-Inspired F-Key Migration + Count Prefix

## The Problem

Digits `0`-`6` are bound to actions across these contexts:

| Digit | Context | Action | Replace with |
|-------|---------|--------|-------------|
| `0` | Global (line 808) | ViewLogs | **F11** |
| `1` | Playlist (line 830) | ViewBrowser | **F4** |
| `2` | Browser (line 1029) | Search | Already have `/` — REMOVE |
| `3` | Song lists (5+ contexts) | Filter | **F2** |
| `4` | Song lists (5+ contexts) | Sort | **F3** |
| `5` | Browser (line 1022) | ViewPlaylist | **F5** |
| `6` | Browser (line 1047) | ChangeSearchType | **F6** |

After migration: ALL digits `0`-`9` are pure count prefixes everywhere.

## Phase 1: Free Digits — F-Key Migration

### Files to change: just `config/keymap.rs`

#### 1a. Remove `0`=ViewLogs (global keybinds)
```rust
// REMOVE these lines (808-812):
(Keybind::new_unmodified(KeyCode::Char('0')), ...)
// ADD:
(Keybind::new_unmodified(KeyCode::F(11)),
    KeyActionTree::new_key_with_visibility(AppAction::ViewLogs, Global))
```

#### 1b. Remove `1`=ViewBrowser (playlist keybinds)
```rust
// REMOVE lines 830-834:
(Keybind::new_unmodified(KeyCode::Char('1')), ...)
// ADD to browser keybinds or global:
(Keybind::new_unmodified(KeyCode::F(4)),
    KeyActionTree::new_key_with_visibility(
        AppAction::Playlist(PlaylistAction::ViewBrowser), Global))
```

Wait — `ViewBrowser` in the playlist context dispatches `AppAction::Playlist(PlaylistAction::ViewBrowser)`. This is different from `BrowserAction::ViewPlaylist`. Let me check what it does:

From the action handler:
```rust
PlaylistAction::ViewBrowser => (AsyncTask::new_no_op(), Some(self.view_browser())),
```
which does:
```rust
pub fn view_browser(&mut self) -> AppCallback {
    AppCallback::ChangeContext(WindowContext::Browser)
}
```

So `1` in playlist = switch to browser view. And `5` in browser = switch to playlist view.

I'll bind F4 = ViewBrowser (switch to browser) and F5 = ViewPlaylist (switch to playlist). This makes F4/F5 the tab-switch pair.

#### 1c. Remove `2`=Search (browser keybinds)
```rust
// REMOVE lines 1029-1033:
(Keybind::new_unmodified(KeyCode::Char('2')), ...)
// `/` at line 1036 already does the same thing.
// Optionally ADD F1=Search as alternative:
(Keybind::new_unmodified(KeyCode::F(1)),
    KeyActionTree::new_key_with_visibility(
        AppAction::Browser(BrowserAction::Search), Global))
```

#### 1d. Remove `3`=Filter (ALL song list contexts)
Must remove `3` from:
- `default_browser_songs_keybinds` (line 1470)
- `default_browser_artist_songs_keybinds` (line 1186)
- `default_browser_playlist_songs_keybinds` (line 1308)
- `default_browser_library_keybinds` (line 1074)

Add F2=Filter to each:
```rust
(Keybind::new_unmodified(KeyCode::F(2)),
    KeyActionTree::new_key_with_visibility(
        AppAction::BrowserSongs(BrowserSongsAction::Filter), Global))
```

For browser_artist_songs: `AppAction::BrowserArtistSongs(BrowserArtistSongsAction::Filter)`
For browser_playlist_songs: `AppAction::BrowserPlaylistSongs(BrowserPlaylistSongsAction::Filter)`

#### 1e. Remove `4`=Sort (ALL song list contexts)
Same locations as `3`. Replace with F3.

#### 1f. Remove `5`=ViewPlaylist (browser keybinds)
Remove from `default_browser_keybinds` (line 1022). 
ADD F5=ViewPlaylist:
```rust
(Keybind::new_unmodified(KeyCode::F(5)),
    KeyActionTree::new_key_with_visibility(
        AppAction::Browser(BrowserAction::ViewPlaylist), Global))
```

#### 1g. Remove `6`=ChangeSearchType (browser keybinds)
Remove from `default_browser_keybinds` (line 1047).
ADD F6=ChangeSearchType:
```rust
(Keybind::new_unmodified(KeyCode::F(6)),
    KeyActionTree::new_key_with_visibility(
        AppAction::Browser(BrowserAction::ChangeSearchType), Global))
```

### What we end up with

| Old | Replaced by | Active in |
|-----|-------------|-----------|
| `0` = ViewLogs | **F11** = ViewLogs | Global |
| `1` = ViewBrowser | **F4** = ViewBrowser | Global |
| `2` = Search | `/` + **F1** = Search | Global |
| `3` = Filter | **F2** = Filter | Song lists |
| `4` = Sort | **F3** = Sort | Song lists |
| `5` = ViewPlaylist | **F5** = ViewPlaylist | Global |
| `6` = ChangeSearchType | **F6** = ChangeSearchType | Browser |

---

## Phase 2: Visual Mode Highlight + Yank

### 2a. Fix `get_highlighted_row()` — `playlist.rs:561`

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

When visual mode is active, the user sees the selection start highlighted. The current position (`cur_selected`) already has the normal selection highlight. This gives visual feedback of the full range.

### 2b. Yank in visual mode — `playlist.rs`: CopySongUrl handler

When `visual_mode` is true and `y` is pressed, copy selected lines as `"artist — title"` to clipboard:

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
            .map(|s| format!("{} — {}", 
                s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "),
                s.title))
            .collect();
        let _ = std::process::Command::new("wl-copy")
            .arg(lines.join("\n")).spawn();
        self.visual_mode = false;
    } else {
        // existing CopySongUrl logic
    }
}
```

---

## Phase 3: Merge `g` Mode (`gg`=First, `gG`=Last)

### Problem

`g` and `G` are standalone in `default_list_keybinds` (lines 1804-1809):
```rust
Char('g') => ListAction::First
Char('G') => ListAction::Last
```

But `g` is also a mode prefix for `g a`/`g b` in other contexts.

### Fix

Replace standalone `g`/`G` in `default_list_keybinds` with mode entries:
```rust
// In default_list_keybinds, REMOVE:
Char('g') => ListAction::First
Char('G') => ListAction::Last

// ADD g mode:
Char('g') => Mode "Go To":
    Char('g') => ListAction::First    // gg = First
    Char('G') => ListAction::Last     // gG = Last
```

Also add `g` mode entries to `default_browser_library_keybinds` and `default_browser_keybinds` if they don't already have `gg`/`gG`.

And add F7 = First, F8 = Last as backup.

Actually — keep `G` (Shift+g) as standalone for Last. In vim, `G` always means "go to last" regardless of count. And `gg` is for "go to first." So standalone `G` = Last is fine. We just need to make `g` a mode (for `gg`, `ga`, `gb`) instead of a standalone action.

---

## Phase 4: Count Prefix

### Architecture (yazi-inspired)

**New file**: `app/count_prefix.rs`

```rust
use crossterm::event::KeyEvent;

/// Extract count prefix from key stack.
/// Returns (count, remaining_keys).
/// No digits prefix → returns (1, original_keys).
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

**Modified**: `app/component/actionhandler.rs` — Add thread_local count:
```rust
thread_local! {
    pub static COUNT_PREFIX: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
}
```

**Modified**: `app/ui.rs` — In the key processing, before `handle_key_stack`:
```rust
let (count, remaining) = crate::app::count_prefix::extract_count(&self.key_stack);
COUNT_PREFIX.set(1.max(count));
// Build new key_stack from remaining
```

**Modified**: All `increment_list` implementations — multiply amount by count.

Actually, simpler: add a `count` parameter to `Scrollable::increment_list`:
```rust
pub trait Scrollable {
    fn increment_list(&mut self, amount: isize, count: usize);
    // ...
}
```

But this breaks the trait. Simpler: just read the thread_local in each impl.

---

## Phase 5: Build + Test

```bash
cargo build --release -p youtui --bin youtui
cargo test --release -p youtui --bin youtui
git add -A && git commit -m "feat: vim enship — f-key migration, count prefix, visual yank"
```

---

## Summary

| Phase | What | Lines | Est. |
|-------|------|-------|------|
| 1a | Move `0`→F11 (ViewLogs) | 5 | 5min |
| 1b | Move `1`→F4 (ViewBrowser) | 5 | 5min |
| 1c | Remove `2` (Search, `/` exists) | 3 | 2min |
| 1d | Move `3`→F2 (Filter, 4 contexts) | 20 | 10min |
| 1e | Move `4`→F3 (Sort, 4 contexts) | 20 | 10min |
| 1f | Move `5`→F5 (ViewPlaylist) | 5 | 5min |
| 1g | Move `6`→F6 (ChangeSearchType) | 5 | 5min |
| 2a | Visual highlight fix | 5 | 5min |
| 2b | Yank in visual mode | 15 | 10min |
| 3 | Merge `g` mode (`gg`/`gG`) | 15 | 10min |
| 4 | Count prefix | 40 | 30min |
| | Build + test + commit | — | 10min |
| | **Total** | **~138** | **~1.5h** |

## F-Key Layout Summary

| Key | Action | Type |
|-----|--------|------|
| F1 | Search | Core |
| F2 | Filter | Core |
| F3 | Sort | Core |
| F4 | ViewBrowser | Core |
| F5 | ViewPlaylist | Core |
| F6 | ChangeSearchType | Core |
| F7-F10 | *(reserved)* | — |
| F11 | ViewLogs | Rare |
| F12 | *(reserved)* | — |
