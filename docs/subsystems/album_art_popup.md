# Album Art Popup (o.v) — Sixel System

## Quick Summary

`o.v` opens full-screen album art via sixel graphics. h/l cycles through all album arts in queue. Esc/q closes.

## Layout

- Full-terminal overlay via `Clear` widget + early return in `draw_app()` (no main window/footer drawn behind popup)
- Centered rect: `Rect::inner(&Margin { vertical: h/6, horizontal: w/8 })` — symmetric centering with proportional margins
- Min-size guard: skip drawing if rect < 4x4
- Image scaled with `Resize::Fit(None)` — fits within pixel area while preserving aspect ratio

## Sixel Centering Fix (af0acb8) — Root Cause + Resolution

### The Problem
Album art appeared "too up and too left" in the popup. Far from centered.

### Investigation Path

1. **Suspected Layout issue**: First thought 3/94/3% Layout was asymmetrical in small terminals. Replaced with `Rect::inner(&Margin{vertical: h/6, horizontal: w/8})` — cleaner but didn't fix.
2. **Suspected Block interference**: Tried wrapping image in a bordered Block. Image appeared at top-left of Block instead of inside. Block approach was wrong.
3. **Root cause found**: `Resize::Fit(None)` computes fitted pixel dimensions from the image's aspect ratio vs target pixel area. Fitted image may be SMALLER than the target rect in one dimension (e.g., tall portrait image in wide rect → fitted height = target height, fitted width < target width). Without centering offset, image rendered at rect's top-left corner.

### The Fix

After `new_protocol()` succeeds, read the fitted dimensions via `Protocol::area()`:

```rust
if let Ok(protocol) = terminal_image_capabilities.new_protocol(
    popup.current_thumbnail().in_mem_image.clone(),
    centered,
    Resize::Fit(None),
) {
    // Get fitted area AFTER protocol creation
    let fitted = protocol.area();
    // Compute centering offset
    let img_rect = Rect {
        x: centered.x + (centered.width.saturating_sub(fitted.width)) / 2,
        y: centered.y + (centered.height.saturating_sub(fitted.height)) / 2,
        width: fitted.width.min(centered.width),
        height: fitted.height.min(centered.height),
    };
    // Render at offset rect
    f.render_widget(Image::new(&protocol), img_rect);
}
```

Key insight: `new_protocol()` returns the protocol with fitted dimensions already computed. But the caller must read `protocol.area()` to know what those dimensions are. Render `Image` at the computed offset rect, NOT the original `centered` rect.

## Sixel Persistence / Corruption — Fix

### The Problem
When closing album art popup, sixel pixels remained on screen (showed through normal UI text). `\x1bP0p\x1b\\` DCS clear was unreliable in foot terminal.

### Solution (belt-and-suspenders)

1. **DCS clear at draw start**: Every call to `draw_app()` sends `\x1bP0p\x1b\\` (Delete All Sixel Graphics) + `flush()` before any ratatui rendering.
2. **State tracking**: `w.sixel_data: Option<Vec<u8>>` + `w.sixel_rect: Option<Rect>` track live sixel. Reset at draw start, repopulated by footer or album art popup.
3. **ANSI clear + DCS on close**: `ClosePopup` handler sends `\x1bP0p\x1b\\` + `\x1b[2J\x1b[H` directly to stdout.
4. **Normal draw overwrites**: After close, next `draw_app()` draws normal UI over the cleared area. Even if DCS clear partially failed, the blank background cells cover text-layer remnants.

## Pagination (4b35726)

`AlbumArtPopup` struct:
```rust
pub struct AlbumArtPopup {
    pub thumbnails: Vec<Rc<SongThumbnail>>,
    pub index: usize,
}
```

- Methods: `current_thumbnail()` (returns `&Rc<SongThumbnail>`), `total()` (returns `usize`)
- h/Left: `index = (index + total - 1) % total` (wrap-around)
- l/Right: `index = (index + 1) % total` (wrap-around)
- Esc/q: `AppCallback::ClosePopup`
- Page indicator: `"N / M"` rendered at bottom center when `total() > 1`

## Thumbnail Collection (ViewAlbumCover handler in app.rs)

```rust
AppCallback::ViewAlbumCover => {
    // Collect ALL downloaded album arts from queue
    let all_thumbs: Vec<Rc<SongThumbnail>> = self.window_state.playlist
        .song_list.get_list_iter()
        .filter_map(|s| match &s.album_art {
            AlbumArtState::Downloaded(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    // Find selected song's art index via Rc::ptr_eq
    let idx = selected_thumb.as_ref()
        .and_then(|sel| all_thumbs.iter().position(|t| Rc::ptr_eq(t, sel)))
        .unwrap_or(0);
    self.window_state.album_art_popup = Some(AlbumArtPopup::new(all_thumbs, idx));
}
```

Key: `Rc::ptr_eq` compares by pointer identity (same downloaded thumbnail object). Falls back to index 0 if not found.

## Lifecycle Flow

```
Key o.v in queue
  → PlaylistAction::ViewAlbumCover
    → AppCallback::ViewAlbumCover
      → collect all downloaded thumbs from queue
      → find selected song's thumb index via Rc::ptr_eq
      → set album_art_popup = Some(AlbumArtPopup { thumbnails, index })

draw_app():
  → DCS clear + flush
  → reset sixel_data/sixel_rect to None
  → if album_art_popup.is_some():
    → render Clear behind popup
    → compute centered rect
    → new_protocol(image, rect, Fit(None))
    → protocol.area() for fitted dims
    → compute centering offset
    → render Image at offset rect
    → store sixel_data + sixel_rect
    → return early (skip main UI)

Key h/l/Left/Right:
  → album_art_popup.handle_key()
    → cycle index
    → no AppCallback (next draw picks up new thumb)

Key Esc/q:
  → album_art_popup.handle_key()
    → AppCallback::ClosePopup
      → album_art_popup = None
      → DCS clear + ANSI clear to stdout
      → next draw_app() renders normal UI
```

## Key Files

| File | Purpose |
|---|---|
| `app/ui/playlist/album_art_popup.rs` | AlbumArtPopup struct, key handler |
| `app/view/draw.rs` | draw_app: sixel clear, centering, render, pagination |
| `app.rs` | ViewAlbumCover, ClosePopup sixel clear |
