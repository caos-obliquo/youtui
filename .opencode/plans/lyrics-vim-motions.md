# Vim Motions in Lyrics — Character-Level Plan

## Architecture Change

Current: `scroll_offset` moves by LINE only. Text rendered as plain `Paragraph`.
Required: Track CURSOR (line + column), move by WORD, render cursor position.

### New fields on `LyricsPopup`
```rust
count_prefix: usize,
cursor_line: usize,     // line index within lyrics text
cursor_col: usize,      // column within that line
```

### Key concept
The cursor has a LINE and COLUMN position within the full lyrics text. When the cursor moves beyond the visible window, `scroll_offset` adjusts to keep it in view. `w`/`b`/`e` operate on the text at `cursor_line` starting from `cursor_col`.

---

## Implementation Steps

### Step 1: Fields + init
```rust
// Struct additions:
pub count_prefix: usize,
pub cursor_line: usize,
pub cursor_col: usize,

// In new():
count_prefix: 0,
cursor_line: 0,
cursor_col: 0,
```

### Step 2: Digit accumulation
At very start of `handle_key`, before visual mode check:
```rust
if let KeyCode::Char(c) = event.code {
    if let Some(d) = c.to_digit(10) {
        self.count_prefix = self.count_prefix * 10 + d as usize;
        return (AsyncTask::new_no_op(), None);
    }
}
```

### Step 3: Count-aware `j`/`k`
```rust
KeyCode::Char('j') | KeyCode::Down => {
    let n = self.count_prefix.max(1);
    self.count_prefix = 0;
    let new_line = self.cursor_line.saturating_add(n);
    let max_line = self.original_lyrics.lines().count().saturating_sub(1);
    self.cursor_line = new_line.min(max_line);
    // Adjust scroll to keep cursor in view
    if self.cursor_line >= self.scroll_offset + visible_lines {
        self.scroll_offset = self.cursor_line.saturating_add(1).saturating_sub(visible_lines);
    }
    (AsyncTask::new_no_op(), None)
}
```

### Step 4: `w` — next word
```rust
KeyCode::Char('w') => {
    self.count_prefix = 0;
    let text = self.original_lyrics.lines().nth(self.cursor_line).unwrap_or("");
    let rest = &text[self.cursor_col..];
    // Find next word start (non-space after space)
    let mut pos = self.cursor_col;
    for (i, ch) in rest.char_indices().skip(1) { // skip current position
        if ch == ' ' && i > 0 {
            // Found end of current word, next word starts after spaces
            let after = rest[i..].trim_start();
            if !after.is_empty() {
                let consumed = rest.len() - after.len();
                self.cursor_col = self.cursor_col + i + consumed;
                break;
            }
        }
    }
    // If no more words on this line, move to next line cursor_col = 0
    if self.cursor_col >= text.len() {
        self.cursor_line = (self.cursor_line + 1).min(max_line);
        self.cursor_col = 0;
    }
    adjust_scroll();
    (AsyncTask::new_no_op(), None)
}
```

Need to define "word" boundaries:
- **`w`**: start of next word (sequence of alphanumeric + `_` or non-whitespace)
- **`b`**: start of previous word
- **`e`**: end of current/next word
- **`W`/`B`/`E`**: same but WORD = sequence of non-whitespace characters (punctuation included)

### Step 5: Word boundary helpers

```rust
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn next_word_boundary(text: &str, from: usize) -> Option<usize> {
    let rest = &text[from..];
    // Skip current word
    let after_word = rest.chars().skip_while(|&c| is_word_char(c)).count();
    let after_spaces = rest[after_word..].chars().skip_while(|&c| !is_word_char(c)).count();
    let total = from + after_word + after_spaces;
    if total < text.len() { Some(total) } else { None }
}

fn prev_word_boundary(text: &str, from: usize) -> Option<usize> {
    let before = &text[..from];
    let trimmed = before.trim_end();
    let before_pos = trimmed.rfind(|c: char| is_word_char(c)).unwrap_or(0);
    let word_start = trimmed[..=before_pos].rfind(|c: char| !is_word_char(c)).map(|i| i + 1).unwrap_or(0);
    if word_start < from { Some(word_start) } else { None }
}

fn word_end(text: &str, from: usize) -> Option<usize> {
    let rest = &text[from..];
    let end = rest.chars().skip_while(|&c| is_word_char(c)).count();
    if end > 0 { Some(from + end - 1) } else { None }
}
```

### Step 6: Cursor rendering

When rendering lyrics, the cursor position needs to be visible. Two approaches:

**A)** Render the cursor as a highlighted character at `cursor_line`/`cursor_col` within the `Vec<Line>` rendering.

**B)** Only use cursor for movement, keep the "visual mode highlight" as the visible feedback.

**Recommendation: A** — show cursor as inverted character:
```rust
// In the lyrics Vec<Line> render loop:
if line_idx == self.cursor_line {
    let before: String = line.chars().take(self.cursor_col).collect();
    let at_char: String = line.chars().skip(self.cursor_col).take(1).collect();
    let after: String = line.chars().skip(self.cursor_col + 1).collect();
    Line::from(vec![
        Span::raw(before),
        Span::styled(at_char, Style::default().fg(Color::Black).bg(Color::White)),
        Span::raw(after),
    ])
}
```

### Step 7: Sync visual_start with cursor

When entering visual mode (`V`), set `visual_start` and `visual_end` based on `cursor_line`. When `j`/`k` extend in visual mode, move `visual_end` by line (not column).

---

## Files Changed

**One file**: `lyrics_popup.rs`
**Lines**: ~120

## Est. Time
~45-60 min

## Summary

| Step | What | Lines |
|------|------|-------|
| 1 | `count_prefix`, `cursor_line`, `cursor_col` fields | 3 |
| 2 | Digit accumulation | 6 |
| 3 | Count-aware `j`/`k` with cursor | 15 |
| 4 | `w`/`b`/`e` word motions | 45 |
| 5 | `W`/`B`/`E` WORD motions | 10 |
| 6 | Cursor rendering in `Vec<Line>` | 30 |
| 7 | Visual mode sync with cursor | 10 |
| 8 | Reset count on other keys | 5 |
| | **Total** | **~124** |
