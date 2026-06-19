/// Vi-mode text editor for single-line input.
/// Modes: Normal, Insert, VisualLine, OperatorPending.
/// Motions: h/l (char), b/w (word), 0/$, x, dd, u, i/a/I/A, V (visual line).
/// History: j/k navigates history (for `:` command).
#[derive(Clone)]
pub struct ViTextEditor {
    pub buffer: String,
    pub cursor: usize,
    pub mode: ViMode,
    clipboard: String,
    history: Vec<String>,
    history_pos: usize,
    undo_stack: Vec<(usize, String)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ViMode {
    Normal,
    Insert,
    VisualLine,
    OperatorPending(char),
}

impl Default for ViTextEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl ViTextEditor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            mode: ViMode::Insert, // Start in Insert mode for backward compat
            clipboard: String::new(),
            history: Vec::new(),
            history_pos: 0,
            undo_stack: Vec::new(),
        }
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

    /// Cursor marker: block █ always (terminal handles shape)
    fn cursor_marker(&self) -> &'static str {
        "█"
    }

    /// Render with given prefix, no mode indicator. Used by `:` command.
    pub fn render_simple(&self, prefix: &str) -> String {
        let mark = self.cursor_marker();
        let total = format!("{}{}", prefix, self.buffer);
        let cursor_pos = prefix.len() + self.cursor;
        if cursor_pos < total.len() {
            format!("{}{}", &total[..cursor_pos], mark)
        } else {
            format!("{}{}", total, mark)
        }
    }

    /// Get mode display string for header/footer
    pub fn mode_char(&self) -> &'static str {
        match self.mode {
            ViMode::Normal => "[N]",
            ViMode::Insert => "[I]",
            ViMode::VisualLine => "[V]",
            ViMode::OperatorPending(_) => "[OP]",
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyCode, shift: bool) -> bool {
        match self.mode {
            ViMode::Insert => self.handle_insert(key),
            ViMode::Normal => self.handle_normal(key, shift),
            ViMode::VisualLine => self.handle_visual_line(key),
            ViMode::OperatorPending(op) => self.handle_operator_pending(key, op),
        }
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
                return true; // submit
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
            crossterm::event::KeyCode::Char('u') => {
                self.undo();
            }
            crossterm::event::KeyCode::Char('d') => {
                self.mode = ViMode::OperatorPending('d');
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
                if self.history_pos < self.history.len() {
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
                if self.history_pos > 0 {
                    self.history_pos -= 1;
                    self.buffer = self.history[self.history_pos].clone();
                    self.cursor = self.buffer.len();
                }
            }
            crossterm::event::KeyCode::Enter => {
                return true; // submit
            }
            _ => {}
        }
        false
    }

    fn handle_operator_pending(&mut self, key: crossterm::event::KeyCode, op: char) -> bool {
        match key {
            crossterm::event::KeyCode::Char('d') if op == 'd' => {
                // dd: delete line
                self.save_undo();
                self.clipboard = self.buffer.clone();
                self.buffer.clear();
                self.cursor = 0;
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('h') | crossterm::event::KeyCode::Left if op == 'd' => {
                if self.cursor > 0 {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor-1..self.cursor].to_string();
                    self.buffer.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('l') | crossterm::event::KeyCode::Right if op == 'd' => {
                if self.cursor < self.buffer.len() {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..self.cursor+1].to_string();
                    self.buffer.remove(self.cursor);
                }
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('w') if op == 'd' => {
                let end = next_word_boundary(&self.buffer, self.cursor);
                if end > self.cursor {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..end].to_string();
                    self.buffer.drain(self.cursor..end);
                }
                self.mode = ViMode::Normal;
            }
            crossterm::event::KeyCode::Char('$') if op == 'd' => {
                if self.cursor < self.buffer.len() {
                    self.save_undo();
                    self.clipboard = self.buffer[self.cursor..].to_string();
                    self.buffer.truncate(self.cursor);
                }
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
}
