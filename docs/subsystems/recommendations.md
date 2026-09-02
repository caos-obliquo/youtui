# Subsystem: Recommendations (F4 + CLI)

## Quick Summary

`F4` opens the Recommendations popup: Last.fm recs synthesized from the user's top scrobbles via `getSimilar` discovery, cached in SQLite, plus a niche mode that favors low-listener artists. A parallel ListenBrainz collaborative-filtering CLI covers the LB side. Songs discovered in the popup can be queued, played, or opened in the Song Info popup (with RYM genre descriptions).

## Background

Last.fm's `user.getRecommended*` endpoints are REMOVED from the public catalog (return error 3, "No method with that name in this package"). Recs are therefore synthesized from still-public discovery endpoints:

- Seeds: `user.getTop{Track,Album,Artist}` (scrobble history)
- Similarity: `track.getSimilar` / `artist.getSimilar`
- `album.getSimilar` does not exist, so Albums use `artist.getSimilar(album.artist)`

## CLI

### `youtui recommendations`

```
youtui recommendations [--type all|tracks|albums|artists] [--limit 20] [--page 1]
    [--niche-level 0.7] [--seed-count 35] [--seed <artist|artist - title>]
    [--similar-limit 10] [--json]
```

- `--type` kind to recommend (default `all`); `--limit` global cap; `--page` `getTop*` page
- `--niche-level` A+B weighting (0 = naive/backward-compat); `--seed-count` seeds per kind
- `--seed` starts discovery from a specific artist (or `artist - title`) via `fetch_recommendations_for_seed`; graceful artist fallback when the seed artist yields no similar results
- `--similar-limit` caps per-seed similar results
- `--json` machine-readable output
- Missing session_key -> stderr + non-zero exit
- Timeouts: `with_timeout` (30s naive) / `NICHE_TIMEOUT` (600s niche)

### `youtui listenbrainz-recommendations`

```
youtui listenbrainz-recommendations [--artist-type top|similar|raw] [--json]
```

- LB collaborative filtering from `1/cf/recommendation/user/{u}/recording?artist_type=...`
- HTTP 204 (recs not ready; LB nightly batch) is NOT an error: `fetch_listenbrainz_recommendations` falls back to `synthesize_listenbrainz_recommendations`, which walks the LB listens corpus -> top artists -> `artist.getSimilar` -> 20 artist recs

## Niche Engine (A+B)

Niche score = `match_score*(1-niche) + niche_favor(listeners)*niche`

`niche_favor(listeners)`:

| Listeners | Favor |
|-----------|-------|
| <= 50k | 1.0 |
| <= 200k | 0.7 |
| <= 1M | 0.4 |
| else | 0.1 |

- Seeds from `getTop*` (matching kind)
- Filters out already-scrobbled / seed artists
- Caps ~3 recs per seed + global `--limit`
- Dedups by name / MBID
- Appends `[niche]` suffix when `niche_level >= 0.5`
- Every rec prints `Similar to: <reason>` (synthesized from the seed source)

## Performance

`fetch_niche_recommendations` (L479) uses `buffer_unordered(8)` on both the per-seed `getSimilar` fan-out and the per-candidate `fetch_artist_listeners` getInfo fan-out. Cuts ~90s -> ~12s. The niche path uses `NICHE_TIMEOUT` (600s) via `with_timeout_dur`; the ~140-round-trip fan-out exceeds the default 30s.

`FetchAllRecommendations` (messages.rs) runs `fetch_niche_recommendations` for all 3 kinds concurrently with `buffer_unordered(3)`. `FetchNicheRecommendations` (single kind) remains but is `#[allow(dead_code)]`.

## SQLite Persistent Cache

File: `youtui/src/recommendations_store.rs`

`RecommendationStore` uses rusqlite 0.31 (bundled):

- `open_default()` -> `get_data_dir()/recommendations_cache.db` (`~/.local/share/youtui/`)
- `open(path)` explicit path
- `load(cache_key)` returns items only when <24h TTL fresh
- `save(cache_key, &[RecItem])` stores serde_json blob + fetched_at epoch
- `clear(cache_key)` removes an entry
- TTL: `TTL_SECS = 24*60*60`

Model: lastfm-homepage daily rotation. F4 recs persist across app restarts within 24h. `r` reloads (re-fetch); `--seed` does not refresh the cache.

## F4 Recommendations Popup

File: `youtui/src/app/ui/playlist/recommendations_popup.rs`

Open via Global `F(4)` -> `AppAction::Recommend` (keymap.rs). Header shows the `Recs` F4 button (header.rs); browser tabs hide while the popup is open. `open_recommendations()` (ui.rs) checks the persistent store, then the in-memory cache (`recommendations_cache: Option<(Instant, Vec<RecItem>)>`), then fetches.

