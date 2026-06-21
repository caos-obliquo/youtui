# Youtui — Project Knowledge

## GOLDEN RULE
One feature at a time. Implement → test → commit → next. Never batch changes.
If things break, rollback and re-apply one-by-one.

## Build
- Workspace root: `/home/caos/builds/youtui/`
- Rust nightly (1.97.0)
- Binary: `cargo build --release` → `target/release/youtui`
- Dependencies: yt-dlp, ffmpeg, alsa-lib
- Tests: `cargo test --release -p youtui --bin youtui` (132 pass, 2 pre-existing config failures)

## Vision
Full vim-driven TUI for YouTube Music. Keyboard-only. No mouse.

### Design Principles
1. **Vim motions = direct keys.** `j/k/h/l/g/G/d/y/V/u/n/N/[/]` are muscle memory — always direct, never buried in menus.
2. **Context menu = everything else.** API calls, toggles, settings, info views → `o` mode context menu. User should not need to guess random direct keys for app-specific actions.
3. **Reusable component crates.** Every TUI component (ViTextEditor, SearchBlock, ScrollingTable) is extractable to `libs/` for reuse in future libre projects (Libre.fm client, Bandcamp nameyourprice browser, embedded player).
4. **Keyboard warrior stack.** dwl, Arch Linux, Neovim, Vimium, zsh-vi-mode — every keypress should feel right. Count prefixes (`5dd`), operators (`d`/`y`/`c`), motions (`w`/`b`/`e`/`gg`/`G`), visual mode (`V`+j/k, y yank).

### Future Integrations
- Libre.fm scrobbling (Last.fm API protocol — already have the client)
- Bandcamp nameyourprice scraper as a metadata provider
- Embedded music player via the already-decoupled `TaskManager` + `DecodeSong` pipeline
- Full libre source stack (more permissive than open source)

## Architecture

3-layer async callback:
```
Frontend (UI) → TaskManager → Backend (Server)
```

- **Frontend**: Ratatui TUI components in `app/ui/`. State mutations happen via `FrontendEffect` handlers.
- **Backend**: Server at `app/server/`. Handles downloads, API calls, audio decoding.
- **Tasks**: `ValidateMetadata`, `GetLyrics`, `DecodeSong`, etc. defined in `messages.rs` as `BackendTask` impls.
- **Effects**: Frontend reactions to backend results. `impl FrontendEffect<Playlist, ArcServer, TaskMetadata>`.

## Metadata Validation Pipeline

`messages.rs:730` — `ValidateMetadata::into_future`. Progressive search order:

```
1.  album.search(norm_for_lfm(clean_title))          # ← takes priority
    → album.getInfo → 17 tracks from Last.fm ✅
2.  track.getInfo(artist, clean_title)                # exact track match
3.  track.search(norm_for_lfm(clean_title))           # fuzzy track → re-fetch for album
4.  Discogs API (no auth, underground metal)          # `fetch_album_tracks` Phase 2
5.  Last.fm album.search fallback                     # `fetch_album_tracks` Phase 3
6.  MusicBrainz recording search                      # 1 req/s rate limit
```

### `norm_for_lfm` (`messages.rs:1074`)
Normalizes messy titles for database queries. Strips in order:
- `"FULL ALBUM"`, `"Full Album"`, `"full album"`, `"FULL LP"`, `"FULL EP"`, `"full-length album"`
- `" - Single"`, `" - EP"`, `" - LP"`, `" - full album"`
- Parenthesized blocks: ` (year - genre / genre2)`, ` (2000)`
- Bracketed blocks: ` [genre]`, ` [HD]`
- Replaces ` & ` → ` and ` for Last.fm compatibility

### `add_yt_video` title cleaning (`playlist.rs:735`)
Before `ValidateMetadata` is spawned, the raw yt-dlp title is cleaned:
1. Strip `"{artist} - "` prefix (case-insensitive)
2. Strip `"FULL ALBUM"` suffix (case-insensitive)
3. Strip `"  ("` suffix (parenthetical metadata)

### `fetch_album_tracks` (`messages.rs:926`)
Three-phase fallback for getting full tracklists:
- **Phase 1**: Last.fm `album.getInfo` (requires API key)
- **Phase 2**: Discogs API (no auth, works for underground extreme metal)
- **Phase 3**: Last.fm `album.search` → re-fetch `album.getInfo`

## Album Splitting

