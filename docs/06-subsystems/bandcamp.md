# Bandcamp

Second music source alongside YouTube Music. Search, queue, download, and playback all branch on one predicate: `is_bandcamp_url()` in `youtui/src/bandcamp.rs`. That file holds pure functions only (classification, normalization, JSON/HTML parsing). All I/O lives in `youtui/src/app/server/messages.rs`.

## The `video_id` convention

A Bandcamp track stores its FULL page URL in the song's `video_id` field:

```
https://artist.bandcamp.com/track/song-slug
```

not an 11-character YouTube id. Every downstream path checks `is_bandcamp_url(&raw_id)` and branches: yt-dlp target arg (`yt_dlp_target_arg` passes the URL verbatim instead of wrapping it as `youtu.be/...`), downloader binary selection, metadata resolution, share-URL copy (`song_share_url` in `structures.rs` copies Bandcamp rows verbatim instead of prefixing a watch URL), thumbnail cache keying. This is deliberate. Code that assumes `video_id` is always a YouTube id will mishandle these rows.

`is_bandcamp_url` accepts `<artist>.bandcamp.com/...` subdomains and rejects the bare `bandcamp.com` main site. `bandcamp_kind` classifies the path:

| Path | Kind |
|------|------|
| `/track/<slug>` | Track - queues directly |
| `/album/<slug>` | Album - resolved to track list first |
| `/` or `/music` | Discography - resolved to track list first |
| anything else | `None` - not playable |

`normalize_bandcamp_url` strips query, fragment, and trailing slash so the same album pasted with different tracking junk dedups to one queue entry. Non-Bandcamp input passes through unchanged.

## Search

Public endpoint: `POST https://bandcamp.com/api/bcsearch_public_api/1/autocomplete_elastic` with JSON body `{fan_id: null, full_page: false, search_filter, search_text}`. `search_filter` is always sent. `fetch_bandcamp_search` in `messages.rs` sends it with a Firefox User-Agent and Referer header, and retries twice on HTTP 429 with a 3s sleep.

One search fans out into three parallel calls (`SearchBandcamp` task):

| `search_filter` | Result type | Routed to |
|-----------------|-------------|-----------|
| `t` | track | Songs tab |
| `a` | album | Albums tab |
| `b` | band | Artists tab |

Each tab filters the merged result vec by `BandcampType` and appends its slice after the YouTube results (YouTube first, then Bandcamp). In Songs the merge is ordered: whichever of YouTube/Bandcamp finishes first waits via `pending_bandcamp` / `search_pending` so Bandcamp rows always land after YouTube rows. Bandcamp rows carry a `BC` badge in the `Src` column (`source_badge_for_song` in `songsearch.rs`, same badge in the artist search panel and album draw path).

Two parse quirks in `parse_bandcamp_search_results_all_types` (`bandcamp.rs`):

- Band results arrive in `item_url_root`, not `item_url_path` (what tracks and albums use). The parser tries both keys and skips items with neither.
- Band results have no `band_name` field, so it falls back to `name`.

Search rows have no duration, play count, or thumbnails. Queued Bandcamp tracks get duration and art later (see below).

## Opening an album or discography

`FetchBandcampAlbumEntries(url, use_cookies, cookie_browser)` in `messages.rs`:

1. `yt-dlp --flat-playlist --dump-json --no-warnings -- <url>` enumerates the tracks. `parse_bandcamp_album_entries` parses one JSON object per line, skips nested `playlist` entries, reads album/uploader from the `playlist_*` keys with per-item fallback.
2. One fetch of the album page parses the `data-tralbum` HTML attribute for what flat-playlist output lacks: per-track `duration` and `track_num` (`parse_tralbum_tracks`, matched to entries by `/track/...` URL path, positional fallback), the cover URL from `<meta property="og:image">` (`parse_tralbum_art_url`), and the release year (`parse_tralbum_release_year`). If the page fetch fails, the flat-playlist metadata is kept as-is.

Year parsing detail: the page contains many `release_date` keys (one per track inside `trackinfo`), so the year is read from the `current` object only - the slice between the `current` and `trackinfo` markers. Bandcamp's published date is treated as the authoritative album date.

The enriched entries land in two places:

- Albums tab (`HandleFetchBandcampAlbumEntriesOk` in `albumsearch.rs`): builds queueable rows with `video_id` set to each track URL. Note the browser track view sets `thumbnails: vec![]` and clears `album_year`; duration/year/art attach when the rows reach the queue.
- Queue (`insert_bandcamp_track_entry` in `playlist.rs`): one row per entry, no per-track yt-dlp probe. `cover_url` becomes a 1200x1200 thumbnail, `year` stamps `song.year`, `track_no` stamps `song.track_no`. The comment on the function states why: one probe per track means hundreds of yt-dlp processes on large compilations, which draws HTTP 429.

Pasting a URL (`:` prompt or `play_yt_url` in `ui.rs`): track URLs queue directly via `add_yt_video`; album/discography URLs go through `FetchBandcampAlbumEntries` first, then each entry enters through the same queue path.

