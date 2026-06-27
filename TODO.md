# Future TODOs

## Bug Investigation (Blocked)
- **54 integration tests fail** — YT API format drift (missing JSON keys like `gridRenderer/items`, `musicShelfRenderer/contents`). Needs API response reverse-engineering. Blocked on network captures.
- **Artist album pagination** — only first page returned. Needs `ParseFromContinuable` impl for `GetArtistAlbumsQuery`. Significant feature.

## Performance (Minor)
- `compute_artists_string` still duplicated between footer + table per draw. Would need interior mutability on `ListSong` to cache — marginal gain for complexity.

## Dep Tracking
- Upstream removed `AudioQuality` from structures.rs — if they finalize removal, adapt our fork's re-exports.
