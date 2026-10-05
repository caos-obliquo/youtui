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
- **Bandcamp as a music source: search, album import, and metadata.** Songs/Albums/Artists search each fire a Bandcamp probe alongside the YouTube query and merge the results (YouTube first, Bandcamp appended), with a `BC` source badge on Bandcamp rows. Songs takes tracks, Albums takes albums (Enter lists the album), Artists takes bands (Enter lists the band's discography via `FetchBandcampDiscography`)
- **Bandcamp album import fills everything a flat playlist cannot:** track numbers, durations, cover art, and release year, all scraped from the album page's `data-tralbum` blob
- **Album-level metadata validation for Bandcamp imports:** one `ValidateMetadata` lookup per album instead of one per track, with the resolved year broadcast to every row of that album
- **`youtui test-validate-metadata` CLI** for inspecting provider resolution (provider chosen, score, tracklist, genres, year) without launching the TUI

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
- **Bandcamp requests are rate limited and retried.** Search queries all three result types and still renders partial results when one type fails. Downloads are capped at a few concurrent with retries on failure. The album page is always fetched because it is the only source for year, track numbers, and durations on compilations
- **Album art is cached by artwork URL for URL-added songs.** YouTube-URL and Bandcamp songs carry an empty album id, so the cache used to key on the per-track id and re-download one shared cover per track. The cover is now downloaded once per artwork URL
### Fixed
- **A track that cannot be downloaded is now skipped instead of stalling playback
  (playlist.rs).** Why: when the download of the track that was playing failed,
  the queue was marked failed and the next download was prefetched, but playback
  was never moved to the next track and nothing ever started it, so the music
  stopped and stayed stopped. Effect: a failed track that was the one playing is
  now skipped automatically and playback continues with the next track.
- **Repeated download failures no longer slow the queue to a crawl (yt_dlp.rs,
  song_downloader.rs).** Why: a video that has been deleted or made private was
  retried the full number of times before giving up, even though it could never
  succeed, and yt-dlp's reason was read only into the log rather than into the
  error. Effect: the reason now travels with the error, so an unavailable track
  shows why instead of just failing, and it is skipped without retrying.
- **A systemic download failure now stops with an explanation instead of skipping
  forever (playlist.rs).** Why: skipping every failed track is right for a few
  dead videos but wrong when the cause is systemic, such as expired cookies or no
  network, because it would work through the entire queue. Effect: after ten
  tracks fail in a row, playback stops and says so. Skipping a track in the
  background no longer interrupts the track being played.
- **A failed download now says why it failed, instead of "Max retries exceeded"
  (song_downloader.rs).** Why: the retry loop kept only whether the last attempt
  succeeded and threw the actual error away, so a track whose video no longer
  exists showed "Max retries exceeded" - a message about our retry policy that
  named nothing about the cause. Effect: the reason yt-dlp gave ("Video
  unavailable") is now kept through the retries and shown in the queue status,
  so an unavailable track can be recognised and removed rather than retried
  forever.
- **Album art no longer fails in bulk when many songs are played at once.**
  Why: every song that became the current one started its own cover download with
  nothing limiting how many ran at once, so skipping through a queue could start
  hundreds at the same time and the network connections were exhausted - the
  downloads failed before any server replied.
  Effect: cover downloads are limited to a few at a time, so a burst of tracks
  fetches its covers over a few seconds instead of failing.
- **Playback no longer freezes into an endless skip (playlist.rs).** Why: when the audio decoder produced a buffer that ended immediately, the song finished at once, the queue advanced, and the next song started decoding before any audio had played. Because the decoder was the problem rather than a stalled forwarder, the existing guard for truncated downloads could not see it: it deliberately ignores cases with zero played time or fewer than ten progress updates, which is exactly the shape of this failure. The result was the whole playlist being walked and wrapped thousands of times, saturating the main loop so the app stopped responding to keys. A live log showed 710,456 decode submissions across 444 songs over 20 hours with no playback at all. Effect: ten consecutive queue advances with no audio progress now stop playback, log the reason and show a message, instead of spinning
- **A second youtui instance can now start while another is running (media_controls.rs).** Why: every instance asked the session bus for the same media-player name. Registering it happens on a background thread whose result is unwrapped, so a name already taken panicked off-thread, ended the run loop, and left a window that rendered once and then ignored all keyboard input. Effect: each instance registers under its own name, so a second one starts normally; a desktop widget pinned to the exact previous name would no longer find it
- **Tracks are no longer silently downloaded as 96k video rips (yt_dlp.rs).** Why: when the authenticated session could not see a video's audio-only formats, the format request fell through to a progressive format and the app accepted it, so a 360p video containing about 96k audio was stored and played as the song with nothing shown to the user. Five tracks were affected. Effect: that outcome is now reported as a failure and goes through the normal retry path instead of storing bad audio, and the message names the cause (the cookie session cannot see this video's audio-only formats, so refresh cookies or play without them)
- **Two menu actions are reachable again (keymap.rs).** Why: two menu keys were each bound twice and the later entry silently won, leaving the earlier action impossible to invoke: the queue category filter and saving the queue to an existing playlist. Effect: the actions that were working keep their key and the unreachable ones moved to the previously unused uppercase `C`
- **A failed recommendations fetch no longer breaks F4 for a day (messages.rs, ui.rs, effect_handlers_playlist.rs).** Why: the three recommendation kinds (tracks, albums, artists) are fetched together and their failures were logged and discarded, so when all three failed the task still reported success with an empty list. An empty list is indistinguishable from "Last.fm has no recommendations", so it was written into the on-disk store with a fresh timestamp and the 24 hour cache then served that empty list forever, showing "No recommendations returned" on every F4 press and never retrying, even across restarts. Effect: a fetch where every kind failed is now reported as a failure instead of an empty result, so nothing is cached and the error is shown; an empty list already sitting in the store is discarded on load and refetched; partial success (at least one kind answered) still works and keeps the items it got
- **Recommendations can be refreshed with Shift+F4.** Why: clearing the cache had no keybinding, so there was no way to recover from a bad fetch. Effect: `Shift+F4` clears the cached and persisted recommendations and fetches them again
- **Recommendations no longer play the wrong song (messages.rs, bandcamp.rs).** Why: YouTube answers almost any search query with something, and the recommendation resolver took the first result without checking it was the track that was recommended. Pressing Enter on a Bandcamp-only recommendation could queue a completely unrelated YouTube video, which also meant the Bandcamp fallback could never run. Effect: the YouTube hit is now verified against the recommended artist and title (ignoring case and punctuation) before it is used; if it does not correspond, the Bandcamp search is tried; if neither source has a matching track, the unverified YouTube result is still played rather than failing, so no row that played before stops playing
- **Recommendations resolve tracks that only exist on Bandcamp (messages.rs).** Why: Last.fm recommends plenty of tracks that were never uploaded to YouTube, and pressing Enter on such a row failed outright with "No YTM search results". Bandcamp's autocomplete is fuzzy, so accepting its first hit would have queued an unrelated track. Effect: when YouTube returns nothing, the recommendation falls back to a Bandcamp track search and only accepts a candidate whose artist and title actually correspond, ignoring case and punctuation. If neither source has it, the error now names both
- **FLAC downloads no longer fail validation (yt_dlp.rs).** Why: yt-dlp runs without `--audio-format`, so the source container is written through untouched, and Bandcamp serves lossless tracks as raw FLAC. `detect_container` had no `fLaC` arm, so a perfectly valid download was reported as `invalid header: [66, 4c, 61, 43, ...]` and retried three times before failing with "Max retries exceeded". Effect: FLAC is now recognised and passes through to symphonia, which already decodes it
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
- **Seek reports only positions the sink actually reached.** Why: a failed seek still moved the progress bar while audio stayed, and key-repeat failures ran away. Effect: on failed seek the bar stays at the pre-seek position with best-effort restore; 4 new tests (success, Seek/SeekTo failure, repeat-no-accumulation)
- **yt-dlp auth uses the exported cookie file.** Why: the cookie path was never passed to yt-dlp, so every download fell back to unauthenticated low-quality audio. Effect: cookies passed when cookie.txt exists (legacy browser fallback otherwise); real 140/251 audio; ERROR log when a progressive fallback is picked anyway
- **AudioQuality selection re-introduced as a cycle Best->High->Medium->Low with honest indicator.** Supersedes the "AudioQuality dead plumbing removal" entry below: removal was premature, selection is back. The queue shows the requested quality; actual bitrate/format in the yt-dlp completion log is the source of truth. Also: audio cache keyed by video_id+quality (no stale-quality resurrection), Resize debounced 200ms, seek tmp files sniff container + unique names
- **Dash-ID downloads no longer parsed as flags: end-of-options marker before the video id in the yt-dlp stream path.** Why: ids starting with `-` were lexed as CLI flags and the download died with exit status 2. Effect: those tracks now download normally; probe paths already pass full URLs so they were never affected
- **Footer album art self-heals and survives narrow panes.** Why: idle compositors wipe the sixel layer with no Focus/Resize event, and 0-dim chunks during pane drags broke image encoding. Effect: art re-emits flicker-free every 30th tick while present, tiny chunks show a placeholder instead of encoding, art auto-restores when space returns
- **Three UI freezes eliminated: watcher-death, yt-dlp block, clipboard hang.** Why: (a) a dead terminal event stream silently ended the watcher and bricked input, (b) adding a video ran a sync download probe on the event loop and parked behind a full network round trip, (c) a wedged clipboard tool held the key-event thread waiting with stdin open. Effect: watcher logs-and-continues on errors, rebuilds with backoff, escalates to quit after 10 consecutive ends; metadata probe runs as a 60s-timeout backend task with identical fallbacks inserted on completion; clipboard closes stdin before waiting, kills after 5s and reaps
- **Copying a Bandcamp song no longer produces a dead link (structures.rs + 6 call sites).** Why: all 7 copy sites hardcoded `https://music.youtube.com/watch?v={video_id}`, but Bandcamp rows keep the full track URL in their `video_id`, yielding `https://music.youtube.com/watch?v=https://dramarecorder.bandcamp.com/track/...`. Effect: one shared `song_share_url` helper returns Bandcamp URLs verbatim and prefixes everything else; 4 new tests including a lookalike-host case
- **Bandcamp 429 rate limiting on large compilations (bandcamp.rs, messages.rs, effect_handlers_playlist.rs, playlist.rs, server.rs).** Why: `HandleBandcampAlbumEntriesOk` looped `add_yt_video` per URL, and each one fired its own `yt-dlp --dump-json` probe, so a 349-track album fired 349 concurrent requests and Bandcamp rate-limited after ~56. Effect: the single flat-playlist call now returns structured entries that insert directly, with a semaphore and backoff as defence for single-track adds
- **Bandcamp compilation rows showed `title=1` for every track (bandcamp.rs, playlist.rs).** Why: `resolve_bandcamp_metadata`'s 4th parameter is a track *name*, but `merge_tralbum_metadata` was writing `track_num` into it. Effect: `track_num` moved to its own `BandcampTrackEntry.track_no` field and is now written to `ListSong.track_no`; a test covers the previously untested insert path
- **Bandcamp release year is read from the right blob (bandcamp.rs).** Why: the whole-page search for `release_date` matched 11 occurrences, one per track inside `trackinfo`, and only worked by luck of `current` coming first. Effect: the parse is scoped to the `current`..`trackinfo` slice
- **Album-split year came from the upload date instead of the release date (util.rs, messages.rs).** Why: the fallback parsed `upload_date[..4]`, so a 2009 album uploaded in 2014 displayed 2014. Effect: release year first, then the description; `upload_date` is never used, and a `warn!` fires when a provider year disagrees with yt-dlp
- **`Untitled` tracks in split albums get real names (util.rs, messages.rs).** Why: a description-derived tracklist named every track `Untitled`, discarding a perfectly good provider tracklist. Effect: a Needleman-Wunsch alignment matches the two lists by duration before the yt-dlp tracklist overrides the provider's, renaming only placeholder titles; wrong-album providers yield zero renames because a mismatch scores worse than a drop
- **Age-restricted videos can be probed (util.rs, server.rs).** Why: `cookie_path` was never forwarded to yt-dlp. Effect: `--cookies-from-browser` is passed when a cookie file is configured
- **rodio dropping a pause reply no longer fails the audio-player test.** Why: pause returns None by design when the song is no longer selected, and the sink can die mid-test on a CI runner. Effect: the reply is awaited without failing, matching the pattern already used for seek in the same file

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
