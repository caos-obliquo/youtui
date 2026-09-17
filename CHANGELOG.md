# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Genre database with MusicBrainz OAuth2 + genre fetch and ListenBrainz genre validation
- MusicBrainz Cover Art Archive pipeline as album-art fallback before Last.fm
- Queue year enrichment on queue-add with rate limiting
- `resolve_fast` enrichment querying ListenBrainz + Last.fm for year, genres, and styles
- Library enrichment status indicator and queue batch enrichment
- SQLite metadata cache: MBID column, batch flush, CAA cache, instant year enrichment, and a CLI tool
- Library `#` column now shows position index
- Genre DB CLI subcommand: `youtui genre-db --list/--lookup/--stats` with persistent SQLite
- Progress bars (indicatif) on batch CLI commands (EnrichCache, TestValidateMetadata, MetadataCache, GenreDb, ScrobbleCache)
- `with_timeout`/`with_timeout_opt` helpers with consistent error reporting across all CLI subcommands
- Data-driven nav hint bar reading keybind config (lowercased labels, DarkGray centered)
- Footer plain Unicode thumbsup icon for liked tracks (replaces Nerd Font heart)
- Footer Nerd Font MDI level-based volume icons (mute/low/medium/high)
- SongInfoPopup enriched display with genres, styles, and descriptors
- Logger ToggleFullscreen + chord keybind (`gg`/`G`)
- `open_persistent()` and `get_subgenres_with_descriptions()` on genre-db-sqlite
- Library cookie auto-recovery via yt-dlp: `auth-refresh` CLI + `rebuild_from_cookie` rebuilds the YTM session from a fresh chromium cookie when the stored cookie expires
- `get-browse` CLI command wired to ytmapi-rs `BrowseQuery` (raw browse JSON for any browseId)
- `filter_youtube_cookies` strips foreign cookies before building the hyper header (avoids 64KB header overflow / 431 errors)
- `youtui log` CLI subcommand: tail/filter/search the `debug*.log` files `init_tracing` writes (`--follow`, `--filter`, `--since`, `--level`, `--json`)
- `ClearDownload` queue action (`o` menu, `c` key): force re-download of a cached track, for truncated-download recovery

### Changed
- Album splitting now only triggers for channel uploads or YTM tracks missing metadata; regular YTM tracks keep their correct structure
- Album art in library uses YTM thumbnail first, with TrackNo dash display and an enrichment cap
- `lookup_cache` falls back to SQLite on LRU miss
- Enrichment results autosave to SQLite instantly and persist across restarts via `sqlite_path`
- Tracing subscriber initialized for CLI commands; `EnvFilter` lets `RUST_LOG` control TUI log output
- Build now ships actual data files instead of absolute symlinks
- Header collapsed to 1 line (TAB_ROWS=1) with solid black background and chip-style command keys
- Nav hint bar replaced old hardcoded context strings with data-driven keybind config lookup
- Removed dead `draw_nav_hint` function (fully replaced by `draw_nav_hint_bar`)
- Metadata provider timeouts: 30s per provider across all 8 providers
- All CLI subcommands: consistent `with_timeout` + error message + progress indicator pattern
- ytmapi-rs library.rs parse improvements (VL prefix, library tracks)
- Footer volume display replaced from text (Vol N%) to Nerd Font level-based icons
- `/` fuzzy finder reworked to filter the visible list live (neovim-style): input on header, main list filters in real time; scoped to the active tab and cleared on tab switch / dive-in so it does not leak across views; `j/k` type into the query, `Up`/`Down` navigate the filtered list, `Esc`/`/` clears, `Enter` commits; `[SEARCH: text (N/M)]` indicator shown under the header