### Track creation
`insert_album_tracks` (`playlist.rs:575`): each track becomes a `ListSong` with:
- `track_no: Some(i+1)`, `start_offset: Some(accum)`, `actual_duration`
- `year` from validation result, falls back to original entry's yt-dlp year
- `duration_string`: `"M:SS"` format
- `download_status: None` initially → shared via `Arc::clone` from original when download completes

### Arc sharing
`handle_song_downloaded` (`playlist.rs:893`): when original album entry finishes downloading, its `Arc<InMemSong>` is cheaply cloned to all track entries with `DownloadStatus::None`.

### DecodeSong (`messages.rs` → `player.rs:112`)
Three fields: `(Arc<InMemSong>, Option<Duration> offset, Option<Duration> actual_duration)`.
- When both offset AND actual_duration are `Some`: ffmpeg extracts `-ss {offset} -t {actual_duration}` from the full audio → each track gets its own file of exact length.
- When only offset is `Some`: ffmpeg extracts from offset to end (no `-t` boundary).
- When offset is 0 and actual_duration is `None`: uses full audio without extraction.

### Progress (`handle_set_song_play_progress`, `playlist.rs:2000`)
- Track entries (`track_no.is_some()`): `d` is directly used as track-relative progress (ffmpeg already extracted the correct section).
- Non-album entries with offset: `d.saturating_sub(offset)`
- Progress is capped at `actual_duration` so display never exceeds track boundary.
- `>` key crash guard: `draw_media_controls.rs` checks `duration == 0` before division.

### Gapless auto-advance
- Gapless threshold: 1s before track end.
- When track-relative progress ≈ `actual_duration - 1s`, `QueueDecodedSong` queues next track.
- Next track is pre-decoded via `DecodeSong(next_arc, next_offset, next_actual_dur)`, played seamlessly.

### Scrobble fixes (`playlist.rs:818,2009`)
- Track entries scrobble individually: `self.album_tracks.is_none() || song.track_no.is_some()` creates `ScrobbleState` for each track.
- Persistent scrobble: `handle_set_song_play_progress` checks `should_scrobble()` on every progress update (~10Hz), not just at song change. Works in any context (lyrics, browser, playlist).

### Original entry removal
- After all tracks are ready (Arc shared + all `Downloaded`), the original full-album entry is removed from the playlist list. `Arc<InMemSong>` stays alive via track clones.
- Path A (validation first): `handle_song_downloaded` → remove original + play track 1.
- Path B (download first): `insert_album_tracks` → share Arc → remove original → play track 1.

### Cascade guard (`effect_handlers_playlist.rs:276`, `playlist.rs:606`)
- `target.album_tracks.is_none()` prevents per-track validation results from re-triggering `insert_album_tracks`.
- `existing_tracks >= tracks.len()` in `insert_album_tracks` itself prevents double-insertion.

### Album art from Last.fm
- `FetchAlbumArt(artist, album, api_key)` backend task queries `album.getInfo` → extracts largest image URL → downloads via `song_thumbnail_downloader`.
- `FetchAlbumArtEffect::Fetched` handler stores art on all playlist entries with matching `artist:album` string.
- Wired in `MetadataEffect::Validated` handler after album tracks are inserted.

## yt-dlp Integration

### Audio download pipeline
- Writes to temp file via `tempfile::Builder::new().suffix(".m4a")` (NOT `-o -` — stdout pipe produces corrupted data on yt-dlp 2026+ due to skipped FixupM4a post-processing).
- `--force-overwrites` prevents yt-dlp's resume feature from treating pre-existing 0-byte temp files as "complete" (writes nothing).
- `--extractor-args youtube:player_client=web_creator` only when cookie_path is configured (requires auth). Default extractor used otherwise.
- `--cookies-from-browser chromium` passed when cookie path configured.
- 5-minute timeout on `proc.wait()` prevents hung processes.
- Post-download validation checks for valid audio container header (ftyp/EBML/RIFF/OggS). Rejects garbage/empty with clear error.
- Logs detected container format (MP4 isom, M4A, WebM, WAV, Ogg).
- `add_yt_video` metadata fetch (`--dump-json`) passes `--cookies-from-browser chromium`.
- Cookie path flows: `main.rs → app.rs → YoutuiWindow::new → Playlist`.

## Lyrics Pipeline
Order: `Musixmatch` → `Genius scrape` (quality gate: reject < 50 chars or < 3 lines) → `Bandcamp URL construction` → `lyr CLI` → `error`

