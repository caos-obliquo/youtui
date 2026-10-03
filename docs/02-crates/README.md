# Crate docs

One file per workspace crate. Files live here, next to this index.

| Doc | Crate | Contents |
|-----|-------|----------|
| [youtui](youtui.md) | `youtui` | Main binary: UI, backend tasks, queue, browser tabs |
| [ytmapi-rs](ytmapi-rs.md) | `ytmapi-rs` | YouTube Music API client |
| [async-callback-manager](async-callback-manager.md) | `async-callback-manager` | Task/effect dispatch between UI and backend |
| [json-crawler](json-crawler.md) | `json-crawler` | serde_json traversal helpers |
| [vi-text-editor](vi-text-editor.md) | `vi-text-editor` | Vim text editor widget |
| [genius-rs](genius-rs.md) | `genius-rs` | Genius lyrics + annotations client |
| [metadata-provider](metadata-provider.md) | `metadata-provider` | Metadata trait + provider impls |
| [ytmapi-cli](ytmapi-cli.md) | `ytmapi-cli` | YTM API debug CLI |

## Not yet documented

These crates have no doc file. Read the source directly:

| Crate | What it is |
|-------|------------|
| `audio-player` | Async rodio-based audio player, extracted from youtui |
| `lrclib-rs` | LRCLIB lyrics provider (free, no API key) |
| `rym-genre-data` | RYM genre/descriptor hierarchy data |
| `genre-db-sqlite` | SQLite genre hierarchy with MusicBee + RYM seed data |
| `metadata-cache-sqlite` | SQLite disk cache for enriched metadata |

## ytmapi-cli: which doc is canonical

Two files overlap:

- `docs/ytmapi-cli.md` - full reference: all 50+ endpoints as CLI commands.
- `02-crates/ytmapi-cli.md` (this directory) - short crate summary: purpose, auth setup, example invocations.

Start with `docs/ytmapi-cli.md` for usage. The crate file here covers crate-level layout only.