### Fixed
- Sixel tmux persistence: EnableFocusChange/DisableFocusChange (?1004h) at init/exit, flush_sixel re-emits popup sixel on FocusGained with rect-tracking guard, 3s keepalive re-arms ?1004h. Requires focus-events on + allow-passthrough on in tmux
- Stale `album_tracks` leaking split track names into the next song's scrobble
- Album split trusts metadata provider; six regressions fixed (VL prefix, reqwest version, EP/singles detection, Netscape cookie parsing)
- Album split guard skips YTM tracks that already have proper metadata
- Real AlbumID propagated through playlist conversion and metadata apply
- Snap selection to next matching index when a filter is active; snap-filter j/k in playlist tracks with backspace dismiss
- Duration guard on album split; `track_no` + year enrichment
- `resolve_year_fast` for enrichment speed; year overwrite fix
- Unconditional DCS clear removed from the album art popup (prevents sixel flash)
- Build: absolute symlinks replaced with actual data files
- Lyrics empty state: `set_lyrics` handles empty → Error transition properly
- Lyrics Japanese romanization: graceful fallback on parse failure instead of panic
- Lyrics error draw: retry hint shown in error display
- Lyrics timestamp parse: runtime logging for debug
- TestValidateMetadata: removed double-fetch (per-provider loop then `registry.resolve`)
- Footer album-art flicker: skip sixel redraw when the encoded image is unchanged
- CI security audit: bump `rkyv` 0.8.16→0.8.18 (clears RUSTSEC-2026-0233/0234/0235); ignore `RUSTSEC-2026-0258` (h2 0.3.27, unfixable without reqwest 0.11→0.12 migration)
- **Liked-songs `/` filter selection mismatch: Enter/j/k/menu now correctly target the filtered row instead of the full list (library.rs)**
- **First-entry filter snap: cursor now jumps to the first matching row when a filter is applied in Liked Songs and Playlists views (library.rs, browser.rs)**
- **Symphonia AAC `check failed` log spam suppressed in F11 view via three layers (tracing `EnvFilter` directives + tui-logger env-filter with explicit default level + exact-target `Off` table); startup fingerprint line proves fresh binary (app.rs)**
- **Repeat-One scrobble now fires on every replay by resetting scrobble state at the repeat boundary (playlist.rs)**
- **Stray `eprintln!` debug leftover removed from browser filter handler (browser.rs)**
- **Scrobble state duration updated from 240s fallback to actual track duration when available (playlist.rs)**
- **Logger fullscreen (`f`) now expands the log widget to the full window area (draw.rs)**
- **Footer progress total backfilled from decoded duration when YTM provides none, instead of `00:00` (playlist.rs)**
- **Songs-search Like column now maps `like_status` from YTM results instead of hardcoding Indifferent (structures.rs)**
- **Now-playing failures no longer silent: rejected `track.updateNowPlaying` responses log at error level, and the request now includes track duration (scrobbler.rs)**
- **Early audio end detection: tracks ending with played far below expected duration reset `download_status` for re-download and log loudly instead of silently stopping (playlist.rs)**
- **Progress bar freeze fixed: progress tracks the rodio audio position uncapped instead of clamping at the decoded duration estimate, which runs short on VBR streams (playlist.rs)**
- **Seek (`[`/`]`) no longer resets progress to zero on tracks with unknown duration, and no longer stalls at a short metadata duration (audio-player)**
- **Liked-songs filtered Enter/j/k fixed for real: cursor resolves through the matching set with first-match fallback, and `j/k` move in filtered-position space (library.rs)**
- **Early-end detector gated on observed progress updates so a stalled forwarder can no longer nuke a healthy download (playlist.rs)**
- **AudioQuality dead plumbing removal: enum + downloader plumbing + SetBestQuality binding removed (structures.rs, playlist.rs, keymap.rs)**
- **Lyrics popup key hints moved out of the box onto the shared nav-hint bar above the Status footer, matching queue placement (lyrics_popup.rs, draw.rs)**
- **Unknown keybinds no longer brick startup: runtime keymap parsing warns-and-skips stale bindings like playlist.set_best_quality instead of failing config load (keymap.rs)**
- **Footer elapsed time frozen at 00:00 fixed: duration clamp had shadowed the progress variable, bar filled while text stayed zero (footer.rs)**
- **Seek reports only positions the sink actually reached (audio-player, 96f577f).** Why: rodio 0.22 writes the requested pos into its position cache even when the inner seek returns Err, so the bar moved while audio stayed and key-repeat failures ran away. Effect: on failed seek the bar stays at the pre-seek position with best-effort restore; 4 new tests (success, Seek/SeekTo failure, repeat-no-accumulation)
- **yt-dlp auth uses the exported cookie file (yt_dlp.rs, 99312bf).** Why: cookie_path was only a web_creator flag and never passed to yt-dlp, so every download fell back to unauthenticated format 18 (96k AAC). Effect: `--cookies` passed when cookie.txt exists (legacy browser fallback otherwise); real 140/251 audio; ERROR log when progressive 18/22 is picked anyway
- **AudioQuality selection re-introduced as a cycle Best->High->Medium->Low with honest indicator (playlist.rs, ef37d67).** Supersedes the "AudioQuality dead plumbing removal" entry below: removal was premature, selection is back. `[Q:*]` shows the requested quality; actual abr/ext/yt_format in the yt-dlp completion log is the source of truth. Also: audio cache keyed by video_id+quality (no stale-quality resurrection), Resize debounced 200ms, seek tmp files sniff container + unique names
- **Dash-ID downloads no longer parsed as flags: end-of-options `--` before the video id in the yt-dlp stream path (yt_dlp.rs, b7752f8).** Why: ids like `-nIkN6le_wY` were lexed as CLI flags and the download died with exit status 2. Effect: those tracks now download normally; probe paths already pass full `https://youtu.be/` URLs so they were never affected
- **Footer album art self-heals and survives narrow panes (ui.rs, footer.rs, 93c4122).** Why: idle compositors wipe the sixel layer with no Focus/Resize event, and 0-dim chunks during pane drags broke `new_protocol`. Effect: art re-emits flicker-free every 30th tick while present, sub-4x2 chunks show a placeholder instead of encoding (normal chunk is 7x3), art auto-restores when space returns
- **Three UI freezes eliminated: watcher-death, yt-dlp block, clipboard hang (appevent.rs, messages.rs, playlist.rs, structures.rs, ae13629).** Why: (a) a dead crossterm EventStream silently ended the watcher and bricked input, (b) `add_yt_video` ran sync `Command::output()` on the event loop and parked behind a full network RTT, (c) a wedged `pbcopy` held the key-event thread on `wait()` with stdin open. Effect: watcher logs-and-continues on Err, rebuilds on None with 250ms backoff, escalates to QuitSignal after 10 consecutive ends; metadata probe runs as a 60s-timeout backend task with identical artist/year/duration fallbacks inserted on completion; clipboard closes stdin (EOF) before waiting, kills after 5s and reaps