## F-Key Architecture (2026-06-20)

3 primary F-keys for navigation/search. Vi motions are the foundation — all navigation built on j/k/h/l/g/G/etc.

| Key | Action | Scope | Behavior |
|---|---|---|---|
| `F1` | Toggle YTM search | Everywhere | Opens SearchBlock with suggestions. Closes on F1 again, or on view switch (F2/F3). Text clears on close. From Playlist: opens search popup overlay. |
| `F2` | Toggle Browser | Everywhere | Go to Browser / return to previous view (prev_context restore). Auto-dismisses any open search. |
| `F3` | Toggle Queue | Everywhere | Go to Playlist / return to previous view (prev_context restore). Auto-dismisses any open search. |
| `F7` | ChangeSearchType | Browser | Switches browser search type (artist/song/playlist). Moved from F6. |
| `F11` | ViewLogs | Global | Logger view. Unchanged. |

### F2/F3 toggle mechanism (Option B2)
```
F2:
  if context == Browser → restore prev_context (leave Browser)
  else → save context to prev_context, switch to Browser

F3:
  if context == Playlist → restore prev_context (leave Playlist)
  else → save context to prev_context, switch to Playlist
```
Each F-key has single identity — press to enter its view, press again to return to last context. No-op when no previous context exists (already in target view at startup).

### Search split: F1 vs `/`

| Key | Type | Scope | Behavior |
|---|---|---|---|
| `F1` | Native YTM search | All windows | Backend API call. Uses `SearchBlock` with YTM suggestions. Returns new results from API. |
| `/` | Local fuzzy finder | All windows | In-memory filter of current list. Case-insensitive fuzzy matching on visible data. No API call. |

This replaces the old behavior where `/` in Browser triggered an API search. Now `/` is always local, F1 is always native.

### Queue View Fidelity
The Playlist view (queue) is left **entirely untouched** by all refactoring. It is the user's favorite view. All changes target Browser, popups, search — never the Playlist core logic, rendering, or gapless auto-advance.

## ViTextEditor (`libs/vi-text-editor/`)
Standalone crate for reuse. Single-line (command input) or multiline (config editor).
- **Modes**: Normal, Insert, VisualLine, VisualChar, OperatorPending
- **Motions**: h/l/w/b/e/0/$/gg/G/j/k, f/F/t/T, ;/, repeat
- **Operators**: d (dd/dw/d$/dh/dl), c (cw/c$/cc), y (yy/yank), r (replace)
- **Undo/Redo**: 50-entry stack (undo), 50-entry stack (redo via C-r)
- **Clipboard**: internal string, p/P paste
- **Count prefix**: supported through youtui's `pending_count` mechanism
- **Tests**: 23 pass
- **Deps**: crossterm only
- **API**: `handle_key(KeyCode, shift: bool, ctrl: bool) -> bool`

## Design Principles (Suckless + Vim-Driven)
1. **Vim motions = direct keys.** Never buried in menus.
2. **Context menu = everything else.** `o` mode for API calls, toggles, settings.
3. **Only Browser Library and Playlist/Queue may differ** — everything else must be consistent.
4. **Minimal deps.** ASCII word boundaries > unicode-segmentation dependency.
5. **Keyboard warrior stack.** dwl, Arch, Neovim, Vimium, zsh-vi-mode. Count prefixes, operators, motions, visual mode.

## Known Issues
- Native downloader (`rusty_ytdl::stream()`): ignores custom filter for some videos, downloads video-only MPEG-4. Workaround: `:` command uses yt-dlp (works).
- Metallum CLI integration blocked by Cloudflare (cf_clearance cookie + TLS fingerprint mismatch).
- 53 ytmapi-rs integration tests: 28 pass, 52 fail (missing browser auth + API format drift).
- Annotations display: last entry may be cut off (lyrics_popup.rs height calc).
- `:` command: single video URLs with autogenerated &list= no longer load extra tracks.
- Crossterm 0.29 `Event::Key` destructure mismatch (pre-existing, not our changes)

## Library Browser (4th Tab: Artist | Song | Playlist | Library)
- `app/ui/browser/library.rs` — LibraryBrowser struct, two-panel layout (category list + content)
- Categories: Liked Songs, Playlists, Artists, Albums — each fetches on focus
- `GetAllLibrarySongs`, `GetAllLibraryArtists`, `GetAllLibraryAlbums` backend tasks (messages.rs)
- `browser_library` keymap field with context menu (o: play, queue, lyrics, copy URL)
- `r` key reloads current category, `y` copies URL

