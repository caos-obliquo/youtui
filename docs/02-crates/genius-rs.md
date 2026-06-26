# genius-rs

Genius.com API client with HTML page scraping for lyrics and annotations.

## Purpose

- Search Genius for songs (Bearer token API + public fallback)
- Scrape song page HTML for lyrics (avoids 403 on lyrics API)
- Extract all annotations from page's `__INITIAL_STATE__` JSON (no pagination limit, no token needed)
- API-based annotation fetching via `/referents` endpoint (requires `GENIUS_TOKEN`)

## Architecture

| Module | Function | Method |
|--------|----------|--------|
| `search.rs` | `search()` → `Vec<SongHit>` | `GET api.genius.com/search` (Bearer) or `GET genius.com/api/search/song` (public) |
| `scrape.rs` | `fetch_lyrics()` → `String` | `GET genius.com{path}` → parse `<div data-lyrics-container>` |
| `scrape.rs` | `fetch_annotations()` → `Vec<Annotation>` | Parse `window.__INITIAL_STATE__` JSON from page |
| `annotations.rs` | `fetch_from_api()` → `Vec<Annotation>` | `GET api.genius.com/referents?song_id={id}` with Bearer token |
| `lib.rs` | `GeniusClient` | High-level API combining search + scrape + API |

## API

```rust
use genius_rs::GeniusClient;

let client = GeniusClient::with_default_client(Some(token));

// Search → get song ID + URL path (slug URL first, then API search)
let hit = client.find_song("FIDLAR", "Wasted").await?;

// Fetch lyrics (scraped from HTML)
let lyrics = client.fetch_lyrics("/Fidlar-wasted-lyrics").await?;

// Fetch annotations (page scrape — only works if __INITIAL_STATE__ present)
let annotations = client.fetch_annotations("/Fidlar-wasted-lyrics").await?;

// Fetch annotations with API fallback (tries API first if token available, then page scrape)
let annotations = client.fetch_annotations_with_token("/path", song_id).await?;

// Both at once (slug URL → lyrics, API search → real song ID for annotations)
let (hit, lyrics, annotations) = client.find_fetch_all("FIDLAR", "Wasted").await?;

// Only lyrics (slug URL first, then API search)
let (hit, lyrics) = client.find_and_fetch("FIDLAR", "Wasted").await?;

// Compute slug URL without API call
let path = genius_rs::search::compute_path("FIDLAR", "Wasted");
// → "/fidlar-wasted-lyrics"
```

## CLI

```bash
# Fetch lyrics (slug URL first, then API search)
cargo run --bin genius-rs fetch "FIDLAR" "Wasted"

# Fetch lyrics + all annotations
cargo run --bin genius-rs all "FIDLAR" "Wasted"

# Search for song info
cargo run --bin genius-rs search "Beatles" "Here Comes The Sun"

# Compute slug URL only (no API call)
cargo run --bin genius-rs slug "Love Letter" "Love Letter"
# → /love-letter-love-letter-lyrics
```

### CLI Options

| Flag | Description |
|------|-------------|
| `--json` | Machine-readable JSON output |
| `--fixture <dir>` | Save parsed lyrics/annotations to directory |
| `--verbose` | Show debug logs (tracing output) |
| `--raw-html` | Print raw HTML of the Genius page (debug) |

### CLI Examples

```bash
# JSON output for piping/parsing
genius-rs fetch "FIDLAR" "Wasted" --json

# Save fixture files for test snapshots
genius-rs all "Queen" "Bohemian Rhapsody" --fixture ./test_fixtures/

# Debug: dump raw HTML to find annotation IDs
genius-rs fetch "FIDLAR" "Wasted" --raw-html | head -100

# Debug: see what the scraper is doing
genius-rs fetch "100 gecs" "money machine" --verbose
```

## Lyrics Scraping

The Genius lyrics API endpoint (`/api/songs/{id}/lyrics`) returns **403 Forbidden**. We scrape the public song page HTML instead:

1. Compute slug URL first: `https://genius.com/{artist-slug}-{title-slug}-lyrics`
2. If slug URL fails, search Genius API for the song → get canonical path
3. Fetch the public song page HTML
4. Find `<div data-lyrics-container="true">` with CSS selector
5. Walk DOM: extract text from all child nodes, preserving `<br>` as newlines
6. `<a>`, `<i>`, `<b>`, `<span>` tags are recursed into (span was missing — fixed bug)
7. Section headers like `[Verse 1]`, `[Chorus]` are plain text — preserved verbatim
8. Blank lines inserted BETWEEN sections (not after headers): `[Verse 1]\ntext\n\n[Chorus]\ntext`
9. Clean HTML entities, strip junk lines ("You might also like", "Contributors", etc.)

### Slug URL Fallback

When search API returns wrong results (e.g., artist=title="Love Letter" was finding wrong song), we try the computed slug URL FIRST before any API call:

```rust
let path = search::compute_path("Love Letter", "Love Letter");
// → "/love-letter-love-letter-lyrics"
// Fetches https://genius.com/love-letter-love-letter-lyrics directly
// No API call needed — works even with ambiguous search queries
```

## Annotations

Three-tier fallback for annotations (in order):

1. **API with Bearer token** (`annotations.rs:fetch_from_api()`):
   - Calls `https://api.genius.com/referents?song_id={id}` with `Authorization: Bearer {token}`
   - Returns ALL annotations for the song (no pagination limit)
   - **Requires `GENIUS_TOKEN` env var** — set in your shell profile
   - Most reliable method

2. **Page scrape** (`scrape.rs:extract_annotations()`):
   - Parses `window.__INITIAL_STATE__` JSON embedded in the page
   - Returns all annotations from the JSON
   - **Only works on modern Genius pages** — many pages don't embed this JSON

3. **Empty result** — if both fail, returns empty vec with warning

```bash
# With token (reliable):
GENIUS_TOKEN=your_token_here cargo run --bin genius-rs all "Queen" "Bohemian Rhapsody"

# Without token (unreliable — depends on page structure):
cargo run --bin genius-rs all "FIDLAR" "Wasted"
```

### Why annotations fail without token

The Genius API endpoint `/referents` requires authentication. The page-embedded `__INITIAL_STATE__` JSON is only present on newer Genius pages served to logged-in users or modern browser requests. Our `reqwest` client with a default user-agent may not get this JSON. **Setting `GENIUS_TOKEN` in your environment is the only reliable way to get annotations.**

## Section Spacing

Lyrics format with proper section spacing:

```
[Verse 1]
I'm out of town, you're out of luck
Let's get 68 more beers in the back of my truck

[Chorus]
I'm wasted
So wasted
```

Blank line BETWEEN sections (not after header). This is controlled in `scrape.rs` `clean_lyrics()`:
- When a line starts with `[` and ends with `]`, a blank line is inserted BEFORE it (unless it's the first line)
- This creates visual separation between song sections

## Known Issues

| Issue | Cause | Workaround |
|-------|-------|------------|
| Annotations return 0 | No `GENIUS_TOKEN` set | Add `GENIUS_TOKEN` env var |
| `__INITIAL_STATE__` missing | Old/legacy Genius pages | Use Bearer token with `fetch_annotations_with_token()` |
| Lyrics truncated | `<span>` tags inside `<a>` | Fixed — `span` added to recursive tag handler |
| Wrong song matched | Ambiguous artist/title (e.g., "Love Letter") | Fixed — slug URL tried first before API search |

## Deps

- `reqwest` — HTTP client
- `scraper` — HTML parsing (CSS selectors)
- `serde_json` — JSON parsing for search API + initial state
- `tokio` — async runtime
- `tracing` + `tracing-subscriber` — logging

## Tests

```bash
cargo test --release -p genius-rs
# 18 tests: lyrics extraction, annotations, HTML entities, edge cases, DOM text extraction
```
