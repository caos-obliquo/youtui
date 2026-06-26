# TODO

## Vision

Full vim-driven TUI for YouTube Music. Keyboard-only. No mouse.

**Vim motions = direct keys.** `j/k/h/l/g/G/d/y/V/u/n/N/[/]` are muscle memory — always direct, never buried in menus.

**Context menu = everything else.** API calls, toggles, settings, info views → `o` mode context menu. Never guess random direct keys.

**Reusable component crates.** ViTextEditor extracted to `libs/vi-text-editor/`. SearchBlock, ScrollingTable are future extraction candidates for Libre.fm, Bandcamp nameyourprice, embedded player.

**Suckless philosophy.** Minimal deps, focused scope, no bloat. Keyboard warrior stack: dwl, Arch, Neovim, Vimium, zsh-vi-mode.

**Doc hygiene is hard rule.** Every doc change cross-references CLAUDE.md, TODO.md, docs/*.md — update all stale info. Test counts, file paths, line counts, feature status verified after every edit. Stale docs = bug.

## Architecture Decisions

### Key Mapping Principle
- **Direct keys reserved for vim motions/operators only**: `j/k/h/l`, `gg/G`, `d/y/V/u/n/N`, `[/]`, `g` prefix, `/` search, `Esc` clear search
- **Context menu (`o` mode) holds everything else**: shuffle, repeat, like, quality, filters, delete all, song info, album cover, go to artist/album, save to playlist
- Rationale: muscle memory keys consistent; app actions discovered via `o` (never guessed)

### Only views that may differ
- **Browser > Library** (4th tab) — unique layout with categories
- **Playlist/Queue** (F3) — core view, untouched
- Everything else (browser tabs 1-3, all popups) — must be consistent

### F-Key System
- F1 = toggle native YTM search (SearchBlock, everywhere)
- F2 = cycle Browser tabs / enter Browser
- F3 = toggle Queue/Playlist (prev_context restore)
- F7 = ChangeSearchType (Browser)
- F11 = ViewLogs

### Search Split
- F1 = backend API search with YTM suggestions (SearchBlock)
- `/` = in-memory fuzzy filter (ViTextEditor, no API)

## Done

### Session 2026-06-25 — Test gaps + dead_code + add-to-playlist + lowercase preserve

- **3 test holes filled**: resolve() integration (3 tests), metal_api parsing (6 tests), insert_album_tracks propagation (1 test)
- **#[allow(dead_code)] cleanup**: removed from scrobbler, api, actionhandler MouseHandler trait. Gated test-only builders with `#[cfg(test)]`
- **E button rename**: "Save Queue to Existing Playlist" → "Add Queue to Playlist" (always appends, no toggle)
- **normalize_artist_name**: preserves intentional lowercase (e.g. "data da morte"). Same fix in metal_api
- Test counts: youtui 136, metadata-provider 45, workspace ~369

### Session 2026-06-25 — Metadata Cache Persistence + Library Album Fix

- **Library songs now keep album data**: `HandleLibrarySongsOk` no longer drops `ts.album.name` and `ts.album.id` — Album column in library browser now shows real names from YTM API.
- **Genre pipeline loop closed**: `MetadataEffect::Validated` handler now copies `data.genres`/`data.styles` into `ListSong` — SongInfoPopup (`o.I`) shows real genres where providers return them.
- **Metadata cache persists to disk**: `~/.local/share/youtui/metadata_cache.json` — JSON file, atomic write via `.tmp`+rename. Loaded on startup, saved after each successful resolve. Survives restart.
- **youtui tests**: 134 passed (was 133 — 1 new `full_length_detected` test fixed via tag normalization)
- **CLI cache-test**: `ytmapi debug cache-test <artist> <title>` — verifies cache file write+reload end-to-end.

### Session 2026-06-25 — Toggle fixes + Annotations + Audit + Docs
- **ToggleSubscribeArtist global**: replaced separate o.S/o.U in browser_songs context with single o.S toggle. Added `subscribed_artists: HashSet` to AlbumSearchBrowser for state tracking.
- **RatePlaylist toggle bug**: fixed `liked_playlists.insert()` pattern (broke after first unlike) → `contains()` + explicit remove/insert in albumsearch.rs and library.rs.
- **Annotation wrapping fix**: added Paragraph widget line-wrap accounting in lyrics_popup.rs. Fragment + explanation `\n`-split lines now use `(text_len + inner_width - 1) / inner_width` to estimate visual rows.
- **ytmapi-rs test fix**: fixed `get_library_artists()` signature change + 25 deprecation warnings. 24 replaced with `new_filtered()`/`From` impl; 2 calls pass `None`. Warnings: 25→1 (pre-existing ytmapi-cli).
- **Dead code cleanup**: deleted `async_rodio_sink.rs` (dead after Phase 4 extraction to libs/audio-player/), duplicate `libs/rym-hierarchy.txt`.
- **Catch-all warn**: albumsearch.rs:518 `_ => {}` → `other => warn!(...)`.
- **Context menu awareness**: `Browser::is_song_action_visible()` filters BrowserSongsAction per browser variant + sub-state.
- **Phase 5 (Related tracks)**: yt-dlp per-video bounded concurrent enrichment (max 30, 5 semaphore) — `EnrichRelatedTracks` task + handlers.
- **Docs hygiene**: CLAUDE.md + TODO.md updated. Phase status, test counts (46→47 metadata, 14→18 genius), crate count (9→12). Added liked-songs-table planned feature.

### Phases A–M (All Implemented)
- **A**: Annotations cutoff fixed — `lyrics_popup.rs:547` added `.saturating_sub(1)`
- **B**: GoToArtist/Album moved to `o.a`/`o.b` in context menu (was broken `g` mode shadowed by list keybinds)
- **C**: Count prefix carries through modes — `5dd` deletes 5 items
- **D**: `delete_selected(count)` — deletes N items from current position
- **E**: ViTextEditor extracted to `libs/vi-text-editor/` — standalone crate, 12 tests pass
- **F**: Popup consistency verified — all use Cyan borders, ALL, Esc closes, j/k, footer hints
- **G**: `o.E` sends ALL queue IDs (not just current song), overwrite toggle (`O` key), gg/G vi motions in update popup, title shows `[Replace]`/`[Append]`
- **H**: Duplicate `r`/`l` lyrics removed — `r` direct key deleted, keep `o.l` only
- **K**: `e` motion (end of word), `c` operator (change = delete+insert), visual char mode (`v`)
- **M**: Non-vim direct keys moved to context menu — removed `s/A/c/D/z/;/I/E/Z/L` from direct, added to `o` mode: `o.s/A/c/D/I/z/t/E`

### ViTextEditor Steps 0–2
- **0**: Delete stale `components/vi_text_editor.rs` (unused duplicate)
- **1**: `f`/`F`/`t`/`T` motions + `;`/`,` repeat
- **2**: `r` replace single char

### Session 2026-06-24 (All 5 Batches Committed)
- **Batch A**: `:` colon routing in lyrics, annotations Bearer-first + pagination
- **Batch B**: Metadata scoring + Discogs artist filter, browser play triggers album split, artist/album normalization
- **Batch C**: Visual mode yank/paste (p), Esc exit visual mode, consistent color
- **Batch D**: Sixel belt-and-suspenders clear, heart spacing
- **Batch E**: Annotation visual mode (cyan highlight, cross-panel range clearing)
- Tests: 136 youtui, 46 metadata-provider, 369 workspace total

## Phase Tracking (from m0094 — 2026-06-25 session)

### Phase 1 — Small UI fixes ✅ DONE
| Item | File(s) |
|------|---------|
| Annotation panel last entry cut-off | `lyrics_popup.rs` |
| Force-split visual feedback (`last_status`) | `playlist.rs`, `effect_handlers_playlist.rs` |
| Album `audio_playlist_id` None guard | `albumsearch.rs` |

### Phase 1.5 — Scroll-centering ✅ DONE
| Item | File(s) |
|------|---------|
| Vim centered-scrolling (all table views) | `scrolling_table.rs`, `draw.rs` |
| Library Albums format (Artist - Album) | `draw.rs` |
| Browser Albums auto-load removed | `albumsearch.rs`, `browser.rs` |
| GoToArtist in Library | `library.rs` |

### Library Page Revision ✅ DONE
| Item | File(s) |
|------|---------|
| Context menu per-category filtering | `songsearch.rs`, `ui.rs`, `browser.rs` |
| GoToAlbum → AlbumOpen (direct tracks) | `library.rs`, `app.rs`, `albumsearch.rs`, `browser.rs`, `ytmapi-rs` |
| Enter: Artists→channel, Albums→AlbumOpen | `library.rs` |
| F1 search (all categories) | `library.rs` |
| `/` filter all 4 categories | `draw.rs` |
| `/` filter guard rail (zero command bleeding) | `ui.rs`, `browser.rs` |
| Subscribe single toggle (S key) | `songsearch.rs`, `library.rs`, `keymap.rs` |
| Plays column preserved from YTM | `structures.rs` |
| Lowercase artist names | `structures.rs` |
| Album art vanish fix | `draw.rs` |
| RatePlaylist for Library Albums | `library.rs` |
| Removed hardcoded "No albums/playlists found" | `draw.rs` |

### Metadata Pipeline Fixes ✅ DONE
| Item | File(s) |
|------|---------|
| Fallback split guard (requires video_dur OR album tags) | `effect_handlers_playlist.rs` |
| Album override guard (keep YTM when present) | `effect_handlers_playlist.rs` |
| Track-presence check (reject wrong album) | `effect_handlers_playlist.rs` |
| DiscogsProvider track validation | `discogs.rs` |
| TrackSearchProvider title matching | `lastfm_track.rs` |
| AlbumSearchProvider track validation | `lastfm_album.rs` |
| normalize_artist_name (ALL-CAPS preserved) | `structures.rs` |
| 6-provider metadata audit | analysis only |

### Phase 2 — Genius fallback + Year coverage + Scoring ✅ DONE
| # | Step | Status |
|---|------|--------|
| 1 | Genius annotations fallback — `__PRELOADED_STATE__` extraction | ✅ done |
| 2 | FFT footer bars (audio frequency viz) | ❌ cancelled (user cosmetic skip) |
| 3 | Year coverage gaps (Library LikedSongs + parenthetical fallbacks) | ✅ done |
| 4 | Metadata scoring review (+100 tracklist bias gated) | ✅ done |

### Phase 3 — LRCLIB + RYM genre data ✅ DONE
| # | Step | Status |
|---|------|--------|
| 1 | LRCLIB lyrics crate (`libs/lrclib-rs/`) with CLI debug tool | ✅ done |
| 2 | RYM genre/descriptor data from pre-scraped GitHub (`libs/rym-genre-data/`) | ✅ done |
| 3 | RYM genre descriptions in song info popup | ✅ done |

### Phase 4 — Audio-player crate extraction ✅ DONE
| # | Step | Status |
|---|------|--------|
| 1 | Extract audio logic to libs/audio-player/ | ✅ done |

### Phase 5 — Related tracks metadata enrichment ✅ DONE
| # | Step | Status |
|---|------|--------|
| 1 | yt-dlp per-video bounded concurrent (max 30, 5 semaphore) enrichment | ✅ done |

### Phase 6 — Cross-platform clipboard 🔴 NOT STARTED
| # | Step | Note |
|---|------|------|
| 1 | X11/macOS clipboard support | Wayland-only `wl-copy` today |

### Notes Popup — additional fixes ✅ DONE
| Item | File(s) |
|------|---------|
| Start in Normal mode (not Insert) | `notes_popup.rs` |
| Esc never closes (only `:q`/`:q!`/`:wq`) | `notes_popup.rs` |
| Scroll-offset tracking for long files | `notes_popup.rs` |

**Build**: `cargo build --release && cargo test --release` — verify no regressions before commit.

## Blocked
- Cross-platform clipboard (Wayland-only `wl-copy` — low priority, sidequest)
- Config template syntax (`o.enter`/`enter.enter` 2 pre-existing test failures)
- YouTube API format drift (external issue)
- Crossterm 0.29 `Event::Key` destructure mismatch (pre-existing, not our changes)

## Planned Features
- **Liked songs in table**: Show `LikeStatus` column in browser tables (Songs/Albums/Library) — not just footer heart on current track. Needs: parse `like_status` from YTM search response (SearchResultSong), wire into table rendering. Medium effort.

## Known Gaps (Consistency)
- **Footer album art**: Fetches async, brief blank on song change. Cache helps but not instantaneous.
- **Annotation panel last entry cut off**: Last entry partially hidden with very long explanation text. Wrapping-aware counting added but terminal-dependent.