## [v1.0.3] - 2026-06-27

### Fixed
- Cross-song album-art fetch guard used raw vs cleaned album name
- `canonical_album_name` cleared on every song change, breaking same-album tracks
- YTM `EP:/Album:/Single:` prefixes not stripped before scrobble
- `state.album` set from raw song name with prefixes instead of cleaned name
- Year parsed from channel upload titles `(YYYY - Genre)` before cleaning
- Autoplay scrobble path had no scrobble setup
- Boundary scrobbler double-firing on split tracks
- Footer cache wiped on `AlbumArtState::None`, clearing cached art
- `FetchAlbumArt` never fired on initial play and in autoplay
- Autoplay scrobbled album name as track title
- Tmux sixel vanishing on flush
- Last track duration leak giving uncapped progress bar
- Gapless advance used current song ID instead of next song ID

## [v1.0.2] - 2026-06-27

### Fixed
- Canonical Last.fm album name applied across all scrobble paths
- YTM `EP:/Album:/Single:` prefixes stripped before scrobble

## [v1.0.1] - 2026-06-27

### Added
- Cross-platform compatibility: clipboard fallback chain (wl-copy/xclip/xsel/pbcopy), `cookie_browser` config field, `std::env::temp_dir()` paths, Windows compile-time block
- Artist categories enum with Videos/Related/Playlists wiring
- Batch playlist streaming via continuations
- Audio cache keyed by `video_id` to avoid re-download on replay
- CLI sort flags and a liked-songs column
- Liked-songs column across all five browser tabs
- Metadata cache enrichment for library songs
- YTM album enrichment in the metadata pipeline
- Album art popup with pagination and like toggle
- Annotations UI with visual-mode yank/paste
- Library sort-order UI
- `ytmapi-cli` wiring for all 44 ytmapi-rs endpoints
- Genius CLI annotations subcommand
- Metal Archives proxy with Cloudflare handling and chromium support
- nvim-driven playlist editor with overwrite save
- ViTextEditor enhancements: visual block mode, text objects, f/F/t/T motions, `.` repeat, `C-r` redo, `~` toggle case, `J` join, `%` bracket match
- Playlist popups and visual-mode enhancements
- Config reload (`:reload`) and `SeekTo` callback
- Genius JSON lyrics API with annotations right panel and Enter-to-seek
- NavigationController and `:cmd` parser
- Library browser tab with visual mode and cookie dedup fix
- Metadata providers, song-info popup, and yt-dlp fix
- Album video splitting with ffmpeg extraction and metadata pipeline
- Album track splitting with scrobbling indicator
- URL playback, lyrics pipeline, annotations, romaji, and metadata validation
- Share (`y`) in context menu and URL playback scaffold
- Embedded Rescrobbled spawn on start, kill on exit
- Native scrobbler with Last.fm API integration
- o context menu in browser views
- vi-mode for search boxes
- Multi-provider lyrics (Musixmatch + Genius/AZLyrics/JahLyrics fallback)
- Fuzzy lyrics matching and scrollable lyrics popup
- Artist album category filter (`c` key)
- Global `/` search in browser views
- Dark Souls quit confirmation screen
- Native lyrics display via musixmatch-inofficial
- YouTube fallback search via yt-dlp
- Playlist creation set to Unlisted so it syncs to devices
- Audio quality default set to Best, downloader switched to yt-dlp with android_vr client
- Queue persistence across launches
- DBus notifications
- Simple shuffle and queue filter
- Performance: render throttle, stale download cancel, enter-spam guard, library lazy iterator, footer protocol cache, help-menu single pass

