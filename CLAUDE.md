# Youtui — Project Knowledge

## Build
- Workspace root: `/home/caos/builds/youtui/`
- Rust nightly (1.97.0)
- Binary: `cargo build --release` → `/home/caos/builds/youtui/target/release/youtui`
- Release not debug. Target dir at workspace root, not `youtui/` subdir.

## `:` URL Playback
- `play_yt_url` → `add_yt_video` only (no `play_selected` call — it interfered with download stream spawning)
- `add_yt_video` returns effect directly, mapped via `map_frontend` to YoutuiWindow
- Download effect must be RETURNED from event handler — dropped effects = no download
- `handle_song_downloaded` auto-plays on `NotPlaying`/`Stopped`/`Buffering(id)` state (modified from only Buffering)
- Bug found: `play_selected` → `play_song_id` called `download_upcoming_from_id` twice (once in `add_yt_video`, once in `play_song_id`). First status=None, second status=Queued → no_op. But the first call's effect was DROPPED because `add_yt_video` original didn't return it.
- Fix: `add_yt_video` returns `ComponentEffect<Self>`, `play_yt_url` only returns `add_yt_video().map_frontend(...)`

## Metadata Validation
- `ValidateMetadata` backend task queries **Last.fm first** (track.getInfo), then **MusicBrainz** (recording search) as fallback
- Last.fm API key from `scrobbling_config.api_key` (same as scrobbler)
- Rate-limited: MusicBrainz 1 req/s via `tokio::time::sleep(1200ms)`
- Result updates song album/year/artist via `MetadataEffect::Validated`
- Handler updates `ListSong` fields: `album`, `year`, `artists` directly (all pub fields)
- Track number (`@attr.rank`) extracted from Last.fm when available, stored in `ListSong.track_no`

## Title Cleaning
- yt-dlp returns `title: "Artist - Song"` format (artist prefix baked in)
- Case-insensitive strip of `"{artist} - "` prefix needed for clean metadata queries
- Example: "Flowers Taped To Pens - Well I Guess" → "Well I Guess" (note "To" ≠ "to", need case-insensitive)

## Lyrics Pipeline
Order: Musixmatch → Genius scrape (quality gate: reject < 50 chars or < 3 lines) → Bandcamp URL construction → lyr CLI → error

## Bandcamp Lyrics
- `~/builds/bandcamp-lyrics/` — standalone Rust CLI (`bandcamp-lyrics` in PATH)
- Search (`bandcamp-lyrics <artist> <title>` or `bandcamp-lyrics search --artist ... --song ...`): tries Bandcamp search page first (often blocked), falls back to slug-based URL construction (`{slug}.bandcamp.com/track/{title}[-2..-5]`, tries both hyphenated and non-hyphenated artist slugs)
- Direct URL: `bandcamp-lyrics https://...bandcamp.com/track/...`
- Bandcamp track pages NOT blocked (only search page is)

## Album Art
- `AlbumArtState::Init` shows spinning icon `` — changed to `AlbumArtState::None` for songs without thumbnails
- Footer always reserves `ALBUM_ART_WIDTH` (7) + 1 spacing = 8 chars for album art space, even when no art
- Blank space `" "` rendered when no album art available, matching native layout
- Default album art init changed from `Default::default()` (Init) to `AlbumArtState::None` globally in all `add_raw_*` functions + test constructor
- **Future**: fetch album cover from Last.fm `album.getInfo` when validation finds album name

## Romaji Mode
- `R` key toggles `romaji_mode` on Playlist + LyricsPopup
- **Playlist**: saves original titles in `romaji_originals: HashMap<ListSongID, String>`, converts on toggle ON, restores on toggle OFF
- **Lyrics popup**: `R` toggle converts lyrics text at display time
- Uses `lindera` (IPADIC dictionary, ~55MB embedded) for kanji→hiragana readings, then `ib-romaji` for kana→latin
- Line-aware: preserves line breaks, only converts Japanese segments (hiragana/katakana/kanji), leaves English/ASCII/punctuation untouched
- Cached in `romaji_cache` — recomputed only when toggle or new lyrics arrive

## Annotations
- `GetAnnotations` task queries Genius API (search → referents `/response/referents`)
- Results stored via `AnnotationsEffect::FetchAnnotationsSuccess` → `LyricsPopup.set_annotations`
- `a` key toggles annotations view in lyrics popup
- Annotation display: `┌ fragment` header with indented explanation text
- `genius_token` read from `config.scrobbling.genius_token` (inside `[scrobbling]` section of config.toml)

## Footer
- Current song: `{play_icon} {title} - {artists}` on line 1, `{album}` on line 2
- Album art rendered left of progress bar when `AlbumArtState::Downloaded`
- `play_status.list_icon()` returns `''` (Buffering), `''` (Playing), `''` (Paused), `''` (Stopped/NotPlaying), `''` (Error)
- `DownloadStatus.list_icon_str()` returns " " (None), "↓" (Queued/Downloading), "✓" (Downloaded), "X" (Failed), "↻" (Retrying)

## Known Issues
- No command input popup — `:` shows as cyan `:text█` in footer
- Full album video → track splitting not implemented (future feature)
- Album art from Last.fm not yet fetched for URL songs

## Key Dependencies
- `ytmapi-rs` — YouTube Music API client (local workspace member)
- `tui-logger` — log viewer with j/k scroll (Logs view via `0`)
- `ib-romaji` — kana/kanji → romaji conversion (builder API, not direct constructor)
- `lindera` + `lindera-ipadic` — Japanese morphological analysis for kanji→hiragana readings
- `async-callback-manager` — task/effect framework (local workspace member)
- `musixmatch-inofficial` — lyrics source
- `reqwest` — HTTP client for Genius/Last.fm/MusicBrainz API calls
