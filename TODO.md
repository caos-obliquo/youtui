# Future TODOs

## Done (this session)
- `:` URL playback — auto-play on download complete, album/year metadata, case-insensitive title cleaning
- Metadata validation — Last.fm (primary) → MusicBrainz (fallback) for album/year/artist/track_no
- Lyrics pipeline — Musixmatch → Genius scrape (quality gate: >50 chars, >2 lines) → Bandcamp URL construction → lyr CLI
- Bandcamp lyrics CLI — `bandcamp-lyrics <artist> <title>` with slug-based URL fallback when search blocked
- Romaji mode — lindera (IPADIC) + ib-romaji for full kanji→kana→latin, line-aware segment conversion
- Annotations — Genius API search + referents, `a` toggle in lyrics popup
- Album art — `AlbumArtState::None` default for songs without thumbnails, 8-char footer padding always reserved
- Duration formatting — MM:SS instead of raw seconds
- Footer layout — matches native (indent, spacing)
- `genius_token` moved to `[scrobbling]` section in config (matches user's config.toml)

## Next
- **Full album video → track splitting** — `AlbumTrack`/`fetch_album_tracks` wired. TODO: scale track boundaries to video duration, scrobble at each boundary transition (silent, no UI changes)
- **Album art from Last.fm** — fetch cover when validation finds album name
- **Command input popup** — improve `:` UX (currently shows as cyan `:text█` in footer)

## Bug Investigation (Blocked)
- **54 integration tests fail** — YT API format drift (missing JSON keys like `gridRenderer/items`, `musicShelfRenderer/contents`). Needs API response reverse-engineering. Blocked on network captures.
- **Artist album pagination** — only first page returned. Needs `ParseFromContinuable` impl for `GetArtistAlbumsQuery`. Significant feature.

## Performance (Minor)
- `compute_artists_string` still duplicated between footer + table per draw. Would need interior mutability on `ListSong` to cache — marginal gain for complexity.

## Dep Tracking
- Upstream removed `AudioQuality` from structures.rs — if they finalize removal, adapt our fork's re-exports.
