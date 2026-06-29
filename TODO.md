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

## Next (User Priority Queue)
- **SQLite metadata cache crate** - cache metadata results to reduce API calls
- **MusicBrainz Cover Art Archive** - wire album art into footer/art popup
- **Wire SQLite cache into metadata-provider**
- **Plan trim** - remove dead items from robustness plan, align with fork reality

## Low Priority
- **OAuth refresh** - token expiry handling in ytmapi-rs
- **Native streaming** - symphonia/basic-tcp-streaming prototype
- **Liked songs in browser tables** - parse like_status from search results, add "Liked" column
- **Artist album pagination** - `ParseFromContinuable` for `GetArtistAlbumsQuery`
- **Upstream dep tracking** - `AudioQuality` removal from structures.rs
- **compute_artists_string** - minor perf: cached/footer duplication

## Blocked
- **54 ytmapi-rs integration tests** - YT API format drift (gridRenderer, musicShelfRenderer). Needs network captures.
