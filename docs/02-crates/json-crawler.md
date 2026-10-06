# Crate: json-crawler

**1,089 LOC, 3 files** - Wrapper for serde_json that provides nice errors when traversing large JSON blobs.

## Module Tree

```
src/
├── lib.rs     - JsonCrawler trait, JsonCrawlerOwned, JsonCrawlerBorrowed, JsonPath
├── error.rs   - CrawlerError with path tracking, CrawlerResult alias
└── iter.rs    - JsonCrawlerIterator, array into-iter / iter-mut types
```

## Purpose

YTM API returns massive nested JSON responses (thousands of lines). `json-crawler` provides chainable navigation methods on an owned or borrowed `serde_json::Value` that track the path for error messages:

```rust
use json_crawler::JsonCrawlerOwned;

let mut crawler = JsonCrawlerOwned::new("source".to_string(), api_response);
let title: String = crawler.take_value_pointer("contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/title/runs/0/text")?;
```

On error: `CrawlerError { path: "contents/...tabs[0]...", ... }` with the JSON-pointer-style path, expected parse target, and source snippet.

## Key API

```rust
pub trait JsonCrawler: Sized {
    fn navigate_pointer(self, new_path: impl AsRef<str>) -> CrawlerResult<Self>;
    fn navigate_index(self, index: usize) -> CrawlerResult<Self>;
    fn borrow_pointer(&mut self, path: impl AsRef<str>) -> CrawlerResult<Self::BorrowTo<'_>>;
    fn borrow_index(&mut self, index: usize) -> CrawlerResult<Self::BorrowTo<'_>>;
    fn take_value<T: DeserializeOwned>(&mut self) -> CrawlerResult<T>;
    fn take_value_pointer<T: DeserializeOwned>(&mut self, path: impl AsRef<str>) -> CrawlerResult<T>;
    fn borrow_value<T>(&self) -> CrawlerResult<T>;
    fn borrow_value_pointer<T>(&self, path: impl AsRef<str>) -> CrawlerResult<T>;
    fn take_value_pointers<T, S: AsRef<str>>(&mut self, paths: &[S]) -> CrawlerResult<T>;
    fn take_and_parse_str<F: FromStr>(&mut self) -> CrawlerResult<F>;
}

pub struct JsonCrawlerOwned { ... }       // owns the serde_json::Value
pub struct JsonCrawlerBorrowed<'a> { ... } // borrows from the Value

pub enum JsonPath { ... } // pointer-style path builder

pub struct CrawlerError { ... } // path + parse target + source context
pub type CrawlerResult<T> = Result<T, CrawlerError>;
```

## Streaming Iteration

`iter.rs` provides `JsonCrawlerIterator` plus `JsonCrawlerArrayIntoIter` / `JsonCrawlerArrayIterMut` for stepping through JSON arrays one item at a time - useful for paginated responses. Consume via `try_into_iter()` / `try_iter_mut()` on the `JsonCrawler` trait.

## Tests

```bash
cargo test --release -p json-crawler
# 2 tests pass (0 lib + 2 doctests)
```
