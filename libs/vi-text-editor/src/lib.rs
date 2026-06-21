/// Vi-mode text editor. Single-line or multiline.
/// Modes: Normal, Insert, VisualLine, OperatorPending.
/// Single-line: j/k = history nav, Enter = submit.
/// Multiline: j/k = line up/down, Enter = newline, gg/G = first/last line.
#[derive(Clone, Copy, PartialEq, Debug)]
enum FindDir { Forward, Backward }

#[derive(Clone)]
pub struct ViTextEditor {
    pub buffer: String,
    pub cursor: usize,
    pub mode: ViMode,
    clipboard: String,
    history: Vec<String>,
    history_pos: usize,
    undo_stack: Vec<(usize, String)>,
    redo_stack: Vec<(usize, String)>,
    pub multiline: bool,
    last_find: Option<(char, FindDir, bool)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ViMode {
    Normal,
    Insert,
    VisualLine,
    VisualChar,
    OperatorPending(char),
}

impl Default for ViTextEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl ViTextEditor {
    pub fn new() -> Self {
        Self::with_multiline(false)
    }

    pub fn new_multiline() -> Self {
        Self::with_multiline(true)
    }

    fn with_multiline(multiline: bool) -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            mode: ViMode::Insert,
            clipboard: String::new(),
            history: Vec::new(),
            history_pos: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            multiline,
            last_find: None,
        }
    }

    pub fn cursor_line(&self) -> usize {
        self.buffer[..self.cursor].matches('\n').count()
    }

    pub fn cursor_col(&self) -> usize {
        let last_nl = self.buffer[..self.cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.cursor - last_nl
    }

    fn line_start(&self) -> usize {
        self.buffer[..self.cursor].rfind('\n').map(|i| i + 1).unwrap_or(0)
    }

    fn line_end(&self) -> usize {
        self.buffer[self.cursor..].find('\n')
            .map(|i| self.cursor + i)
            .unwrap_or(self.buffer.len())
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer = text.to_string();
        self.cursor = self.buffer.len();
        self.undo_stack.clear();
    }

    pub fn get_text(&self) -> &str {
        &self.buffer
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.undo_stack.clear();
        self.mode = ViMode::Insert;
    }

    pub fn push_history(&mut self, entry: String) {
        if !entry.is_empty() {
            if self.history.last() != Some(&entry) {
                self.history.push(entry);
            }
        }
        self.history_pos = self.history.len();
    }

    fn cursor_marker(&self) -> &'static str {
        match self.mode {
            ViMode::Insert => "▎",
            _ => "█",
        }
    }

    /// Render with cursor block. Single-line: inline cursor. Multiline: per-line.
    pub fn render_simple(&self, prefix: &str) -> String {
        if !self.multiline {
            let mark = self.cursor_marker();
            let total = format!("{}{}", prefix, self.buffer);
            let cursor_pos = prefix.len() + self.cursor;
            if cursor_pos < total.len() {
                format!("{}{}", &total[..cursor_pos], mark)
            } else {
                format!("{}{}", total, mark)
            }
        } else {
            self.render_multiline()
        }
    }

    fn render_multiline(&self) -> String {
        let mark = self.cursor_marker();
        let cur_line = self.cursor_line();
        let cur_col = self.cursor_col();
        let mut result = String::new();
        for (i, line_text) in self.buffer.split('\n').enumerate() {
            if i > 0 { result.push('\n'); }
            if i == cur_line {
                if cur_col < line_text.len() {
                    result.push_str(&line_text[..cur_col]);
                    result.push_str(mark);
                    result.push_str(&line_text[cur_col..]);
                } else {
                    result.push_str(line_text);
                    result.push_str(mark);
                }
            } else {
                result.push_str(line_text);
            }
        }
        result
    }

    /// Get mode display string for header/footer
    pub fn mode_char(&self) -> &'static str {
        match self.mode {
            ViMode::Normal => "[N]",
            ViMode::Insert => "[I]",
            ViMode::VisualLine => "[V]",
            ViMode::VisualChar => "[v]",
            ViMode::OperatorPending(_) => "[OP]",
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyCode, shift: bool) -> bool {
        match self.mode {
            ViMode::Insert => self.handle_insert(key),
            ViMode::Normal => self.handle_normal(key, shift),
            ViMode::VisualLine => self.handle_visual_line(key),
            ViMode::VisualChar => self.handle_visual_char(key),
            ViMode::OperatorPending(op) => self.handle_operator_pending(key, op),
        }
    }

    fn handle_visual_char(&mut self, key: crossterm::event::KeyCode) -> bool {
        match key {
            crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('v') => {
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('d') => {
                self.save_undo();
                self.clipboard = self.buffer.clone();
                self.buffer.clear();
                self.cursor = 0;
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('y') => {
                self.clipboard = self.buffer.clone();
                self.mode = ViMode::Normal;
            }
            _ => self.mode = ViMode::Normal,
        }
        false
    }

    fn handle_visual_line(&mut self, key: crossterm::event::KeyCode) -> bool {
        match key {
            crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('V') => {
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('d') => {
                // Delete entire line
                self.save_undo();
                self.clipboard = self.buffer.clone();
                self.buffer.clear();
                self.cursor = 0;
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('y') => {
                // Yank entire line
                self.clipboard = self.buffer.clone();
                self.mode = ViMode::Normal;
            }
            _ => self.mode = ViMode::Normal,
        }
        false
    }

    fn handle_insert(&mut self, key: crossterm::event::KeyCode) -> bool {
        match key {
            crossterm::event::KeyCode::Esc => {
                self.mode = ViMode::Normal;
                if self.cursor > 0 { self.cursor -= 1; }
            }
            crossterm::event::KeyCode::Enter => {
                if self.multiline {
                    self.save_undo();
                    self.buffer.insert(self.cursor, '\n');
                    self.cursor += 1;
                } else {
                    return true; // submit
                }
            }
            crossterm::event::KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.save_undo();
                    self.cursor -= 1;
                    self.buffer.remove(self.cursor);
                }
            }
            crossterm::event::KeyCode::Char(c) => {
                self.save_undo();
                self.buffer.insert(self.cursor, c);
                self.cursor += 1;
            }
            crossterm::event::KeyCode::Left => {
                if self.cursor > 0 { self.cursor -= 1; }
            }
            crossterm::event::KeyCode::Right => {
                if self.cursor < self.buffer.len() { self.cursor += 1; }
            }
            crossterm::event::KeyCode::Home => self.cursor = 0,
            crossterm::event::KeyCode::End => self.cursor = self.buffer.len(),
            _ => {}
        }
        false
    }

    fn handle_normal(&mut self, key: crossterm::event::KeyCode, _shift: bool) -> bool {
        match key {
            crossterm::event::KeyCode::Char('i') => {
                self.mode = ViMode::Insert;
            }
            crossterm::event::KeyCode::Char('a') => {
                if self.cursor < self.buffer.len() { self.cursor += 1; }
                self.mode = ViMode::Insert;
            }
            crossterm::event::KeyCode::Char('I') => {
                self.cursor = 0;
                self.mode = ViMode::Insert;
            }
            crossterm::event::KeyCode::Char('A') => {
                self.cursor = self.buffer.len();
                self.mode = ViMode::Insert;
            }
            crossterm::event::KeyCode::Char('h') | crossterm::event::KeyCode::Left => {
                if self.cursor > 0 { self.cursor -= 1; }
            }
            crossterm::event::KeyCode::Char('l') | crossterm::event::KeyCode::Right => {
                if self.cursor < self.buffer.len() { self.cursor += 1; }
            }
            crossterm::event::KeyCode::Char('b') => {
                self.cursor = prev_word_boundary(&self.buffer, self.cursor);
            }
            crossterm::event::KeyCode::Char('w') => {
                self.cursor = next_word_boundary(&self.buffer, self.cursor);
            }
            crossterm::event::KeyCode::Char('e') => {
                self.cursor = end_of_word(&self.buffer, self.cursor);
            }
            crossterm::event::KeyCode::Char('0') | crossterm::event::KeyCode::Home => {
                self.cursor = 0;
            }
            crossterm::event::KeyCode::Char('$') | crossterm::event::KeyCode::End => {
                self.cursor = self.buffer.len();
            }
            crossterm::event::KeyCode::Char('x') => {
                if !self.buffer.is_empty() && self.cursor < self.buffer.len() {
                    self.save_undo();
                    self.buffer.remove(self.cursor);
                }
            }
            crossterm::event::KeyCode::Char(c @ ('f' | 'F' | 't' | 'T')) => {
                self.mode = ViMode::OperatorPending(c);
            }
            crossterm::event::KeyCode::Char(';') => {
                if let Some((ch, dir, till)) = self.last_find {
                    let pos = find_char(&self.buffer, self.cursor, ch, dir, till);
                    if pos < self.buffer.len() { self.cursor = pos; }
                }
            }
            crossterm::event::KeyCode::Char(',') => {
                if let Some((ch, dir, till)) = self.last_find {
                    let rev = match dir { FindDir::Forward => FindDir::Backward, FindDir::Backward => FindDir::Forward };
                    let pos = find_char(&self.buffer, self.cursor, ch, rev, till);
                    if pos < self.buffer.len() { self.cursor = pos; }
                }
            }
            crossterm::event::KeyCode::Char('u') => {
                self.undo();
            }
            crossterm::event::KeyCode::Char('r') => {
                self.mode = ViMode::OperatorPending('r');
            }
            crossterm::event::KeyCode::Char('c') => {
                self.mode = ViMode::OperatorPending('c');
            }
            crossterm::event::KeyCode::Char('d') => {
                self.mode = ViMode::OperatorPending('d');
            }
            crossterm::event::KeyCode::Char('v') => {
                self.mode = ViMode::VisualChar;
            }
            crossterm::event::KeyCode::Char('V') => {
                self.mode = ViMode::VisualLine;
            }
            crossterm::event::KeyCode::Char('y') => {
                self.mode = ViMode::OperatorPending('y');
            }
            crossterm::event::KeyCode::Char('p') => {
                if !self.clipboard.is_empty() {
                    self.save_undo();
                    let pos = self.cursor;
                    self.buffer.insert_str(pos, &self.clipboard);
                    self.cursor = pos + self.clipboard.len();
                }
            }
            crossterm::event::KeyCode::Char('P') => {
                if !self.clipboard.is_empty() {
                    self.save_undo();
                    self.buffer.insert_str(self.cursor, &self.clipboard);
                }
            }
            crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                if self.multiline {
                    let col = self.cursor_col();
                    let after = &self.buffer[self.cursor..];
                    if let Some(nl) = after.find('\n') {
                        self.cursor += nl + 1;
                        let line_len = self.buffer[self.cursor..].find('\n')
                            .unwrap_or(self.buffer.len() - self.cursor);
                        self.cursor += col.min(line_len);
                    }
                } else if self.history_pos < self.history.len() {
                    self.history_pos += 1;
                    if self.history_pos < self.history.len() {
                        self.buffer = self.history[self.history_pos].clone();
                    } else {
                        self.buffer.clear();
                    }
                    self.cursor = self.buffer.len();
                }
            }
            crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                if self.multiline {
                    let col = self.cursor_col();
                    let before = &self.buffer[..self.cursor];
                    if let Some(nl) = before[..before.len().saturating_sub(1)].rfind('\n') {
                        self.cursor = nl + 1;
                        let line_len = self.buffer[self.cursor..].find('\n')
                            .unwrap_or(self.buffer.len() - self.cursor);
                        self.cursor += col.min(line_len);
                    } else {
                        self.cursor = 0;
                    }
                } else if self.history_pos > 0 {
                    self.history_pos -= 1;
                    self.buffer = self.history[self.history_pos].clone();
                    self.cursor = self.buffer.len();
                }
            }
            crossterm::event::KeyCode::Char('g') => {
                if self.multiline {
                    self.mode = ViMode::OperatorPending('g');
                }
            }
            crossterm::event::KeyCode::Char('G') => {
                if self.multiline {
                    self.cursor = self.buffer.len();
                }
            }
            crossterm::event::KeyCode::Enter => {
                if self.multiline {
                    // no-op in normal mode for multiline
                } else {
                    return true; // submit
                }
            }
            _ => {}
        }
        false
    }

    fn handle_operator_pending(&mut self, key: crossterm::event::KeyCode, op: char) -> bool {
        match (key, op) {
            (crossterm::event::KeyCode::Char('d'), 'd') => {
                // dd: delete line
                self.save_undo();
                if self.multiline {
                    let start = self.line_start();
                    let end = self.line_end();
                    let after_nl = if end < self.buffer.len() { end + 1 } else { end };
                    self.clipboard = self.buffer[start..end].to_string();
                    self.buffer.drain(start..after_nl);
                    self.cursor = start.min(self.buffer.len());
                } else {
                    self.clipboard = self.buffer.clone();
                    self.buffer.clear();
                    self.cursor = 0;
                }
                self.mode = ViMode::Normal;
            }
            (crossterm::event::KeyCode::Char('h'), _) | (crossterm::event::KeyCode::Left, _) if op == 'd' => {
                if self.cursor > 0 {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor-1..self.cursor].to_string();
                    self.buffer.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                self.mode = ViMode::Normal;
            }
            (crossterm::event::KeyCode::Char('l'), _) | (crossterm::event::KeyCode::Right, _) if op == 'd' => {
                if self.cursor < self.buffer.len() {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..self.cursor+1].to_string();
                    self.buffer.remove(self.cursor);
                }
                self.mode = ViMode::Normal;
            }
            (crossterm::event::KeyCode::Char('w'), _) if op == 'd' => {
                let end = next_word_boundary(&self.buffer, self.cursor);
                if end > self.cursor {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..end].to_string();
                    self.buffer.drain(self.cursor..end);
                }
                self.mode = ViMode::Normal;
            }
            (crossterm::event::KeyCode::Char('$'), _) if op == 'd' => {
                if self.cursor < self.buffer.len() {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..].to_string();
                    self.buffer.truncate(self.cursor);
                }
                self.mode = ViMode::Normal;
            }
            // c operator: change (delete + insert)
            (crossterm::event::KeyCode::Char('w'), 'c') => {
                let end = next_word_boundary(&self.buffer, self.cursor);
                if end > self.cursor {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..end].to_string();
                    self.buffer.drain(self.cursor..end);
                }
                self.mode = ViMode::Insert;
            }
            (crossterm::event::KeyCode::Char('$'), 'c') => {
                if self.cursor < self.buffer.len() {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..].to_string();
                    self.buffer.truncate(self.cursor);
                }
                self.mode = ViMode::Insert;
            }
            (crossterm::event::KeyCode::Char('c'), 'c') => {
                // cc: change entire line
                self.save_undo();
                if self.multiline {
                    let start = self.line_start();
                    let end = self.line_end();
                    let after_nl = if end < self.buffer.len() { end + 1 } else { end };
                    self.clipboard = self.buffer[start..end].to_string();
                    self.buffer.drain(start..after_nl);
                    self.cursor = start.min(self.buffer.len());
                } else {
                    self.clipboard = self.buffer.clone();
                    self.buffer.clear();
                    self.cursor = 0;
                }
                self.mode = ViMode::Insert;
            }
            (crossterm::event::KeyCode::Char('g'), 'g') => {
                self.cursor = 0;
                self.mode = ViMode::Normal;
            }
            // r: replace single char
            (crossterm::event::KeyCode::Char(ch), 'r') => {
                if self.cursor < self.buffer.len() {
                    self.save_undo();
                    self.buffer.remove(self.cursor);
                    self.buffer.insert(self.cursor, ch);
                }
                self.mode = ViMode::Normal;
            }
            // f/F/t/T: next char keypress is the target
            (crossterm::event::KeyCode::Char(ch), 'f') => {
                let dir = FindDir::Forward;
                let till = false;
                let pos = find_char(&self.buffer, self.cursor, ch, dir, till);
                if pos < self.buffer.len() { self.cursor = pos; }
                self.last_find = Some((ch, dir, till));
                self.mode = ViMode::Normal;
            }
            (crossterm::event::KeyCode::Char(ch), 'F') => {
                let dir = FindDir::Backward;
                let till = false;
                let pos = find_char(&self.buffer, self.cursor, ch, dir, till);
                if pos < self.buffer.len() { self.cursor = pos; }
                self.last_find = Some((ch, dir, till));
                self.mode = ViMode::Normal;
            }
            (crossterm::event::KeyCode::Char(ch), 't') => {
                let dir = FindDir::Forward;
                let till = true;
                let pos = find_char(&self.buffer, self.cursor, ch, dir, till);
                if pos < self.buffer.len() { self.cursor = pos; }
                self.last_find = Some((ch, dir, till));
                self.mode = ViMode::Normal;
            }
            (crossterm::event::KeyCode::Char(ch), 'T') => {
                let dir = FindDir::Backward;
                let till = true;
                let pos = find_char(&self.buffer, self.cursor, ch, dir, till);
                if pos < self.buffer.len() { self.cursor = pos; }
                self.last_find = Some((ch, dir, till));
                self.mode = ViMode::Normal;
            }
            _ => {
                self.mode = ViMode::Normal;
            }
        }
        false
    }

    fn save_undo(&mut self) {
        if self.undo_stack.is_empty() || self.undo_stack.last().map(|(_, s)| s) != Some(&self.buffer) {
            self.undo_stack.push((self.cursor, self.buffer.clone()));
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
        }
    }

    fn undo(&mut self) {
        if let Some((cursor, text)) = self.undo_stack.pop() {
            self.buffer = text;
            self.cursor = cursor;
        }
    }
}

