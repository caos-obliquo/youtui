# TODO

## Done

- Album splitting: ffmpeg `-ss offset -t duration` per track, gapless QueueDecodedSong, track-relative progress, original entry removal
- Metadata pipeline: title normalization (FULL ALBUM, genres, brackets, - Single), album.search priority over track.search, Discogs API fallback, MusicBrainz fallback
- Scrobble: persistent check on every progress update, individual track entry scrobbling
- Year: from Last.fm album.getInfo wiki (not YouTube upload date)
- Album art: FetchAlbumArt task from Last.fm album.getInfo
- yt-dlp: web_creator client, --cookies-from-browser chromium, cookie path flow
- Empty download (0 bytes) → Failed
- `>` key crash fix (duration == 0 guard)
- Tests: 17 playlist tests (metadata, Arc sharing, progress, offset decode)
- Lyrics: Musixmatch → Genius → Bandcamp → lyr
- Annotations + Romaji mode

## Next

| Priority | Item | Notes |
|---|---|---|
| High | **Metallum CLI** | Standalone Rust CLI for Encyclopaedia Metallum. Blocked by Cloudflare cf_clearance TLS fingerprint mismatch. Need UA-matched cookies or curl_cffi approach. repo at `~/builds/metal-archives-cli/` |
| Medium | **Command input popup** | `:` shows as `:text█` in footer. Should be a centered popup with proper cursor. Pure UI, no logic change. |
| Low | **Album art edge case** | FetchAlbumArt may not fire when validation returns `artist=None`. Fallback: use album name from song entry fields. |

## Blocked

- 54 integration tests fail — YouTube API format drift (missing JSON keys). Needs API reverse-engineering.
- Artist album pagination — only first page returned. Needs `ParseFromContinuable` impl.

## Performance

- `compute_artists_string` duplicated between footer + table per draw. Marginal gain for caching complexity.
