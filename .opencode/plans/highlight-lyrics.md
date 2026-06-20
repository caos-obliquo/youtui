# Highlight/Yank in Lyrics & Annotations — Plan

## Architecture

### State
Add to `LyricsPopup`:
```rust
pub visual_mode: bool,
pub visual_start: usize,
pub visual_end: usize,
```

### Keybindings
| Key | Action |
|-----|--------|
| `V` | Toggle visual mode on focused pane |
| `j`/`k` | Extend selection down/up (when visual mode) |
| `y` | Yank selected lines to clipboard, exit visual mode |
| `Esc` | Exit visual mode (one Esc = Normal, two = close popup) |

### Rendering
- **Lyrics pane**: Switch from `Paragraph::new(plain_string)` to `Vec<Line>` where each line has its own style
- **Annotations pane**: Already uses `Vec<Line>` (done in the italic fix). Extend to apply visual highlight style to selected lines

When visual mode is active, lines from `visual_start` to `visual_end` get:
```rust
Style::default().bg(ROW_HIGHLIGHT_COLOUR)  // or similar highlight color
```

### Yank
When `y` is pressed in visual mode:
- For lyrics: collect selected lines, join with `\n`, pipe to `wl-copy`
- For annotations: collect selected annotation text (fragment + explanation), join with `\n`, pipe to `wl-copy`

---

## Implementation Steps

| Step | Change | File | Lines |
|------|--------|------|-------|
| 1 | Add `visual_mode`, `visual_start`, `visual_end` + init | `lyrics_popup.rs:48-58` | 5 |
| 2 | Switch lyrics rendering from `String` to `Vec<Line>` | `lyrics_popup.rs:200-213` | 15 |
| 3 | Add visual highlight style to lyrics lines when visual mode active | `lyrics_popup.rs:213-219` | 8 |
| 4 | Add `V`, `y`, Esc visual mode handling in `handle_key` | `lyrics_popup.rs:103-151` | 20 |
| 5 | Highlight annotations lines when visual mode active (pane) | `lyrics_popup.rs:233-263` | 10 |
| 6 | Yank: collect selected lines and copy to `wl-copy` | `lyrics_popup.rs: +method` | 15 |
| | **Total** | | **~73 lines** |

---

## Key Implementation Details

### Lyrics → Vec<Line>
Current:
```rust
let l_visible_text: String = lyrics_text.lines().skip(self.scroll_offset).take(l_visible).collect::<Vec<_>>().join("\n");
frame.render_widget(Paragraph::new(l_visible_text).style(l_style), l_chunks[0]);
```

New:
```rust
let lyrics_lines: Vec<Line> = lyrics_text.lines().skip(self.scroll_offset).take(l_visible).enumerate()
    .map(|(i, line)| {
        let abs_line = self.scroll_offset + i;
        let style = if self.visual_mode && abs_line >= self.visual_start && abs_line <= self.visual_end {
            Style::default().bg(ROW_HIGHLIGHT_COLOUR)
        } else {
            l_style
        };
        Line::from(Span::styled(line.to_string(), style))
    }).collect();
frame.render_widget(Paragraph::new(lyrics_lines).wrap(Wrap { trim: false }), l_chunks[0]);
```

### Annotations → Visual Highlight
The annotations already use `Vec<Line>`. I need to:
1. Track the ABSOLUTE line number for each annotation
2. Check if each line falls within `visual_start..visual_end`
3. Apply highlight style if so

### Yank method
```rust
fn yank_visual_selection(&self) {
    let (start, end) = if self.visual_start <= self.visual_end {
        (self.visual_start, self.visual_end)
    } else {
        (self.visual_end, self.visual_start)
    };
    let lines = match self.focus {
        Focus::Lyrics => self.original_lyrics.lines().skip(start).take(end - start + 1).collect::<Vec<_>>().join("\n"),
        Focus::Annotations => {
            let ann_text: Vec<&str> = self.annotations.iter()
                .flat_map(|a| {
                    let mut l = vec![&a.fragment[..]];
                    l.extend(a.explanation.split('\n'));
                    l.push("");
                    l
                }).collect();
            ann_text.iter().skip(start).take(end - start + 1).copied().collect::<Vec<_>>().join("\n")
        }
    };
    let _ = std::process::Command::new("wl-copy").arg(&lines).spawn();
}
```

---

## Files Changed
**One file**: `lyrics_popup.rs`
**Lines**: ~73

## Est. Time
~30-40 min
