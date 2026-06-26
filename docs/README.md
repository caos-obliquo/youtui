# Youtui Reference Manual

Vim-driven TUI for YouTube Music. Rust. Keyboard-only.

## Sections

| Section | Description |
|---------|-------------|
| [01-architecture](01-architecture.md) | 3-layer callback system, crate dependency graph |
| [02-crates/youtui](02-crates/youtui.md) | Main app crate (~35k LOC, 71 files) |
| [02-crates/ytmapi-rs](02-crates/ytmapi-rs.md) | YTM API client (12.8k LOC, 48 files) |
| [api-services](api-services.md) | External API setup (Last.fm, Discogs, Genius, Metal Archives) |
| [02-crates/async-callback-manager](02-crates/async-callback-manager.md) | Task/effect system (1.8k LOC) |
| [02-crates/json-crawler](02-crates/json-crawler.md) | serde_json wrapper (1k LOC) |
| [02-crates/vi-text-editor](02-crates/vi-text-editor.md) | Full VTE reference (2.3k LOC) |
| [02-crates/genius-rs](02-crates/genius-rs.md) | Genius lyrics + annotations SDK |
| [02-crates/metadata-provider](02-crates/metadata-provider.md) | Metadata resolution (6 providers, 47 tests) |
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
| [02-crates/ytmapi-cli](02-crates/ytmapi-cli.md) | CLI debug tool for ytmapi-rs |
| [ytmapi-rs-status](ytmapi-rs-status.md) | Feature matrix vs Python ytmusicapi |
| [subsystems/album_art_popup](subsystems/album_art_popup.md) | Sixel album art popup architecture |
| [subsystems/notes](subsystems/notes.md) | Notes popup system |
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
cargo test --release -p youtui --bin youtui    # 161 tests
cargo test --release -p vi-text-editor          # 65 tests
cargo test --release -p ytmapi-rs --lib         # 85 tests (no auth)
cargo test --release -p ytmapi-rs              # 29/51 auth (needs cookie)
```

## Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `youtui/src/app/server/messages.rs` | ~1598 | All backend tasks |
| `youtui/src/app/ui/playlist.rs` | ~3104 | Queue, playback, album splitting, visual mode |
| `youtui/src/app/ui/playlist/playlist_editor_popup.rs` | ~748 | Vim-driven playlist editor popup |
| `youtui/src/app/ui/playlist/effect_handlers_playlist.rs` | ~1302 | Frontend effect handlers |
| `youtui/src/app/ui/playlist/lyrics_popup.rs` | ~1210 | Lyrics + annotations display |
| `youtui/src/app/ui/playlist/album_art_popup.rs` | ~54 | Album art sixel popup w/ pagination |
| `youtui/src/app/ui/browser/library.rs` | ~2005 | Library browser (4th tab) |
| `youtui/src/app/ui/browser.rs` | ~1012 | Browser routing, tab dispatch |
| `youtui/src/app/ui/browser/albumsearch.rs` | ~731 | Albums tab |
| `youtui/src/config/keymap.rs` | ~2142 | All keybindings by context |
| `libs/vi-text-editor/src/lib.rs` | ~2486 | Vi-mode text editor |
