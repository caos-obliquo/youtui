# Final Fix Plan — Esc Behavior + Warnings + E Error

## Fix 1: Esc Behavior (revert to vim standard)

**File**: `vi_text_editor.rs:188` — REMOVE `return true;` that I added.

Result:
- **First Esc** (anywhere, Insert mode) → Normal mode ONLY. Search/popup stays open.
- **Second Esc** (Normal mode) → dispatches whatever Esc is bound to (close search via `BrowserSearchAction::Close`, close popup, etc.)

## Fix 2: User config — `5` still exits logs

Run:
```bash
sed -i '/\b5 = /d; /"5"/d' ~/.config/youtui/config.toml
```

## Fix 3: `E` 400 error investigation (~10min)

**Diagnostic test**: In `api.rs:304`, temporarily change `DuplicateHandlingMode::Unhandled` back to `DuplicateHandlingMode::ReturnError`. If it still fails 400, the problem is NOT dedup — it's the video ID or request format.

If it's the video: the video might be a YTM "song" that doesn't have a standard YouTube video ID (YTM uses song IDs that aren't real YouTube videos).

If it's the request format: try stripping the `VL` prefix from the playlist ID before sending.

## Fix 4: 15 warnings (~12 min)

See previous plan — all are safe cleanup.

## Execution

1. Revert Esc (1 line, 30s)
2. User cleans config (30s)  
3. Investigate 400 error (10min)
4. Fix warnings (12min)
5. Build + test + commit (5min)

**Total**: ~28min but most is investigation. Quick fixes are Esc revert + warning cleanup (~15min).
