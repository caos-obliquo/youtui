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

### Fixed
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