fn end_of_word(text: &str, cursor: usize) -> usize {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if cursor >= len { return len; }
    let mut pos = cursor;
    // If at word start, move into the word first
    if pos < len && bytes[pos] == b' ' {
        while pos < len && bytes[pos] == b' ' { pos += 1; }
    }
    // Move to end of word
    while pos < len && bytes[pos] != b' ' { pos += 1; }
    if pos > cursor { pos - 1 } else { cursor }
}

fn prev_word_boundary(text: &str, cursor: usize) -> usize {
    if cursor == 0 { return 0; }
    let bytes = text.as_bytes();
    let mut pos = cursor.saturating_sub(1);
    // Skip current word
    while pos > 0 && bytes[pos] != b' ' {
        pos -= 1;
    }
    // Skip spaces
    while pos > 0 && bytes[pos] == b' ' {
        pos -= 1;
    }
    // Go to start of word
    while pos > 0 && bytes[pos - 1] != b' ' {
        pos -= 1;
    }
    pos
}

fn next_word_boundary(text: &str, cursor: usize) -> usize {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if cursor >= len { return len; }
    let mut pos = cursor;
    // Skip current word
    while pos < len && bytes[pos] != b' ' {
        pos += 1;
    }
    // Skip spaces
    while pos < len && bytes[pos] == b' ' {
        pos += 1;
    }
    pos
}