Struct: `RecommendationsPopup { kind, kind_filter, items, selected, scroll_offset, loading, filter, filter_active, menu_open, menu_selected, table_state, tick }`

| Key | Action |
|-----|--------|
| q / Esc | Close |
| j / Down, k / Up | Move |
| C-g | Top |
| G | Bottom |
| `/` | Filter toggle |
| Backspace / Char | Filter edit |
| Tab | Kind cycle None -> Artists -> Albums -> Tracks -> None |
| Enter / o | Act (o opens context menu) |
| r | Reload (re-fetch, bypass cache) |

- `draw` (L364): AdvancedTableView columns `#/Type/Artist/Name/Similar To/List`
- `menu_items` (L99): per-kind Play / Add to Queue / Copy URL / Song Info / Go to Artist / [Go to Album] [Go to Track]
- `draw_menu` (L491): bottom-left Context Menu
- `set_items` (L356): populates items

### Actions

- **Enter / Play**: `ActOnRecommendation(index, kind, title, artist, cfg)` -> `SearchSongs` -> first result -> queue + play (`HandleActOnRecommendationOk`)
- **Song Info**: `AppCallback::ViewRecSongInfo(usize, RecKind, String, String)` (app.rs L193) resolves the rec via `SearchSongs` -> `ListSong` -> `open_song_info_popup` (fires RYM genre description lines)
- **Go to Artist / Album / Track**: `AppCallback::Navigate(NavTarget)` closes the popup before navigating
- **Reload**: `AppCallback::ReloadRecommendations`

Handlers (`effect_handlers_playlist.rs`): `HandleRecommendationsOk/Err` writes `recommendations_store` + `recommendations_cache`; `HandleActOnRecommendationOk/Err` queue + play; `HandleRecSongInfoOk/Err` opens the Song Info popup.

## Key Files

| File | Purpose |
|------|---------|
| `youtui/src/lastfm_recommend.rs` | All rec logic: `lastfm_get` (signed GET, `format=json` excluded from signature), `fetch_top_items`, `fetch_similar_for_source`, `fetch_recommendations` (L313), `fetch_recommendations_for_seed` (L334), `fetch_artist_listeners`, `fetch_artist_top_album`, `niche_favor`, `fetch_niche_recommendations` (L479), `niche_suffix`, `fetch_listenbrainz_recommendations` (L734), `synthesize_listenbrainz_recommendations` (L877), `print_recommendations` (L622), `print_listenbrainz_recommendations` (L956), `print_listenbrainz_recommendations_json` (L977), `print_recommendations_json` (L993) |
| `youtui/src/recommendations_store.rs` | `RecommendationStore` SQLite cache (24h TTL) |
| `youtui/src/app/ui/playlist/recommendations_popup.rs` | `RecommendationsPopup` F4 popup, `handle_key`, `set_items`, `draw`, `menu_items`, `activate_menu_item`, `draw_menu` |
| `youtui/src/app/server/messages.rs` | `FetchAllRecommendations`, `FetchNicheRecommendations`, `ActOnRecommendation`, `SearchSongs` |
| `youtui/src/app.rs` | `ReloadRecommendations`, `ActOnRecommendation`, `ViewRecSongInfo` (L193), `Navigate` |
| `youtui/src/app/ui/playlist/effect_handlers_playlist.rs` | `HandleRecommendationsOk/Err`, `HandleActOnRecommendationOk/Err`, `HandleRecSongInfoOk/Err` |
| `youtui/src/app/ui.rs` | `recommendations_store` + `recommendations_cache` fields, `open_recommendations()`, `close_popup()` |
| `youtui/src/app/ui/header.rs` | F4 `Recs` button; browser tabs hidden while popup open; F3 label `Queue` |
| `youtui/src/config/keymap.rs` | `F(4) -> AppAction::Recommend` (Global) |

## Related

- [docs/09-roadmap.md](../09-roadmap.md) - shipped feature list
- [docs/06-subsystems/scrobbling.md](../06-subsystems/scrobbling.md) - `submit_to_listenbrainz`, LB backfill, LB recs CLI
- [docs/06-subsystems/validation.md](../06-subsystems/validation.md) - genre pipeline fix
- [TODO.md](../../TODO.md) - completed catalog

## Tests

- 6 fixture-parse tests: parse_top_tracks/albums/artists, parse_similar_track/artist_with_reason, `sign_lastfm` known-vector
- LB parse tests for the listenbrainz endpoints
- youtui: 194 pass, 0 fail, 4 ignored