## Auth Fix (Cookie Dedup)
- `ytmapi-rs/src/auth/browser.rs:96-130` — `parse_netscape_cookies()` now deduplicates via BTreeMap (last-wins)
- yt-dlp auto-refresh appends cookies without removing old ones → duplicates with DIFFERENT values for critical auth cookies (OSID, __Secure-3PSIDCC, etc.)
- Fix: use BTreeMap insert (last-wins) matching Python dict behavior

## Visual Mode (Vim-style)
- `V` key toggles visual line mode in playlist view
- `[V]` indicator shown in header
- `d` deletes visual range (visual_start to cur_selected)
- `u` undoes last delete (stack-based)
- `d gg` deletes from top to cursor, `d G` deletes from cursor to bottom

## Backend Tasks
- `GetAllLibraryPlaylists` — stream_api_with_retry_n (10 pages) → `Vec<LibraryPlaylist>`
- `GetAllLibrarySongs` — stream_api_with_retry_n (10 pages) → `Vec<TableListSong>` → converted to `Vec<ListSong>`
- `GetAllLibraryArtists` — stream_api_with_retry_n (10 pages) → `Vec<LibraryArtist>`
- `GetAllLibraryAlbums` — stream_api_with_retry_n (10 pages) → `Vec<SearchResultAlbum>`
- `GetPlaylistTracks` — stream_api_with_retry_n (50 pages) → `Vec<PlaylistSong>` (all pages, not just 100)
- `RenamePlaylist` — EditPlaylistQuery via query_browser_or_oauth
- `RemovePlaylistItems` — RemovePlaylistItemsQuery via query_browser_or_oauth
- `CreatePlaylistWithVideos` — 5k per playlist, auto-splits to pt. 1/pt. 2, 100-batch adds
- `AddSongsToPlaylist` — 100-batch adds (YouTube API per-request limit)

## Key Files

| File | Lines | Purpose |
|---|---|---|
| `app/server/messages.rs` | ~1250 | All backend tasks |
| `app/ui/playlist.rs` | ~2440 | Main playlist: track management, playback, scrobbling, visual mode |
| `app/ui/playlist/effect_handlers_playlist.rs` | ~555 | Frontend effect handlers |
| `app/ui/browser/library.rs` | ~660 | Library browser (4th tab) |
| `app/ui/browser.rs` | ~690 | Browser routing, tab dispatch |
| `config/keymap.rs` | ~2025 | All keybindings by context |

## Priority Order

| # | Step | File(s) | Est |
|---|------|---------|-----|
| 3 | `C-r` redo (commit) | `libs/vi-text-editor/src/lib.rs` + 6 callers | ✓ ready |
| 4 | `.` repeat last change | `libs/vi-text-editor/src/lib.rs` | med |
| 5 | `J` join lines | `libs/vi-text-editor/src/lib.rs` | small |
| 6 | `~` toggle case | `libs/vi-text-editor/src/lib.rs` | small |
| 7 | Lyrics hybrid line numbers | `lyrics_popup.rs` | med |
| 8 | Footer album format fix | `footer.rs` | small |
| 9 | Remove wide config | `~/.config/youtui/config.toml` | tiny |
| 10 | Text objects iw, i(, a(, i", a" | `libs/vi-text-editor/src/lib.rs` | large |
| 11 | `%` bracket match | `libs/vi-text-editor/src/lib.rs` | med |
| 12 | Album art full-window popup (`o.v`) | new popup + `playlist.rs` + `action.rs` + `keymap.rs` | large |
| 13 | Remove `r` direct key for lyrics | `keymap.rs` | tiny |
| 14 | `o.a`/`o.A` conflict + `o.r`→`o.l` | `keymap.rs` | small |
| 15 | Config.toml completeness (all in config) | `config.toml` + `keymap.rs` | med |
| 16 | Build + full test suite | verify | verify |

## Tests
- `insert_album_tracks_sets_correct_metadata` — start_offset accumulation, duration_string, year fallback
- `album_download_shares_arc_with_tracks` — Arc sharing via handle_song_downloaded
- `play_song_id_uses_start_offset_in_decode` — DecodeSong includes offset in effect chain
- `progress_is_relative_to_start_offset` — handle_set_song_play_progress subtracts start_offset (album tracks use d directly)
- `non_album_progress_subtracts_offset` — non-album entries with start_offset still subtract
