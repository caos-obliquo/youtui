# ytmapi-rs Implementation Status

> Last updated: 2026-06-27
> Compared against: upstream ytmusicapi (Python) feature matrix

## Legend

| Icon | Meaning |
|------|---------|
| ✅ | Implemented + tested |
| [x] | Implemented |
| [~] | Partial |
| [ ] | Not implemented |
| SKIP | Intentional - not useful for youtui |

## Auth

| Endpoint | Status | Notes |
|----------|--------|-------|
| OAuth device code | ✅ | `generate_oauth_code_and_url()` |
| OAuth token exchange | ✅ | `generate_oauth_token()` |
| Token refresh | ✅ | `YtMusic<OAuthToken>::refresh_token()` |
| Browser token from cookie | ✅ | `from_cookie()`, `from_cookie_file()` |
| Unauthenticated client | ✅ | `YtMusic::new_unauthenticated()` |
| Language/location params | ✅ | `with_language()`, `with_location()` |

## Search

| Endpoint | Status | Notes |
|----------|--------|-------|
| Basic search | ✅ | `SearchQuery` + all 10 filtered variants |
| Search suggestions | ✅ | `GetSearchSuggestionsQuery` |
| Upload search | ✅ | `SearchQuery::new_uploads()` |
| Library search | ✅ | `SearchQuery::new_library()` |
| Continuations | ✅ | Streaming via `stream_api_with_retry_n` |

## Browse

| Endpoint | Status | Notes |
|----------|--------|-------|
| GetAlbum | ✅ | Full album details + tracks |
| GetArtist | ✅ | Albums + singles + songs + videos + related |
| GetArtistAlbums | ✅ | Full album list via browse params |
| GetUser | ✅ | Public user profile |
| GetUserPlaylists | ✅ | Paginated |
| GetUserVideos | ✅ | Paginated |
| GetChannel | ✅ | Podcast channel |
| GetChannelEpisodes | ✅ | Paginated |
| GetPodcast | [~] | **NO continuations** - single page only |
| GetEpisode | ✅ | |
| GetNewEpisodes | ✅ | |
| GetMoodCategories | ✅ | |
| GetMoodPlaylists | ✅ | |
| GetTasteProfile | ✅ | |
| SetTasteProfile | ✅ | |
| GetSearchSuggestions | ✅ | |

## Library (all require auth)

| Endpoint | Status | Notes |
|----------|--------|-------|
| GetLibraryPlaylists | ✅ | |
| GetLibrarySongs | ✅ | Liked songs - with continuations |
| GetLibraryAlbums | ✅ | With continuations |
| GetLibraryArtists | ✅ | With continuations |
| GetLibraryArtistSubscriptions | ✅ | With continuations |
| GetLibraryPodcasts | ✅ | With continuations |
| GetLibraryChannels | ✅ | With continuations |
| **GetLikedSongs** | [x] | Same as GetLibrarySongs (`FEmusic_liked_videos`) |
| **GetSavedEpisodes** | [ ] | SKIP - podcasts not wired in UI |
| **GetAlbumBrowseId** | [~] | Name→ID resolver added `resolve_album_browse_id()` |
| **GetAccountInfo** | [ ] | SKIP - no UI use case |
| **EditSongLibraryStatus** | ✅ | Add/remove from library |

## Playlist CRUD

| Endpoint | Status | Notes |
|----------|--------|-------|
| GetPlaylistTracks | ✅ | With continuations |
| GetPlaylistDetails | ✅ | |
| CreatePlaylist | ✅ | Basic + from videos + from playlist |
| EditPlaylist | ✅ | Title, description, privacy |
| DeletePlaylist | ✅ | |
| AddPlaylistItems | ✅ | Videos to playlist |
| AddPlaylistItems (playlist copy) | ✅ | Playlist to playlist |
| RemovePlaylistItems | ✅ | |
| VL prefix handling | ✅ | Stripped on mutation queries |

## Playback