fn find_char(text: &str, cursor: usize, ch: char, dir: FindDir, till: bool) -> usize {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let target = ch as u8;
    match dir {
        FindDir::Forward => {
            let start = if cursor + 1 < len { cursor + 1 } else { return len; };
            for i in start..len {
                if bytes[i] == target {
                    return if till { i.saturating_sub(1) } else { i };
                }
            }
            len
        }
        FindDir::Backward => {
            if cursor == 0 { return 0; }
            let end = cursor;
            for i in (0..end).rev() {
                if bytes[i] == target {
                    return if till { i + 1 } else { i };
                }
            }
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_chars() {
        let mut e = ViTextEditor::new();
        e.mode = ViMode::Insert;
        e.handle_key(crossterm::event::KeyCode::Char('h'), false);
        e.handle_key(crossterm::event::KeyCode::Char('i'), false);
        assert_eq!(e.buffer, "hi");
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn test_insert_esc_to_normal() {
        let mut e = ViTextEditor::new();
        e.mode = ViMode::Insert;
        e.set_text("hello");
        e.cursor = 5;
        e.handle_key(crossterm::event::KeyCode::Esc, false);
        assert_eq!(e.mode, ViMode::Normal);
        assert_eq!(e.cursor, 4); // moved back one
    }

    #[test]
    fn test_normal_motions() {
        let mut e = ViTextEditor::new();
        e.set_text("hello world");
        e.mode = ViMode::Normal;
        e.cursor = 0;
        e.handle_key(crossterm::event::KeyCode::Char('w'), false);
        assert_eq!(e.cursor, 6); // start of "world"
        e.handle_key(crossterm::event::KeyCode::Char('b'), false);
        assert_eq!(e.cursor, 0); // back to "hello"
        e.handle_key(crossterm::event::KeyCode::Char('$'), false);
        assert_eq!(e.cursor, 11);
        e.handle_key(crossterm::event::KeyCode::Char('0'), false);
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn test_delete_char() {
        let mut e = ViTextEditor::new();
        e.set_text("hello");
        e.mode = ViMode::Normal;
        e.cursor = 1;
        e.handle_key(crossterm::event::KeyCode::Char('x'), false);
        assert_eq!(e.buffer, "hllo");
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn test_delete_line() {
        let mut e = ViTextEditor::new();
        e.set_text("hello world");
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('d'), false);
        e.handle_key(crossterm::event::KeyCode::Char('d'), false);
        assert_eq!(e.buffer, "");
        assert_eq!(e.clipboard, "hello world");
    }

    #[test]
    fn test_undo() {
        let mut e = ViTextEditor::new();
        e.set_text("hello");
        e.mode = ViMode::Normal;
        e.cursor = 4;
        e.handle_key(crossterm::event::KeyCode::Char('x'), false);
        assert_eq!(e.buffer, "hell");
        e.handle_key(crossterm::event::KeyCode::Char('u'), false);
        assert_eq!(e.buffer, "hello");
    }

    #[test]
    fn test_history() {
        let mut e = ViTextEditor::new();
        e.push_history("cmd1".into());
        e.push_history("cmd2".into());
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('k'), false);
        assert_eq!(e.buffer, "cmd2");
        e.handle_key(crossterm::event::KeyCode::Char('k'), false);
        assert_eq!(e.buffer, "cmd1");
        e.handle_key(crossterm::event::KeyCode::Char('j'), false);
        assert_eq!(e.buffer, "cmd2");
    }

    #[test]
    fn test_visual_line_delete() {
        let mut e = ViTextEditor::new();
        e.set_text("hello world");
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('V'), false);
        assert_eq!(e.mode, ViMode::VisualLine);
        e.handle_key(crossterm::event::KeyCode::Char('d'), false);
        assert_eq!(e.buffer, "");
        assert_eq!(e.clipboard, "hello world");
        assert_eq!(e.mode, ViMode::Normal);
    }

    #[test]
    fn test_visual_line_yank_and_paste() {
        let mut e = ViTextEditor::new();
        e.set_text("yank me");
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('V'), false);
        e.handle_key(crossterm::event::KeyCode::Char('y'), false);
        assert_eq!(e.clipboard, "yank me");
        assert_eq!(e.buffer, "yank me"); // buffer unchanged
    }

    #[test]
    fn test_paste_after() {
        let mut e = ViTextEditor::new();
        e.set_text("hello");
        e.clipboard = " world".to_string();
        e.cursor = 5;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('p'), false);
        assert_eq!(e.buffer, "hello world");
        assert_eq!(e.cursor, 11);
    }

    #[test]
    fn test_operator_pending_dw() {
        let mut e = ViTextEditor::new();
        e.set_text("delete word here");
        e.cursor = 0;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('d'), false);
        assert_eq!(e.mode, ViMode::OperatorPending('d'));
        e.handle_key(crossterm::event::KeyCode::Char('w'), false);
        assert_eq!(e.buffer, "word here");
        assert_eq!(e.mode, ViMode::Normal);
    }

    #[test]
    fn test_operator_pending_dollar() {
        let mut e = ViTextEditor::new();
        e.set_text("delete from here to end");
        e.cursor = 0;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('d'), false);
        e.handle_key(crossterm::event::KeyCode::Char('$'), false);
        assert_eq!(e.buffer, "");
    }

    #[test]
    fn test_find_char_forward() {
        let mut e = ViTextEditor::new();
        e.set_text("hello world");
        e.cursor = 0;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('f'), false);
        e.handle_key(crossterm::event::KeyCode::Char('o'), false);
        assert_eq!(e.cursor, 4);
    }

    #[test]
    fn test_find_char_backward() {
        let mut e = ViTextEditor::new();
        e.set_text("hello world");
        e.cursor = 6;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('F'), false);
        e.handle_key(crossterm::event::KeyCode::Char('o'), false);
        assert_eq!(e.cursor, 4);
    }

    #[test]
    fn test_till_char_forward() {
        let mut e = ViTextEditor::new();
        e.set_text("hello world");
        e.cursor = 0;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('t'), false);
        e.handle_key(crossterm::event::KeyCode::Char('o'), false);
        assert_eq!(e.cursor, 3);
    }

    #[test]
    fn test_till_char_backward() {
        let mut e = ViTextEditor::new();
        e.set_text("hello world");
        e.cursor = 6;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('T'), false);
        e.handle_key(crossterm::event::KeyCode::Char('o'), false);
        assert_eq!(e.cursor, 5);
    }

    #[test]
    fn test_repeat_find() {
        let mut e = ViTextEditor::new();
        e.set_text("axbxcx");
        e.cursor = 0;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('f'), false);
        e.handle_key(crossterm::event::KeyCode::Char('x'), false);
        assert_eq!(e.cursor, 1);
        e.handle_key(crossterm::event::KeyCode::Char(';'), false);
        assert_eq!(e.cursor, 3);
        e.handle_key(crossterm::event::KeyCode::Char(';'), false);
        assert_eq!(e.cursor, 5);
    }

    #[test]
    fn test_reverse_find() {
        let mut e = ViTextEditor::new();
        e.set_text("x x x");
        e.cursor = 2;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('f'), false);
        e.handle_key(crossterm::event::KeyCode::Char('x'), false);
        assert_eq!(e.cursor, 4);
        e.handle_key(crossterm::event::KeyCode::Char(','), false);
        assert_eq!(e.cursor, 2);
        e.handle_key(crossterm::event::KeyCode::Char(';'), false);
        assert_eq!(e.cursor, 4);
    }

    #[test]
    fn test_find_char_not_found() {
        let mut e = ViTextEditor::new();
        e.set_text("hello");
        e.cursor = 0;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('f'), false);
        e.handle_key(crossterm::event::KeyCode::Char('z'), false);
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn test_replace_char() {
        let mut e = ViTextEditor::new();
        e.set_text("hello");
        e.cursor = 1;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('r'), false);
        assert_eq!(e.mode, ViMode::OperatorPending('r'));
        e.handle_key(crossterm::event::KeyCode::Char('a'), false);
        assert_eq!(e.buffer, "hallo");
        assert_eq!(e.mode, ViMode::Normal);
    }

    #[test]
    fn test_replace_at_end_noop() {
        let mut e = ViTextEditor::new();
        e.set_text("hi");
        e.cursor = 2;
        e.mode = ViMode::Normal;
        e.handle_key(crossterm::event::KeyCode::Char('r'), false);
        e.handle_key(crossterm::event::KeyCode::Char('x'), false);
        assert_eq!(e.buffer, "hi");
    }
}
