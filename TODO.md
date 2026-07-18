# TODO

## Completed (v1.0.0)
- CI pipeline: PR checks (test/linux/macos/freebsd/openbsd, build, lint, security audit)
- CI pipeline: automated release on push to main (patch bump + GitHub release)
- README: fixed F-keys claim, added independent fork tagline, ytmapi-rs reliability note
- LICENSE: single MIT file with all 3 copyright holders (sigma67, nick42d, caos-obliquo)
- ytmapi-rs regression fixes: auth cookies, EP/singles, reqwest 0.13->0.11, VL prefix, RemovePlaylistItems
- Scrobbler: signature fix, persistent cache, rate limiting, 5 new tests
- Album tracks leak: stale split track names bleeding into next song scrobble
- Last.fm canonical album name: 4 bugs fixed, 8 tests
- Gapless advance: fix ID mismatch stopping playback after track 2
- Suckless refactoring: -630 lines (panics, dead crates, boilerplate, method subdivisions)
- Perf batch: render throttle, stale download cancel, enter-spam guard, lazy iterator, protocol cache, help menu single-pass

## Completed (v1.1.0 - feat/next-release-v1.1)
- **genre-db-sqlite**: SQLite-backed GenreDb with MusicBee+RYM seed data, hierarchy propagation
- **MB OAuth2+genres**: device flow, auto-refresh, release group genre fetch, MBID capture
- **LB genre validation**: known-genre tags without mbid promoted to genres
- **Genre+year merge**: provider-weighted voting (MB=3, LB=2, rest=1), cumulative weights
- **MB Cover Art Archive**: fallback pipeline before Last.fm in FetchAlbumArt
- **Queue year enrichment**: EnrichSongYear on queue-add, rate-limited 1/2s, stale-guard
- **CLI tools**: test-musicbrainz, test-caa, test-listenbrainz, test-validate-metadata
- **metadata-cache-sqlite crate**: SQLite metadata cache (+16 tests)
- **LibreFM provider**: new metadata provider (+355 lines)
- **SQLite MBID**: `musicbrainz_release_group_id` column in DDL + PRAGMA user_version migration + put/get/iter
- **LB provider MBID**: `release_group_mbid` extracted in triple destructure
- **SQLite batch flush**: `put_batch()` with explicit transaction, CAA cache wire
- **Instant year enrichment**: cache check inline in GetPlaylistTracks + GetAllLibrarySongs — years appear instantly on Library open
- **CLI metadata-cache**: `--show/--clear/--stats` subcommand
- **CAA cache**: SQLite check before HTTP, save on success/404 (7-day TTL for not-found)
- 20 metadata-cache-sqlite tests (+4: roundtrip mbid, migration, put_batch), 538 total pass, 0 warnings

## Low Priority
- **Native streaming** - symphonia/basic-tcp-streaming prototype
- **Liked songs in browser tables** - parse like_status from search results, add "Liked" column
- **Artist album pagination** - `ParseFromContinuable` for `GetArtistAlbumsQuery`
- **Upstream dep tracking** - `AudioQuality` removal from structures.rs
- **compute_artists_string** - minor perf: cached/footer duplication

## Blocked
- **54 ytmapi-rs integration tests** - YT API format drift (gridRenderer, musicShelfRenderer). Needs network captures.
