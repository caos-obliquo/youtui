# Fix: Genius Lyrics Truncation

## Root Cause

`messages.rs:592` — Genius HTML scraper uses `inside.split("</div>").next()` which takes only the content BEFORE the first inner `</div>`. Genius wraps each lyric line in nested `<div>` elements, so the scraper captures only the first partial line.

For "Psycho Killer — Talking Heads", only the first line is captured out of ~60 lines of lyrics.

## Fix

Replace the naive `split("</div>").next()` with a proper depth-counting approach that finds the matching OUTER `</div>`:

**Current code** (line 588-605):
```rust
for part in html.split("data-lyrics-container=\"true\"").skip(1) {
    if let Some(start) = part.find(">") {
        let inside = &part[start + 1..];
        let content = inside.split("</div>").next().unwrap_or(inside);
        // strip HTML tags...
    }
}
```

**New code**:
```rust
for part in html.split("data-lyrics-container=\"true\"").skip(1) {
    if let Some(start) = part.find(">") {
        let inside = &part[start + 1..];
        // Find matching outer </div> by tracking nested depth
        let mut depth = 1usize;
        let mut end = 0;
        for (i, _) in inside.char_indices() {
            if inside[i..].starts_with("</div>") {
                depth -= 1;
                if depth == 0 { end = i; break; }
            } else if inside[i..].starts_with("<div ") || inside[i..].starts_with("<div>") {
                depth += 1;
            }
        }
        let content = if end > 0 { &inside[..end] } else { inside };
        // strip HTML tags from content...
    }
}
```

## Safety

- Handles nested `<div>` correctly
- Falls back to full `inside` string if no outer `</div>` found
- Same HTML tag stripping logic continues to work unchanged

## File

`app/server/messages.rs:588-605`
**Lines**: ~15 changed (replacing 5 lines with 15)

## Est. Time

~10 min