| Endpoint | Status | Notes |
|----------|--------|-------|
| GetWatchPlaylist | ✅ | From video ID, playlist ID, or both |
| GetLyricsID | ✅ | |
| GetLyrics | ✅ | |
| GetSongTrackingUrl | ✅ | |

## Ratings & Subscriptions

| Endpoint | Status | Notes |
|----------|--------|-------|
| RateSong | ✅ | Like/dislike/indifferent |
| RatePlaylist | ✅ | |
| SubscribeArtist | ✅ | |
| UnsubscribeArtists | ✅ | |

## History

| Endpoint | Status | Notes |
|----------|--------|-------|
| GetHistory | ✅ | With continuations |
| RemoveHistoryItems | ✅ | |
| AddHistoryItem | ✅ | Via song tracking URL |

## Uploads (all require auth)

| Endpoint | Status | Notes |
|----------|--------|-------|
| UploadSong | ✅ | BrowserToken only |
| GetLibraryUploadSongs | ✅ | With continuations |
| GetLibraryUploadArtists | ✅ | With continuations |
| GetLibraryUploadAlbums | ✅ | With continuations |
| GetLibraryUploadArtist | ✅ | With continuations |
| GetLibraryUploadAlbum | ✅ | |
| DeleteUploadEntity | ✅ | |

## Library Sort Order

| Endpoint | Status | Notes |
|----------|--------|-------|
| GetLibrarySortOrder enum | ✅ | NameAsc, NameDesc, RecentlySaved, Default |
| Sort in query structs | ✅ | All 6 library Query structs accept sort |
| Sort in simplified API | ✅ | 6 methods now accept `Option<GetLibrarySortOrder>` |
| Sort in CLI | ✅ | `--sort` flag for library commands |
| Sort in youtui UI | ✅ | Wired via o.O in Library context menu, cycles Default→NameAsc→NameDesc→RecentlySaved |

## Internal Code Quality

| Item | Status | Notes |
|------|--------|-------|
| Clippy warnings | ✅ | **0 across all 10 crates** (ytmapi-cli removed from workspace) |
| `#[allow(dead_code)]` | [~] | 3 proposital, 0 stale (cleaned in f723535, 206 lines deleted) |
| `unwrap()` in production | ✅ | **0** - all in doc tests/tests |
| Stale TODOs removed | ✅ | **62 removed**, 37 legitimate remain |
| Tests | ✅ | ytmapi-rs: 82/82 lib, 28/52 auth (3 locale tests removed in slimming) |
| ytmapi-cli docs | ✅ | `docs/ytmapi-cli.md` (ytmapi-cli removed from workspace but doc preserved) |

## Remaining ytmapi-rs TODOs (37+ items - LOW priority)

All remaining TODOs are legitimate feature gaps but LOW value for youtui.
Note: the working tree slimming (590d336) added/reverted some TODO annotations (e.g., ArtistTopReleaseCategory enum → `Option<String>`).

| Category | Count | Examples |
|----------|-------|---------|
| Artist categories | 4 | Singles, related, videos within GetArtist |
| i18n | 3 | Locale-dependent filter strings in library |
| VL prefix | 4 | Already handled externally in youtui |
| Continuations | 3 | Podcast, search endpoint extensions |
| Consolidation | 8 | Code movement, helper sharing |
| Library/upload fields | 4 | Author, count fields in library responses |
| Misc feature fields | 11 | Endpoint IDs, search params, menu entries |

---

## Summary

**85% parity** (40/47 endpoints). 

**High-value gaps (all DONE):**
1. ~~Library sort order~~ → wired in youtui UI (b26bb4c)
2. ~~Code trim~~ → dead_code cleanup done (f723535)
3. ~~Doc polish~~ → CLAUDE.md, TODO.md, roadmap udpated

**Low-value gaps (skip):**
- GetSavedEpisodes (podcasts not wired)
- GetAccountInfo (no UI use)
- GetPodcast continuations (podcasts not wired)
- GetSong (full) (not planned by upstream)
