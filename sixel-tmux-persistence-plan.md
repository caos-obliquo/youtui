# Sixel Tmux Persistence Restudy Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix two related defects in youtui's footer album-art sixel rendering: (a) a visible flash on every album-art change, and (b) the album art vanishing after a tmux pane/window switch with no way to restore it.

**Architecture:** The footer album art is rendered two ways that fight each other. The ratatui `Image` widget writes the sixel during `terminal.draw()` (footer.rs:147), and `flush_sixel` (app.rs:977-1012) independently DCS-clears and rewrites it. The fix makes `flush_sixel` stop clearing when the ratatui path already wrote the image this frame, and makes the existing-but-dead `force_sixel_redraw` flag actually drive a flicker-free re-emit on `FocusGained`. Focus reporting (`?1004h`) is enabled at startup so `FocusGained` events can arrive at all.

**Tech Stack:** Rust nightly, ratatui 0.30.0, ratatui-image 10.0.8, crossterm 0.29.0, icy_sixel (via ratatui-image), foot 1.27.0, tmux 3.7c.

**Spec:** This plan synthesizes three investigation traces in `.omo/notepads/sixel-tmux-persistence/learnings.md` (sixel pipeline trace, tmux+terminal trace, OSS patterns trace). It supersedes the stale handoff at `sixel-tmux-persistence-plan.md` (repo root), which claimed zero `FocusGained` handling. That claim is false: the handler exists since commit 879f08a (PR #44) but is dead because focus reporting is never enabled and the `force_sixel_redraw` flag is never consumed.

## Global Constraints

- No em-dash (`--`) anywhere. Use hyphen (`-`) only.
- ASCII-only words, suckless, minimal deps, no new crates.
- 0 warnings across workspace. `cargo build --release` clean.
- Rust only. No shell plugins, no non-Rust solutions.
- One feature at a time: implement, user validates, commit, next.
- Debug logging at every decision point (info/error/debug), no silent paths.
- Docs are code: CLAUDE.md, TODO.md, docs/*.md must stay current with every commit.
- Do NOT touch the user's live Wayland session. All manual repro is user-driven.

---

## Context

Youtui renders the currently playing album cover as a sixel image in the footer (footer.rs:141-154). The sixel lives on the terminal's separate graphics layer, which ratatui's text buffer does not manage. Two code paths write to that layer:

1. **ratatui `Image` widget** (footer.rs:147 via `render_album_protocol`): writes the sixel during `terminal.draw()`. The sixel data is stored as the first cell's symbol (ratatui-image sixel.rs:102), so ratatui's buffer diffing writes it only when the data changes (ratatui-core buffer.rs:501). The encoded data already contains its own area-clear sequence (ratatui-image sixel.rs:56, 61), and inside tmux it is wrapped in DCS passthrough `\x1bPtmux;...\x1b\\` (cap_parser.rs:78).
2. **`flush_sixel`** (app.rs:977-1012): after every `terminal.draw()`, independently DCS-clears the whole graphics layer (`\x1bP0p\x1b\\`, app.rs:997) and rewrites the sixel, but only when `sixel_data != last_sixel_data` (app.rs:991).

The render loop (app.rs:323-386) draws at most every 33ms (app.rs:318, 358) and only when `needs_redraw` is set (app.rs:359). A 1s `AppEvent::Tick` (appevent.rs:14, 71-92) sets `needs_redraw` via app.rs:344, so a frame draws every second even when idle. `flush_sixel` is called after each draw (app.rs:367).

User environment (verified): tmux 3.7c, foot 1.27.0, `allow-passthrough on` (tmux.conf:19), `terminal-features 'foot:sixel'` (tmux.conf:20), `focus-events on` (set by tmux-sensible plugin, tmux.conf:68-69; confirmed live via `tmux show -g focus-events`). The app never sends `?1004h`, so the terminal never emits focus events and tmux has nothing to forward from foot.

## Symptoms

- **(a) Flashing**: the footer album art visibly flashes (blank frame) on every art change: track change, art fetch completion, popup close, terminal resize.
- **(b) Vanishing**: after switching tmux pane or window away and back, the album art is gone. It does not return on its own; only an action that changes the art data (new song, popup toggle) restores it. The stale handoff's claim that "a keypress restores it" is not reproducible in current code: arbitrary keypresses change no art data, so both re-emit paths skip.

## Root Cause (Flashing)

Verified in the sixel pipeline trace. Two writers race on every art change:

1. **Double render on art change**: when `sixel_data` changes, the ratatui `Image` widget writes the new sixel during `terminal.draw()` (footer.rs:147, ratatui-image sixel.rs:102, buffer diff at ratatui-core buffer.rs:501). Then `flush_sixel` sees `sd != last_sixel_data` (app.rs:991), issues a global DCS clear `\x1bP0p\x1b\\` (app.rs:997) which wipes the just-drawn image, then rewrites the same data (app.rs:998-1000). The blank frame between clear and rewrite is the flash. PR #43 (372d362, 67b3fed) fixed the per-frame case with the `sd == last_sixel_data` guard, but the art-change case still double-renders.
2. **The DCS clear is not tmux-passthrough-wrapped**: `flush_sixel` writes raw `\x1bP0p\x1b\\` (app.rs:997), unlike the sixel data which ratatui-image wraps via `escape_tmux` (cap_parser.rs:78). Inside tmux this raw DCS is dropped by tmux (not `\x1bPtmux;` passthrough), so the clear is ineffective there; outside tmux it clears the entire graphics layer.
3. **Popup close wipes the footer art**: `ClosePopup` (app.rs:789-827) writes a blank sixel over the popup rect, then sends `\x1bP0p\x1b\\` + `\x1b[2J\x1b[H` (app.rs:821-823), which clears the whole screen including the footer sixel. The next draw diff-skips the footer image (same symbol) and `flush_sixel` skips (`sd == last`), so the footer art stays gone until the next art change.

## Root Cause (Vanishing)

Verified in the tmux+terminal trace. After a tmux pane/window switch, tmux redraws the pane from its text grid; the outer terminal's sixel graphics layer is not re-emitted. Nothing in youtui restores it:

1. **Focus reporting never enabled**: terminal init (app.rs:256-258) sends only `EnterAlternateScreen` + `EnableMouseCapture`. Grep for `1004` across `youtui/src` returns zero hits. `destruct_terminal` (app.rs:1024-1033) has no `DisableFocusChange`. crossterm 0.29.0 ships `EnableFocusChange`/`DisableFocusChange` (event.rs:383, 399, writing `?1004h`/`?1004l`) and parses `\x1b[I`/`\x1b[O` as `FocusGained`/`FocusLost` (event/sys/unix/parse.rs:170-171), but youtui never uses them.
2. **The `FocusGained` handler is dead**: the arm exists (ui.rs:1020-1028) and sets `force_sixel_redraw = true` (ui.rs:1025) plus `invalidate_protocol_cache()` (ui.rs:1026), but `force_sixel_redraw` is never read anywhere (grep: only ui.rs:112 declaration, ui.rs:669 init, ui.rs:1025 set). The doc comment at ui.rs:108-111 claims "Consumed by `flush_sixel` (reset to false after a re-emit)" - false. `flush_sixel` (app.rs:977-1012) does not check it.
3. **Even if `FocusGained` arrived, nothing re-emits**: `invalidate_protocol_cache` (ui.rs:1403) re-encodes the same image to the same data, so `flush_sixel` still skips (`sd == last_sixel_data`, app.rs:991). The ratatui `Image` path also diff-skips unchanged sixel (ratatui-core buffer.rs:501, ratatui-image sixel.rs:84-85 comment). The fix is doubly broken.
4. **No keepalive timer**: grep `keepalive` finds only the doc comment ui.rs:110 and TCP keepalive server.rs:44. The stale plan's fix #4 was never implemented. Because ratatui diff-skips unchanged sixel, the 1s `Tick` draw alone cannot restore a wiped image.
5. **tmux `focus-events` is on but the app never asks**: `tmux show -g focus-events` returns `on` (set by tmux-sensible). tmux can generate its own `\x1b[I` for the active pane on focus change, but the app-side `?1004h` is still required for the foot-to-tmux path, and the handler is dead regardless. Both paths are broken.

## OSS Reference Patterns

Verified in the OSS patterns trace (learnings.md:1-242):

| Project | FocusGained handler | `?1004h` enable | Keepalive | Dirty check | Tmux passthrough |
|---|---|---|---|---|---|
| ratatui-image | No | No | No | Yes (ratatui diff) | Yes (`is_tmux` flag) |
| herdr | Yes (configurable `redraw_on_focus_gained`) | Yes | No | No (full redraw) | N/A |
| dirge | Yes (mode recovery) | Yes (periodic re-arm) | Yes (re-arm `?1004h`) | No | N/A |
| rmpc | No | Yes (configurable) | No | No | N/A |
| jcode | Yes (no full invalidate) | No | No | Yes (ratatui diff) | N/A |
| forestui | No | Yes (documented `?1004h` requirement) | No | No | N/A |

Key findings:

1. **No project uses a periodic redraw timer for sixel persistence.** The only keepalive-like mechanism is dirge's periodic `?1004h` re-arm (renderer.rs:210), which exists to survive external mode resets, not to re-emit graphics.
2. **`FocusGained` is the standard re-emit trigger** (herdr runtime.rs:242, dirge terminal.rs:211). herdr makes it configurable (`redraw_on_focus_gained`, default true) to trade flashing for corruption recovery.
3. **`?1004h` is mandatory** for focus events to fire at all (forestui terminal.rs:39: "without it the terminal never sends focus in/out at all"). forestui also pins the paired reset: every mode set has a matching reset.
4. **Dirty checks prevent flashing**: ratatui-image relies on ratatui's buffer diffing; jcode explicitly avoids full backend invalidation on focus change because "ED2 clear + full redraw causes visible lag" (state_ui.rs:132).
5. **tmux 3.7b had sixel redraw bugs** (yazi issue #4111, fixed in tmux 3.8 via tmux/tmux#5366). User runs tmux 3.7c, so this specific bug class is not the cause here, but it is a known upstream hazard.

## Fix Steps

### Fix 1 (MUST): Enable focus reporting at startup, disable at exit

**Files:**
- Modify: `youtui/src/app.rs:9` (import), `youtui/src/app.rs:256-258` (terminal init), `youtui/src/app.rs:1024-1033` (`destruct_terminal`)

**Interfaces:**
- Consumes: crossterm 0.29.0 `crossterm::event::{EnableFocusChange, DisableFocusChange}` (event.rs:383, 399).
- Produces: `FocusGained`/`FocusLost` events delivered to `handle_crossterm_event` (ui.rs:720) via the existing event routing (appevent.rs:165-189 passes non-mouse, non-key events through; app.rs:434-444 routes to `window_state.handle_crossterm_event`).

- [ ] **Step 1: Add the imports**

`youtui/src/app.rs:9` currently reads:
```rust
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
```
Change to:
```rust
use crossterm::event::{
    DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
};
```

- [ ] **Step 2: Enable focus reporting in terminal init**

`youtui/src/app.rs:256-258` currently reads:
```rust
enable_raw_mode()?;
let mut stdout = io::stdout();
execute!(stdout, EnterAlternateScreen, EnableMouseCapture,)?;
```
Change to:
```rust
enable_raw_mode()?;
let mut stdout = io::stdout();
execute!(
    stdout,
    EnterAlternateScreen,
    EnableMouseCapture,
    EnableFocusChange,
)?;
```

- [ ] **Step 3: Disable focus reporting in `destruct_terminal`**

`youtui/src/app.rs:1024-1033` currently reads:
```rust
fn destruct_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::cursor::Show
    )?;
    Ok(())
}
```
Change to:
```rust
fn destruct_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableFocusChange,
        crossterm::cursor::Show
    )?;
    Ok(())
}
```

- [ ] **Step 4: Verify the build**

Run: `cargo check -p youtui`
Expected: PASS, no warnings. If crossterm ever drops the structs (future version), fall back to manual escapes: `stdout.write_all(b"\x1b[?1004h")?` at init and `stdout.write_all(b"\x1b[?1004l")?` in `destruct_terminal`. crossterm 0.29.0 has the structs, so the fallback is documentation only.

### Fix 2 (MUST): Make `flush_sixel` consume `force_sixel_redraw` for a flicker-free re-emit

**Files:**
- Modify: `youtui/src/app.rs:977-1012` (`flush_sixel`)
- Modify: `youtui/src/app/ui.rs:108-111` (lying doc comment)

**Interfaces:**
- Consumes: `YoutuiWindow::force_sixel_redraw: bool` (ui.rs:112), set by the `FocusGained` arm (ui.rs:1025).
- Produces: `force_sixel_redraw` reset to `false` after a re-emit (via `std::mem::take`), matching the doc comment's original intent.

- [ ] **Step 1: Consume the flag in `flush_sixel`**

`youtui/src/app.rs:977-1012` currently reads:
```rust
fn flush_sixel(&mut self) -> Result<()> {
    use std::io::Write;
    // When album art popup is open, ratatui_image handles rendering via Image widget.
    // flush_sixel would DCS-clear and re-render the same image → visible flash.
    if self.window_state.album_art_popup.is_some() {
        return Ok(());
    }
    let rect = self.window_state.sixel_rect;
    let sd = self.window_state.sixel_data.clone();
    // Skip the DCS clear+redraw entirely when the sixel is unchanged. The
    // sixel lives on a separate graphics layer that ratatui's text clear
    // does not touch, so re-drawing every frame only causes a visible
    // flash (blank frame between clear and redraw). Persisting it on screen
    // is correct and flicker-free.
    if sd == self.window_state.last_sixel_data {
        return Ok(());
    }
    if let Some((data, rect)) = sd.as_ref().zip(rect) {
        let mut stdout = io::stdout();
        // Clear stale sixel at this position before re-drawing
        stdout.write_all(b"\x1bP0p\x1b\\")?;
        crossterm::execute!(&mut stdout, crossterm::cursor::MoveTo(rect.x, rect.y))?;
        stdout.write_all(data.as_bytes())?;
        stdout.flush()?;
    } else if let Some(rect) = rect {
        let mut stdout = io::stdout();
        crossterm::execute!(&mut stdout, crossterm::cursor::MoveTo(rect.x, rect.y))?;
        for _ in 0..rect.height {
            write!(stdout, "\x1b[{}X\x1b[1B", rect.width)?;
        }
        write!(stdout, "\x1b[{}A", rect.height)?;
        stdout.flush()?;
    }
    self.window_state.last_sixel_data = sd;
    Ok(())
}
```

Change the skip guard to bypass on `force_sixel_redraw`:
```rust
    let rect = self.window_state.sixel_rect;
    let sd = self.window_state.sixel_data.clone();
    let force = std::mem::take(&mut self.window_state.force_sixel_redraw);
    // Skip the DCS clear+redraw entirely when the sixel is unchanged and no
    // forced re-emit is pending. The sixel lives on a separate graphics layer
    // that ratatui's text clear does not touch, so re-drawing every frame only
    // causes a visible flash (blank frame between clear and redraw). A forced
    // re-emit (FocusGained, keepalive) bypasses this so a tmux-wiped image is
    // restored; the write path below uses no DCS clear, so it stays flicker-free.
    if !force && sd == self.window_state.last_sixel_data {
        return Ok(());
    }
```

- [ ] **Step 2: Fix the lying doc comment**

`youtui/src/app/ui.rs:108-111` currently reads:
```rust
    /// When true, `flush_sixel` must re-emit the current sixel even if the image
    /// data is unchanged (e.g. after tmux wiped the graphics layer on a pane/window
    /// switch). Set on `FocusGained` and by the keepalive timer. Consumed by
    /// `flush_sixel` (reset to false after a re-emit).
```
Change to:
```rust
    /// When true, `flush_sixel` must re-emit the current sixel even if the image
    /// data is unchanged (e.g. after tmux wiped the graphics layer on a pane/window
    /// switch). Set on `FocusGained` and by the keepalive timer. Consumed by
    /// `flush_sixel` via `std::mem::take` (reset to false after a re-emit).
```

### Fix 3 (MUST): Eliminate the double-render flash

**Files:**
- Modify: `youtui/src/app.rs:977-1012` (`flush_sixel`)
- Modify: `youtui/src/app/ui.rs:104-115` (add `last_sixel_rect` field), `youtui/src/app/ui.rs:666-668` (init)

**Interfaces:**
- Consumes: `YoutuiWindow::sixel_rect: Option<Rect>` (ui.rs:107), set by footer.rs:148 and draw.rs:65.
- Produces: `YoutuiWindow::last_sixel_rect: Option<Rect>` (new field), the rect of the last emitted sixel, used to decide when a global DCS clear is needed.

**Analysis:** On a same-rect data change, the ratatui `Image` widget already wrote the new sixel (with its own area clear) during `terminal.draw()` this frame. `flush_sixel`'s global DCS clear then wipes it and rewrites it, producing the flash. The global clear is only needed when the image moved (stale pixels at the old rect) or when the data is empty (an erase request from `quit_confirm`/`command_mode`, draw.rs:237/264, which relies on the DCS clear to remove the art). Two options:

- **Option A (recommended): rect-tracking + erase detection.** Add `last_sixel_rect`; emit the global DCS clear only when `data.is_empty()` (erase) or `Some(rect) != last_sixel_rect` (moved). Same-rect data changes and forced re-emits write with no clear. Residual: a one-time flash on terminal resize (the only rect-change case), which is rare and acceptable.
- **Option B: no-clear always.** Remove the DCS clear from the data-change path entirely and rely on the sixel data's own area clear. Residual: stale pixels at the old rect on resize, and the empty-data erase path (draw.rs:237/264) stops working. Simpler, but leaves artifacts and breaks the quit/command overlay clear.

Recommend **Option A** combined with Fix 2's no-clear forced re-emit.

- [ ] **Step 1: Add the `last_sixel_rect` field**

`youtui/src/app/ui.rs:104-115` currently has:
```rust
    pub sixel_data: Option<String>,
    pub last_sixel_data: Option<String>,
    pub sixel_rect: Option<ratatui::layout::Rect>,
```
Add after `last_sixel_data`:
```rust
    pub last_sixel_rect: Option<ratatui::layout::Rect>,
```

- [ ] **Step 2: Initialize the field**

`youtui/src/app/ui.rs:666-668` currently has:
```rust
    sixel_data: None,
    last_sixel_data: None,
    sixel_rect: None,
```
Add:
```rust
    last_sixel_rect: None,
```

- [ ] **Step 3: Gate the DCS clear on rect change**

In `flush_sixel` (app.rs:977-1012), replace the data-write branch:
```rust
    if let Some((data, rect)) = sd.as_ref().zip(rect) {
        let mut stdout = io::stdout();
        // Clear stale sixel at this position before re-drawing
        stdout.write_all(b"\x1bP0p\x1b\\")?;
        crossterm::execute!(&mut stdout, crossterm::cursor::MoveTo(rect.x, rect.y))?;
        stdout.write_all(data.as_bytes())?;
        stdout.flush()?;
    } else if let Some(rect) = rect {
```
with:
```rust
    if let Some((data, rect)) = sd.as_ref().zip(rect) {
        let mut stdout = io::stdout();
        // Global DCS clear only when the image moved or is being erased. On a
        // same-rect data change the ratatui Image widget already wrote the new
        // sixel (with its own area clear) during terminal.draw() this frame, so
        // a global clear here would wipe it and flash. Forced re-emits write
        // with no clear: the sixel data self-clears its own area. Empty data
        // (quit_confirm/command_mode, draw.rs:237/264) is an erase request and
        // MUST clear, otherwise stale pixels stay behind the overlay.
        if data.is_empty() || Some(rect) != self.window_state.last_sixel_rect {
            stdout.write_all(b"\x1bP0p\x1b\\")?;
        }
        crossterm::execute!(&mut stdout, crossterm::cursor::MoveTo(rect.x, rect.y))?;
        stdout.write_all(data.as_bytes())?;
        stdout.flush()?;
    } else if let Some(rect) = rect {
```

- [ ] **Step 4: Record the emitted rect**

At the end of `flush_sixel`, after `self.window_state.last_sixel_data = sd;` (app.rs:1010), add:
```rust
    self.window_state.last_sixel_rect = rect;
```

- [ ] **Step 5: Add a regression test**

Add to `youtui/src/app/ui/footer.rs` (alongside the existing regression tests at 349-410) a test asserting the `flush_sixel` data-write branch contains the rect-change guard and no unconditional DCS clear. Follow the existing source-text assertion style used by `album_art_none_preserves_sixel_data` (footer.rs:352-368):
```rust
/// Regression: flush_sixel must not DCS-clear on a same-rect re-emit (PR #43
/// fixed per-frame flash; this fixes the art-change double-render flash).
#[test]
fn flush_sixel_clears_only_on_rect_change() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/app.rs"))
        .expect("app.rs readable");
    let branch = src
        .split("if let Some((data, rect)) = sd.as_ref().zip(rect)")
        .nth(1)
        .expect("data-write branch present");
    assert!(
        branch.contains("data.is_empty() || Some(rect) != self.window_state.last_sixel_rect"),
        "DCS clear must fire on erase (empty data) or rect change, found:\n{}",
        branch
    );
}
```

- [ ] **Step 6: Run the test**

Run: `cargo test -p youtui flush_sixel_clears_only_on_rect_change`
Expected: PASS.

### Fix 4 (OPTIONAL): Keepalive / `?1004h` re-arm

**Files:**
- Modify: `youtui/src/app/ui.rs:1081-1084` (`handle_tick`), `youtui/src/app/ui/playlist.rs:2522` (empty `handle_tick`)

**Interfaces:**
- Consumes: `YoutuiWindow::tick: u64` (ui.rs:95), incremented by the 1s `AppEvent::Tick` (appevent.rs:14, 71-92).
- Produces: `force_sixel_redraw = true` every 3rd tick when a sixel is visible, driving Fix 2's re-emit.

**Analysis:** dirge's pattern (renderer.rs:210) periodically re-arms `?1004h` because an external reset (terminal reset, tmux detach/reattach, a multiplexer dropping private modes) can turn focus reporting off, after which no `FocusGained` ever arrives. Two sub-options, both guarded by `is_tmux` or a config flag:

- **Option A: periodic `?1004h` re-arm.** Every 3s, write `\x1b[?1004h` to stdout. Keeps the reactive `FocusGained` path alive. Cheap, no visual effect.
- **Option B: periodic forced re-emit.** Every 3s, if `sixel_data.is_some()`, set `force_sixel_redraw = true`. Belt-and-suspenders for tmux redraws that deliver no focus event at all. Costs one sixel write per 3s (no clear, so no flash).

Recommend **Option A** as primary (matches dirge, zero visual cost) with **Option B** as the fallback if empirical testing shows tmux 3.7c still drops the image despite `FocusGained` firing.

- [ ] **Step 1: Add a tick counter to `handle_tick`**

`youtui/src/app/ui.rs:1081-1084` currently reads:
```rust
    pub async fn handle_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.playlist.handle_tick().await;
    }
```
Change to:
```rust
    pub async fn handle_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.playlist.handle_tick().await;
        // dirge pattern: periodically re-arm focus reporting so FocusGained
        // keeps firing after an external reset (tmux detach/reattach, terminal
        // reset) turns ?1004 off. Guarded by is_tmux to avoid writes outside
        // tmux where the mode is already stable.
        if self.tick % 3 == 0 && std::env::var("TMUX").is_ok() {
            use std::io::Write;
            let _ = std::io::stdout().write_all(b"\x1b[?1004h");
            let _ = std::io::stdout().flush();
        }
    }
```

- [ ] **Step 2 (only if Option B needed): periodic forced re-emit**

If empirical verification shows the image still vanishes despite `FocusGained` firing, extend the same block:
```rust
        if self.tick % 3 == 0 && std::env::var("TMUX").is_ok() {
            use std::io::Write;
            let _ = std::io::stdout().write_all(b"\x1b[?1004h");
            let _ = std::io::stdout().flush();
            if self.sixel_data.is_some() {
                self.force_sixel_redraw = true;
            }
        }
```
The 1s `Tick` already sets `needs_redraw` (app.rs:344), so the next draw calls `flush_sixel`, which consumes the flag via Fix 2.

### Fix 5 (MUST): Document tmux requirements

**Files:**
- Modify: `docs/08-known-issues.md:32-60`
- Modify: `README.md` tmux section (if present)

- [ ] **Step 1: Add `focus-events` to every tmux block**

`docs/08-known-issues.md:32-60` documents `allow-passthrough` per terminal but never mentions `focus-events`. Add to each block (Foot, iTerm2, XTerm, Kitty):
```tmux
set -g focus-events on
```
And add one explanatory line after the `terminal-overrides` paragraph (docs/08-known-issues.md:60):
```markdown
`set -g focus-events on` makes tmux forward focus in/out to the pane. Youtui
enables focus reporting (`?1004h`) at startup and re-emits the album art on
`FocusGained`, so the cover returns after a pane/window switch without a keypress.
```

- [ ] **Step 2: Update README tmux section**

Add the same `set -g focus-events on` line wherever `allow-passthrough` is documented in README.md. Grep `allow-passthrough` in README.md first; if absent, skip this step.

- [ ] **Step 3: Update the stale known-issues claim**

`docs/08-known-issues.md:7-8` says "Footer album art broken in tmux - FIXED v1.0.3". Amend to reflect the current state after this plan lands:
```markdown
Footer album art in tmux: flash on art change and vanish on pane switch fixed
in this change (focus reporting + flicker-free re-emit). Requires
`allow-passthrough on` and `focus-events on` in tmux.
```

## Verification Checklist

Run in order. All must pass before commit.

- [ ] `cargo check -p youtui` - PASS, 0 warnings.
- [ ] `cargo test -p youtui` - all pass, including the new `flush_sixel_clears_only_on_rect_change` and the existing footer regression tests (footer.rs:349-410, PR #29/#36/#37 invariants: `None`/`Init` must not clear `sixel_data`, `Error` must clear it).
- [ ] `cargo build --release` - PASS, 0 warnings across workspace.
- [ ] Version checks: `foot --version` (expect 1.27.0), `tmux -V` (expect 3.7c).
- [ ] Config checks: `tmux show -g allow-passthrough` (expect `on`), `tmux show -g focus-events` (expect `on`), `tmux show -g terminal-features` (expect `foot:sixel`).
- [ ] Manual repro (user-driven, in the live session):
  1. Start youtui in tmux, play a track, confirm the footer album art renders.
  2. Switch tracks. Confirm NO flash (no blank frame) on the art change.
  3. `prefix+o` (switch pane) or `prefix+n` (next window), switch away, wait 2s, switch back. Confirm the art is still visible WITHOUT pressing any key.
  4. After switching back, wait 5+ seconds with no input. Confirm the art does not vanish (Fix 4 Option A keeps `?1004h` armed; if the image still drops, apply Fix 4 Option B and re-test).
  5. Open and close the album art popup (`o` menu). Confirm the footer art returns after close (regression for the `ClosePopup` wipe, app.rs:789-827).
  6. Resize the terminal. Confirm no stale pixels and no persistent flash (Fix 3 rect-tracking).
  7. Press `q` (quit confirm) and `:` (command mode). Confirm the album art is erased behind the overlay (regression for the empty-data DCS clear, draw.rs:237/264).
- [ ] Log check: `RUST_LOG=debug` shows `FocusGained: forcing sixel album art re-emit` on pane switch (ui.rs:1027), proving the event now arrives.

## Docs Updates

- `docs/08-known-issues.md:32-60`: add `set -g focus-events on` to all four tmux blocks + explanatory paragraph.
- `docs/08-known-issues.md:7-8`: update the stale "FIXED v1.0.3" claim to reflect this change.
- `README.md`: add `focus-events on` alongside `allow-passthrough` in the tmux section.
- `CLAUDE.md`: update the Known Issues section entry for sixel/tmux if it references the old behavior.
- `TODO.md`: mark the sixel persistence item done after user validation.

## Open Questions

1. **Does foot deliver `FocusGained` on a tmux pane switch?** With `focus-events on` at the tmux level, tmux can generate its own `\x1b[I` for the active pane, and foot can forward its own focus events once youtui sends `?1004h`. Which path actually fires in practice is empirical. The verification log check (ui.rs:1027) answers this.
2. **Does tmux 3.7c retain the sixel across pane switches at all?** The user has `terminal-features 'foot:sixel'` (tmux.conf:20), which tells tmux foot renders sixel so tmux preserves it. The symptom persists, so either tmux still drops it or the re-emit path was broken (it was). After Fix 1-3, re-test to see if `FocusGained` alone suffices or Fix 4 Option B is needed.
3. **Is the `ClosePopup` full-screen clear (app.rs:821-823) still necessary?** It wipes the footer art on popup close. The blank-sixel overwrite (app.rs:797-816) may be sufficient. Out of scope for this plan; note for a follow-up.
4. **Should `redraw_on_focus_gained` be configurable (herdr pattern)?** The current design always re-emits on focus gain. If users report flashing on focus return, add a config flag. Out of scope now.

## Risk / Mitigation

| Risk | Mitigation |
|---|---|
| Re-enabling the PR #43 per-frame flash | Fix 2 keeps the `sd == last_sixel_data` skip for the non-forced path; only `force_sixel_redraw` bypasses it, and that path writes with no clear. |
| Flash on terminal resize (rect change) | Accepted one-time flash; resize is rare. Option B (no-clear always) trades it for stale pixels, which is worse. |
| Empty-data erase stops working (quit_confirm/command_mode, draw.rs:237/264) | Fix 3's guard fires the DCS clear on `data.is_empty()` in addition to rect change; the regression test asserts both conditions. |
| `FocusGained` never fires even with `?1004h` (tmux drops it) | Fix 4 Option B (periodic forced re-emit) is the belt-and-suspenders fallback. |
| tmux 3.7c upstream sixel redraw bugs | Known class (yazi #4111, fixed in tmux 3.8). Not the cause here, but if symptoms persist after this plan, upgrade tmux to 3.8+ and re-test. |
| `force_sixel_redraw` consumed but no sixel visible (e.g. `sixel_data` is `None`) | `flush_sixel` falls through to the erase branch (app.rs:1001-1008), which is a no-op on an empty rect. Safe. |
| Regression: `None`/`Init` album art states must not clear `sixel_data` | Existing footer.rs tests (349-410) pin this invariant; Fix 3 does not touch footer.rs state handling. |
| 0-warning requirement | All changes are additive; no new dead code. The `last_sixel_rect` field is written and read in `flush_sixel`. |