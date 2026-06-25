# Youtui Reference Manual

Vim-driven TUI for YouTube Music. Rust. Keyboard-only.

## Sections

| Section | Description |
|---------|-------------|
| [01-architecture](01-architecture.md) | 3-layer callback system, crate dependency graph |
| [02-crates/youtui](02-crates/youtui.md) | Main app crate (29k LOC, 73 files) |
| [02-crates/ytmapi-rs](02-crates/ytmapi-rs.md) | YTM API client (12.8k LOC, 48 files) |
| [api-services](api-services.md) | External API setup (Last.fm, Discogs, Genius, Metal Archives) |
| [02-crates/async-callback-manager](02-crates/async-callback-manager.md) | Task/effect system (1.8k LOC) |
| [02-crates/json-crawler](02-crates/json-crawler.md) | serde_json wrapper (1k LOC) |
| [02-crates/vi-text-editor](02-crates/vi-text-editor.md) | Full VTE reference (2.3k LOC) |
| [02-crates/genius-rs](02-crates/genius-rs.md) | Genius lyrics + annotations SDK |
| [02-crates/metadata-provider](02-crates/metadata-provider.md) | Metadata resolution (6 providers, 46 tests) |
| [03-data-flow](03-data-flow.md) | Event routing, task spawning, effect chain |
| [04-configuration](04-configuration.md) | All config.toml fields with defaults |
| [05-keybindings](05-keybindings.md) | All contexts, actions, default keys |
| [06-subsystems/lyrics](06-subsystems/lyrics.md) | Pipeline, providers, quality gates, caching |
| [06-subsystems/validation](06-subsystems/validation.md) | Metadata pipeline, Last.fm/Discogs/MB |
| [06-subsystems/audio](06-subsystems/audio.md) | Download, decode, player, gapless |
| [06-subsystems/album-splitting](06-subsystems/album-splitting.md) | Track extraction, Arc sharing, offsets |
| [06-subsystems/scrobbling](06-subsystems/scrobbling.md) | Libre.fm/Last.fm integration |
| [06-subsystems/auth](06-subsystems/auth.md) | OAuth, cookie, browser auth flows |
| [06-subsystems/queue](06-subsystems/queue.md) | Persistence, shuffle, repeat modes |
| [06-subsystems/playlist-editor](06-subsystems/playlist-editor.md) | Vim-driven playlist editing popup |
| [07-testing](07-testing.md) | Test structure, running, coverage |
| [08-known-issues](08-known-issues.md) | Bugs, workarounds, version issues |
| [09-roadmap](09-roadmap.md) | Future features, crate extraction |
| [man/genius-rs.1](man/genius-rs.1) | Man page — genius-rs CLI (lyrics + annotations) |
| [man/ytmapi-cli.1](man/ytmapi-cli.1) | Man page — ytmapi-cli (YTM API debug tool) |

## Man Pages

Man pages for CLI tools are in `docs/man/`. Install system-wide:

```bash
sudo install -m 644 docs/man/genius-rs.1 /usr/local/share/man/man1/
sudo install -m 644 docs/man/ytmapi-cli.1 /usr/local/share/man/man1/
# Then view:
man genius-rs
man ytmapi-cli
```

## Quick Reference

```bash
# Build
cargo build --release

# Run
target/release/youtui

# Tests
cargo test --release -p youtui --bin youtui    # 136 tests
cargo test --release -p vi-text-editor          # 65 tests
cargo test --release -p ytmapi-rs              # 80 tests
```

## Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `app/server/messages.rs` | ~1280 | All backend tasks |
| `app/ui/playlist.rs` | ~2440 | Queue, playback, scrobbling, visual mode |
| `app/ui/playlist/playlist_editor_popup.rs` | ~320 | Vim-driven playlist editor popup |
| `app/ui/playlist/effect_handlers_playlist.rs` | ~555 | Frontend effect handlers |
| `app/ui/browser/library.rs` | ~914 | Library browser (4th tab) |
| `app/ui/browser.rs` | ~690 | Browser routing, tab dispatch |
| `config/keymap.rs` | ~1982 | All keybindings by context |
| `libs/vi-text-editor/src/lib.rs` | ~2260 | Vi-mode text editor |
