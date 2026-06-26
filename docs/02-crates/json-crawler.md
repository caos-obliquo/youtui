# Crate: json-crawler

**1,089 LOC, 3 files** — Wrapper for serde_json that provides nice errors when traversing large JSON blobs.

## Module Tree

```
src/
├── lib.rs     — Crawlable trait, JsonCrawler struct, From impls
├── error.rs   — CrawlError with path tracking
└── iter.rs    — Streaming array iteration
```

## Purpose

YTM API returns massive nested JSON responses (thousands of lines). `json-crawler` provides chainable query methods that track the path for error messages:

```rust
use json_crawler::Crawlable;

let json: serde_json::Value = api_response;
let title = json
    .crawl("contents")?
    .crawl("singleColumnBrowseResultsRenderer")?
    .crawl("tabs")?
    .index(0)?
    .crawl("tabRenderer")?
    .crawl("content")?
    .crawl("sectionListRenderer")?
    .crawl("contents")?
    .index(0)?
    .crawl("musicPlaylistShelfRenderer")?
    .crawl("title")?
    .crawl("runs")?
    .index(0)?
    .crawl("text")?
    .as_str()?;
```

On error: `CrawlError { path: "contents.singleColumnBrowseResultsRenderer.tabs[0].tabRenderer.content...", message: "expected array", value: ... }`

## Key API

```rust
pub trait Crawlable {
    fn crawl(self, key: &str) -> Result<Self>;
    fn index(self, idx: usize) -> Result<Self>;
}

impl Crawlable for serde_json::Value { ... }

pub struct JsonCrawler<'a> { ... }

pub struct CrawlError {
    pub path: String,   // e.g., "contents.tabs[0].tabRenderer"
    pub message: String,
    pub value: Box<serde_json::Value>,
}
```

## Streaming Iteration

```rust
pub fn json_array_iter<'a>(value: &'a serde_json::Value) -> impl Iterator<Item = &'a serde_json::Value>;
```

Returns items one at a time without materializing the entire array — useful for paginated responses.

## Tests

```bash
cargo test --release -p json-crawler
# 2 tests pass (0 lib + 2 doctests)
```