### Changed
- View-indices sorting for three browser tabs (Songs, PlaylistSongs, AlbumSongs)
- Albums tab refactored to AdvancedTableView with like/subscribe/audio_playlist_id
- PlaylistSearch tab fixed (was dead, now live)
- Footer format: 5-line footer, album art 7-char, heart icon, library tracks sort/filter
- Correct Nerd Font repeat/shuffle icons (MDI set, heart-only red)
- Green lettering for the playing song across all browser tabs
- Lyrics help text disambiguated; `()` lyrics vs `[]` song seek
- Genius hit validation relaxed to domain-only check
- Album split detection expanded to cover Full EP and Full LP

### Fixed
- UTF-8 crash on non-ASCII keys; liked column in queue; full heart icon
- Missing Liked column layout constraints across all five browser tabs
- Liked-songs `#` column showing row index instead of empty
- Annotation fragment full-width and absolute line numbers
- Annotation visual-mode highlight leak and page motions
- Sixel persistence: physically overwrite stale pixels on popup close, center within rect
- Extra space before heart icon in footer
- Colon key routing in lyrics popup
- Metadata scoring artist match and Discogs artist filter
- Discogs provider search and fallback behavior
- Playlist editor unsaved-changes warning, correct removal endpoint, `setVideoId`/`videoId` handling, VL prefix strip
- F7 tab cycle now saves back-navigation snapshot
- Log viewer toggle exits properly
- MoveTrackUp/Down local swap and filtered index fix
- Delete results re-routed to LibraryBrowser; filtered/sorted indices fixed
- Preserve tracks view across library refreshes
- Notes popup ctrl modifier for C-r redo, C-v visual block
- Esc in insert mode no longer moves cursor back
- Genius URL validation relaxed
- Genius annotations use real song ID from search API
- Lyrics section spacing, double-Esc
- Albums draw quadrants consistency
- Zero-warning build; rate toggle; J/K reorder
- 46 warnings eliminated; 10 ytmapi-rs fixtures fixed
- Visual mode, annotations scroll, VL prefix, art, editor fixes
- Library context menu, config section, d/g delete, `:playlist` URL
- Filter index mismatch and filter persist on close
- Album art panic guard; filter close interception
- Decode loop guard; album art throttle; nerd icons removed
- Search, icons, album art, annotations final polish
- Like/unlike, direct artist nav, build fixes
- Keybind standard and library playlist tracks browser
- Navigation hub, local search, go-to, UX polish
- Global C-y copy URL; `:` parser; annotations prep
- `:URL` includes album name + duration from yt-dlp metadata
- `:URL` fetches proper title/artist via yt-dlp metadata
- Fallback client version when INNERTUBE_CLIENT_VERSION missing
- `y` (share) in Enter + o menus; duplicate `d` fix
- Annotations fetch via Genius API
- `:URL` switches to playlist view for progress feedback
- Lyrics popup panic when closed before async response
- Lower scrobble threshold (15s or 33%), submit on stop, debug logging
- Proper vi-mode dw/db/dd/D with pending-key detection
- Logs on `0` instead of `l`; `A` for end of line in vim mode
- Sync example config with defaults
- Esc toggles vim mode on first press, closes search on second
- Transparent Dark Souls quit overlay
- Global `/` search in browser views; Esc closes search
- Unescape HTML entities and strip Genius metadata from lyrics
- Zero warnings; direct Genius scrape fallback
- Strip lyr metadata prefix from lyrics text
- Fuzzy lyrics matching via normalized title/artist variants
- Clamp `cur_selected` after category filter; smarter artist matching
- Multi-artist variants for lyrics lookup
- Parse artist Singles/EPs section (was silently ignored)
- Propagate album category through both API paths
- Category filter actually filters displayed items
- Also fetch EPs/singles from artist singles section
- Show album type (EP/Single/Album) in artist song browser
- Playlist creation set to Unlisted
- Remove `--cookies` flag; audio quality default Best
- Save popup size so description field is visible
- List state selection in playlist update popup
- Pass cookie file to yt-dlp for authenticated downloads
- Resolve BasicSearch deprecation; update deps; optimize footer
- Correct YT Music API paths; remove dead code; fix warnings
- Cached album art images
- Scrolling widgets scroll by unicode width
- Prevent playback ending when seeking back repeatedly
- Remove thumbnail download logic from notifications for instant responsiveness
- Fetch thumbnail before showing DBus notification
- Optimize network, memory, and caching
- Optimize download queue; add audio quality; improve UI status
- Compact playlist save format with metadata and prefetch
- Save playlists with minimal metadata, hydrate on load
- Ctrl+W deletes previous word in text inputs
- Optimize redraws, filtering, and table rendering
- Queue persistence across launch
- Search and shuffle fixes
- Simple filter on queue
- Shuffle logic
- go_to_first/last implementation