## Metadata resolution for Bandcamp tracks

`resolve_bandcamp_metadata(raw_title, uploader, album, track)` handles two page layouts:

- Artist-owned page (`domnoise.bandcamp.com`): title self-prefixes (`D.O.M. - Song`), uploader is the artist.
- Label-hosted page (`sphcrecords.bandcamp.com`): uploader is the label, the real artist is the leading `Artist - ...` segment of the album name.

Artist priority: album-name prefix (when it differs from the page owner), then title prefix (when it differs from the owner), then title prefix anyway, then uploader. Title prefers yt-dlp's `track` field; without it, the owner/artist prefix is stripped from the raw title. Album keeps its name with the resolved artist prefix removed.

Two guards protect the result downstream:

- Queue insert (`insert_yt_video_metadata` in `playlist.rs`): Bandcamp rows use `resolve_bandcamp_metadata`; YouTube rows keep the title-split logic.
- Validation (`apply_metadata_fields` in `effect_handlers_playlist.rs`): a non-empty Bandcamp album from yt-dlp is never overridden by metadata providers, same as a YTM album.

## Recommendations (F4)

`ActOnRecommendation` in `messages.rs` resolves Last.fm picks YouTube-first, then Bandcamp:

1. YouTube search. The first hit is accepted only if `title_artist_matches(artist, title, hit.artist, hit.title)` passes - YouTube fuzzy-matches almost anything, so an unverified hit is kept aside, not used.
2. Bandcamp `t`-filtered search. Candidates must be track type, must classify as `BandcampKind::Track`, and must pass `bandcamp_result_matches`. First match wins via `bandcamp_search_result_to_song` (same `video_id`-holds-URL convention).
3. If neither verified, the unverified YouTube hit plays rather than erroring. Only when both sources return nothing does the task fail.

`title_artist_matches` normalizes to lowercase alphanumeric and requires artist equality-or-containment (both directions, for `V/A` and `feat.` credit variants) plus title equality-or-containment. Empty request fields skip that check. The same function verifies YouTube hits and Bandcamp candidates so the two paths cannot drift.

## Rate limiting

Three separate mechanisms:

| Where | What |
|-------|------|
| `server.rs` (`yt_dlp_semaphore`, 3 permits) | Guards `FetchYtVideoMetadata` yt-dlp probes. Album/discography resolution (`FetchBandcampAlbumEntries`) does not take it - it runs one yt-dlp call per URL plus one page fetch. |
| `fetch_bandcamp_search` (`messages.rs`) | Retries HTTP 429 twice, 3s sleep between attempts, per filter call. |
| `fetch_yt_dlp_json` (`util.rs`) | Up to 5 retries with exponential backoff: 3s, 6s, 12s, 24s, 48s. Covers spawn failure, 60s timeout, and non-zero exit. |

## Album art caching

Every entry from one album page gets the same `cover_url`. In the thumbnail downloader (`song_thumbnail_downloader.rs`), Bandcamp rows carry an empty album id, so `SongThumbnailID::from` falls through to `SongThumbnailID::Url(cover_url)`. Rows sharing one cover URL share one cache entry and one download. The `Display` impl keys it `C_<url>` with `/` and `:` replaced so full URLs are safe as filenames.

## Audio download and format

`YtDlpDownloader::stream_song` (`youtube_downloader/yt_dlp.rs`) picks the binary per URL: `bandcamp_yt_dlp_command` when set, else `yt_dlp_command`. yt-dlp runs without `--audio-format`, so Bandcamp's native lossless FLAC arrives raw. `detect_container` in the same file accepts `fLaC` magic alongside MP4/M4A/WebM/WAV/Ogg/MP3; anything unrecognized is rejected before decode.

## Config

```toml
yt_dlp_command = "yt-dlp"                  # default, used for YouTube
bandcamp_yt_dlp_command = "/path/to/yt-dlp" # optional, used for Bandcamp URLs only
```

`bandcamp_yt_dlp_command` defaults to unset and falls back to `yt_dlp_command`. It exists because the two sites need different yt-dlp builds: the curl_cffi-capable binary passes the Bandcamp client challenge but its YouTube extraction is broken, so each source gets its own binary. See `docs/04-configuration.md` for the full config reference.

## Debug tool

`youtui bandcamp-resolve <url>` prints the normalized URL, the `BandcampKind`, and either the track JSON (track URLs) or the enumerated entry list (album/discography URLs), using the configured Bandcamp binary. Same probe the TUI runs, without the UI.

## Tests

`bandcamp.rs` has unit tests for every parser: tralbum trackinfo/durations/paths, missing-key `None`, og:image extraction, year-from-`current`, path-vs-positional merge, album-entry playlist keys, URL classification/rejection, fbclid normalization, all-types search parsing including a live band payload shape (`item_url_root`, no `band_name`), and the match-function accept/reject matrix.
