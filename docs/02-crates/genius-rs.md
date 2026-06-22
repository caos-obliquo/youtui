# genius-rs

Genius.com API client with HTML page scraping for lyrics and annotations.

## Purpose

- Search Genius for songs (Bearer token API + public fallback)
- Scrape song page HTML for lyrics (avoids 403 on lyrics API)
- Extract all annotations from page's `__INITIAL_STATE__` JSON (no pagination limit, no token needed)

## API

```rust
use genius_rs::GeniusClient;

let client = GeniusClient::with_default_client(Some(token));

// Search → get song ID + URL path
let hit = client.find_song("FIDLAR", "Wasted").await?;

// Fetch lyrics (scraped from HTML)
let lyrics = client.fetch_lyrics("/Fidlar-wasted-lyrics").await?;

// Fetch all annotations (from embedded page JSON)
let annotations = client.fetch_annotations("/Fidlar-wasted-lyrics").await?;

// Both at once
let (hit, lyrics, annotations) = client.find_fetch_all("FIDLAR", "Wasted").await?;
```

## CLI

```bash
# Fetch lyrics
cargo run --bin genius fetch "FIDLAR" "Wasted"

# Fetch lyrics + all annotations
cargo run --bin genius all "FIDLAR" "Wasted"

# Search
cargo run --bin genius search "FIDLAR" "Wasted"
```

## Architecture

| Module | Function | Method |
|--------|----------|--------|
| `search.rs` | `search()` → `Vec<SongHit>` | `GET api.genius.com/search` (Bearer) or `GET genius.com/api/search/song` (public) |
| `scrape.rs` | `fetch_lyrics()` → `String` | `GET genius.com{path}` → parse `<div data-lyrics-container>` |
| `scrape.rs` | `fetch_annotations()` → `Vec<Annotation>` | Parse `window.__INITIAL_STATE__` JSON from page |
| `lib.rs` | `GeniusClient` | High-level API combining search + scrape |

## Lyrics Scraping

The Genius lyrics API endpoint (`/api/songs/{id}/lyrics`) returns **403 Forbidden**. We scrape the public song page HTML instead:

1. Fetch `https://genius.com{path}` (e.g., `/Fidlar-wasted-lyrics`)
2. Find `<div data-lyrics-container="true">` with CSS selector
3. Extract text, preserving `<br>` as newlines
4. Strip `<a>` annotation links but keep their text
5. Section headers like `[Verse 1]`, `[Chorus]` are plain text in the container — preserved verbatim
6. Clean HTML entities

## Annotations

Annotations are extracted from the page's embedded `window.__INITIAL_STATE__` JSON data. This gives **all** annotations for the song without any API token or pagination limit.

Each annotation returns `(fragment: String, body: String)` where `fragment` is the lyric snippet and `body` is the explanation text.

## Deps

- `reqwest` — HTTP client
- `scraper` — HTML parsing (CSS selectors)
- `serde_json` — JSON parsing for search API + initial state
- `tokio` — async runtime
- `tracing` — logging

## Tests

```bash
cargo test --release -p genius-rs
# 10 tests: lyrics extraction, annotations, HTML entities, edge cases
```
