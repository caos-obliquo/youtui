# Final Remaining Issues Plan

## Bug 1: Esc Never Closes Browser Search

**Root cause**: `vi_text_editor.rs:220` — `handle_insert()` returns `false` for ALL keys including Esc. The `SearchBlock::handle_text_event_impl` checks `changed` (which is `false`), so it returns `Some(no_op)` instead of `None`. The key never reaches `BrowserSearchAction::Close`.

**Fix**: In `handle_insert()`, when Esc is pressed (line 185-188), add `return true;` after `self.cursor -= 1;` to signal that the key changed the mode. This makes `changed = true`, returning `None`, which falls through to `handle_key_event` → `BrowserSearchAction::Close`.

**File**: `app/ui/components/vi_text_editor.rs:188`
**Lines**: 1

---

## Bug 2: `5` Exits Logs (Should be `F5`)

**Root cause**: `default_log_keybinds` at `keymap.rs:1629` still has `Char('5')` = `LoggerAction::ViewBrowser`. Since `5` is now a pure count prefix, this bind doesn't fire.

**Fix**: Change `Keybind::new_unmodified(crossterm::event::KeyCode::Char('5'))` to `Keybind::new_unmodified(crossterm::event::KeyCode::F(5))`.

**File**: `config/keymap.rs:1629`
**Lines**: 1

---

## Bug 3: `E` Popup Add to Playlist Still Returns 400

**Log evidence**:
```
Adding 1 videos to playlist in batches of 100
Adding batch of 1 videos
Got error Http error code 400
Retrying once
Error adding songs to playlist: Http error code 400
```

Single video ID (`HXxmbHMJ9rI`), playlist (`VLPL1Q2uZ1WIhIc-...`). Using `DuplicateHandlingMode::Unhandled` (dedup skip). Even retry fails.

**Likely causes** (need investigation):
1. **Video already in playlist** — `DEDUPE_OPTION_SKIP` might not be honored by this API endpoint for some playlist types
2. **VL-prefixed playlist ID** — The `VL` prefix on YouTube Music playlists might need stripping before API call. Other parts of the codebase strip `VL` when copying URLs (e.g., `playlist_id.get_raw().strip_prefix("VL").unwrap_or(...)`)
3. **Video not playable** — the video might be region-blocked or deleted

**Fix investigation**:
1. Check if other playlist-add operations also strip the `VL` prefix
2. Try removing the `VL` prefix before sending the request
3. Look at how `CreatePlaylistWithVideos` handles playlist IDs

**File**: `app/server/api.rs:297-306` (add_playlist_items)
**Lines**: ~5 for fix, ~15min investigation

---

## Bug 4: User Config Has Old Digit Keybinds

**Problem**: `~/.config/youtui/config.toml` has `0 = view_logs`, `2 = browser_search`, etc. These shadow the new F-key defaults.

**Fix**: Run this command to remove old digit keybinds:
```bash
sed -i '/^"?" = /d
/^0 = /d
/^"0" = /d
/^2 = /d
/^"2" = /d
/^5 = /d
/^"5" = /d' ~/.config/youtui/config.toml
```

Or delete the config and restart (will be regenerated with defaults).

---

## Execution Order

| # | Item | Est. | Files |
|---|------|------|-------|
| 1 | Esc close search fix (`return true`) | 1min | `vi_text_editor.rs` |
| 2 | Logs `5` → `F5` | 1min | `keymap.rs` |
| 3 | Investigate `E` 400 error (VL prefix) | 15min | `api.rs` |
| 4 | User cleans config | 1min | user's config.toml |
| 5 | Build + test + commit | 5min | — |
| | **Total** | **~25min** | |

## Open: Cookie Pagination

Current `parse_netscape_cookies()` deduplicates via BTreeMap. If the cookie file gets too large (thousands of lines from repeated yt-dlp runs), it's deduped down to ~200 unique cookies. The dedup happens in-memory, so file size isn't a problem. **No fix needed** unless cookies are actually failing (not just being large).

Ready to execute when you confirm.
