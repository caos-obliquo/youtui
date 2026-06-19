# Final Polish Plan — 4 Items

## Item 1: Esc to Exit `/` Search + Popup Search Fix (10 min)

**Esc in library browser search**: Currently `TextHandler` returns `None` for Esc, which should fall through to keybinds → `BrowserSearchAction::Close` → `handle_toggle_search()`. Needs verification.

**`/` in PlaylistUpdatePopup (`E`)**: Add:
- Search text field, active flag, filtered indices
- `/` toggles search, `Esc`/`Enter` closes search
- Char input/Backspace during search
- Draw search term in title, filter playlist list
- MoveUp/MoveDown/Select use filtered indices

**File**: `playlist_update_popup.rs` — struct fields, `handle_key`, `draw`

---

## Item 2: Nerd Font Icons in Action `describe()` (15 min)

Add Nerd Font icons to action descriptions across:
- `playlist.rs` — all `PlaylistAction` variants ✅ DONE
- `songsearch.rs` — `BrowserSongsAction`
- `artistsearch/songs_panel.rs` — `BrowserArtistSongsAction`
- `playlistsearch/songs_panel.rs` — `BrowserPlaylistSongsAction`
- `library.rs` — already uses `BrowserSongsAction`
- `action.rs` — global actions (SeekForward/Back, VolUp/Down, etc.)

Icons to use:
```
󰐊 = Play         = Delete     󰒝 = Shuffle
󰍉 = Search       = Save       󰑐 = Load
󰋼 = Lyrics      󰖟 = Copy URL   󰋑 = Heart (like)
󰓇 = Artist      󰀥 = Album      󰋲 = Info/Quality
󰄸/󰄷 = Next/Prev 󰑩 = Repeat    󰓻 = Radio
󰩍 = Undo        󰂭 = Visual     󰘐 = Romaji
󰇵 = Filter       = Arrow       = Folder/Playlist
```

---

## Item 3: Album Art Quality in Footer (30 min)

**Problem**: Footer shows blank space for most songs because `AlbumArtState::Downloaded` is only set for album-split songs. YTM thumbnails are not used as fallback.

**Fix**: 
1. In `footer.rs`, when `AlbumArtState::Downloaded` is not set, use upgraded YTM thumbnail URL as fallback
2. In `playlist.rs`, trigger `FetchAlbumArt` when a song starts playing and has album metadata

**Challenges**:
- `ratatui_image` needs `DynamicImage` from disk cache, not URLs
- `FetchAlbumArt` queries Last.fm (rate limited)
- Upgrade URL to 600x600 via existing `upgrade_thumbnail_url()`

---

## Item 4: Annotation Display Polish (20 min)

**Current**: Annotations show as plain text in the right panel:
```
── fragment
   explanation line 1
   explanation line 2

── next fragment
   next explanation
```

**Desired**:
- Fragment text in **italic** (matches Genius style — the annotated lyric snippet)
- Explanation text in normal weight
- Better visual separation between annotations (more spacing or a divider line)
- Proper quote formatting for dialog-style annotations (e.g., `"dialogue"` marks for spoken word)

**Changes in `lyrics_popup.rs`**:

The annotation rendering at lines 228-238:
```rust
let ann_text: String = self.annotations.iter()
    .flat_map(|a| {
        let mut lines = vec![format!("  ── {}", a.fragment)];
        for line in a.explanation.split('\n') {
            lines.push(format!("     {}", line));
        }
        lines.push(String::new());
        lines
    })
    .collect::<Vec<_>>()
    .join("\n");
```

Change to:
```rust
let ann_text: Vec<Line> = self.annotations.iter()
    .flat_map(|a| {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("  ── ", Style::default()),
                Span::styled(&a.fragment, Style::default().add_modifier(Modifier::ITALIC)),
            ]),
        ];
        for line in a.explanation.split('\n') {
            lines.push(Line::from(Span::raw(format!("     {}", line))));
        }
        lines.push(Line::from(""));
        lines
    })
    .collect();
```

Then render using `Paragraph::new(ann_text)` instead of string-based rendering.

This uses ratatui's `Line`/`Span` for styled rendering — italic fragment, normal explanation.

**File**: `lyrics_popup.rs:228-238` — ~15 lines

---

## Execution Order

| # | Item | Est. | Files |
|---|------|------|-------|
| 1 | Esc exit `/` + popup search | 10min | `playlist_update_popup.rs`, `keymap.rs` |
| 2 | Nerd Font icons | 15min | `songsearch.rs`, `artistsearch/songs_panel.rs`, `playlistsearch/songs_panel.rs`, `action.rs` |
| 3 | Album art footer | 30min | `footer.rs`, `playlist.rs` |
| 4 | Annotation italic + spacing | 20min | `lyrics_popup.rs` |
| | **Total** | **~75min** | |

---

## Build & Commit

Once all 4 items are done, build + test + commit with:
```
cargo build --release -p youtui --bin youtui
cargo test --release -p youtui --bin youtui
git add -A && git commit -m "feat: final polish — search, icons, album art, annotations"
```
