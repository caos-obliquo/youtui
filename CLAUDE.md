# Youtui — Project Knowledge

## GOLDEN RULE
One feature at a time. Implement → test → commit → next. Never batch changes.
If things break, rollback and re-apply one-by-one.

## User Preferences (Strict)
- **No sudo** without explicit permission.
- **No AUR.** Only official repos + local compilation.
- **Suckless.** Minimal deps, focused scope, ASCII-only words, no bloat.
- **Rust only.** No shell plugins, no non-Rust dependencies.
- **Subagent stack**: `rustacean` for Rust code review, `akita` for architecture/tooling decisions.

## Full Reference Manual

See `docs/` for the comprehensive reference:

```
docs/
├── README.md                        — Entry point
├── 01-architecture.md               — 3-layer callback, crate diagram
├── 02-crates/                       — Each crate: purpose, modules, API
├── 03-data-flow.md                  — Event → task → effect → render
├── 04-configuration.md              — All config.toml fields
├── 05-keybindings.md                — All contexts, actions, defaults
├── 06-subsystems/                   — Deep dive: lyrics, audio, queue, etc.
├── 07-testing.md                    — Test structure, commands
├── 08-known-issues.md               — Bugs and workarounds
└── 09-roadmap.md                    — Next features, crate extraction
```

**5,452 lines, 20 files, ~45 pages** — covers all 5 crates, 49k LOC.

## Build
- Workspace root: `/home/caos/builds/youtui/`
- Rust nightly (1.97.0)
- Binary: `cargo build --release` → `target/release/youtui`
- Dependencies: yt-dlp, ffmpeg, alsa-lib (system packages via pacman)

## Tests

```bash
cargo test --release -p youtui --bin youtui       # 126 pass
cargo test --release -p vi-text-editor             # 65 pass
cargo test --release -p ytmapi-rs                  # 28/80 pass (needs auth)
cargo test --release -p async-callback-manager     # 15 pass
cargo test --release -p json-crawler               # 8 pass
```

## Key Files

| File | Lines | Purpose |
|---|---|---|
| `app/server/messages.rs` | ~1280 | All backend tasks |
| `app/ui/playlist.rs` | ~2440 | Queue, playback, scrobbling, visual mode |
| `app/ui/playlist/effect_handlers_playlist.rs` | ~555 | Frontend effect handlers |
| `app/ui/browser/library.rs` | ~914 | Library browser (4th tab) |
| `app/ui/browser.rs` | ~690 | Browser routing, tab dispatch |
| `config/keymap.rs` | ~1982 | All keybindings by context |
| `libs/vi-text-editor/src/lib.rs` | ~2260 | Vi-mode text editor |

## ViTextEditor Summary

65 tests, all pass. Full feature set:
- Motions: `h/l/j/k/w/b/e/0/$/gg/G/W/B/E`, `f/F/t/T`/`;/,`, `%`
- Operators: `d`/`c`/`y`/`r`/`~`/`J`/`x`, with text objects `iw/aw/i(/a(/i"/a"/i'/a'/`` i`/a` ``
- Visual: `V` (line) and `v` (char) with `o` exchange, `c` change
- Surround: `ds`/`cs`/`ys` with `iw`/`W`/`$`/`ss` targets
- Switch: `^A`/`^X` number increment/decrement
- Repeat: `.`/`u`/`^R` with 50-entry stacks
- Proptest invariants for UTF-8 safety, undo/redo roundtrip
- Deps: crossterm only (intentionally suckless)

## Key Architecture

3-layer async callback:
```
Frontend (UI) → TaskManager → Backend (Server)
```

See `docs/01-architecture.md` and `docs/03-data-flow.md` for full detail.
See `docs/06-subsystems/lyrics.md` for lyrics pipeline.
See `docs/06-subsystems/validation.md` for metadata validation.
See `docs/06-subsystems/audio.md` for audio download + playback.
