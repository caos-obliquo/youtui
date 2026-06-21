use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::num::NonZeroUsize;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsPopupAction {
    Close,
}

impl Action for LyricsPopupAction {
    fn context(&self) -> Cow<'_, str> {
        "Lyrics".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            LyricsPopupAction::Close => "Close",
        }
        .into()
    }
}

pub enum LyricsPopupState {
    Loading,
    #[allow(dead_code)]
    Loaded(String),
    Error(String),
}

pub struct Annotation {
    pub fragment: String,
    pub explanation: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Lyrics,
    Annotations,
}

pub struct LyricsPopup {
    pub state: LyricsPopupState,
    scroll_offset: usize,
    pub annotations: Vec<Annotation>,
    pub show_annotations: bool,
    pub romaji_mode: bool,
    romaji_cache: Option<String>,
    original_lyrics: String,
    lines: Vec<String>,
    focus: Focus,
    pub visual_mode: bool,
    pub visual_start: usize,
    pub visual_end: usize,
    count_prefix: usize,
    cursor_line: usize,
    cursor_col: usize,
    pub lyrics_cache: LruCache<String, String>,
    pub lyrics_cache_key: Option<String>,
}

impl_youtui_component!(LyricsPopup);

impl ActionHandler<LyricsPopupAction> for LyricsPopup {
    fn apply_action(&mut self, action: LyricsPopupAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            LyricsPopupAction::Close => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
        }
    }
}

impl LyricsPopup {
    pub fn new() -> Self {
        Self {
            state: LyricsPopupState::Loading,
            scroll_offset: 0,
            annotations: Vec::new(),
            show_annotations: false,
            romaji_mode: false,
            romaji_cache: None,
            original_lyrics: String::new(),
            lines: Vec::new(),
            focus: Focus::Lyrics,
            visual_mode: false,
            visual_start: 0,
            visual_end: 0,
            count_prefix: 0,
            cursor_line: 0,
            cursor_col: 0,
            lyrics_cache: LruCache::new(NonZeroUsize::new(50).unwrap()),
            lyrics_cache_key: None,
        }
    }

    pub fn set_lyrics(&mut self, lyrics: String) {
        if let Some(key) = &self.lyrics_cache_key {
            self.lyrics_cache.put(key.clone(), lyrics.clone());
        }
        self.original_lyrics = lyrics.clone();
        self.state = LyricsPopupState::Loaded(lyrics);
        self.romaji_cache = None;
        self.scroll_offset = 0;
        self.rebuild_lines();
    }

    #[allow(dead_code)]
    pub fn set_annotations(&mut self, annotations: Vec<Annotation>) {
        self.annotations = annotations;
        self.rebuild_lines();
    }

    pub fn set_error(&mut self, error: String) {
        self.state = LyricsPopupState::Error(error);
    }

    fn rebuild_lines(&mut self) {
        self.lines.clear();
        for line in self.original_lyrics.lines() {
            self.lines.push(line.to_string());
        }
    }

    fn total_lines(&self) -> usize {
        self.lines.len().max(1)
    }

    fn cursor_to_scroll(&mut self) {
        let visible = 20;
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + visible {
            self.scroll_offset = self.cursor_line.saturating_add(1).saturating_sub(visible);
        }
    }
    fn reset_count(&mut self) {
        self.count_prefix = 0;
    }

    fn next_word_boundary(text: &str, from: usize) -> Option<usize> {
        let rest = &text[from..];
        let mut in_word = false;
        for (i, c) in rest.char_indices() {
            let is_word = c.is_alphanumeric() || c == '_';
            if !in_word && is_word { in_word = true; }
            else if in_word && !is_word {
                return Some(from + i);
            }
        }
        None
    }

    fn prev_word_boundary(text: &str, from: usize) -> Option<usize> {
        let before = &text[..from];
        let mut in_word = false;
        for (i, c) in before.char_indices().rev() {
            let is_word = c.is_alphanumeric() || c == '_';
            if !in_word && is_word { in_word = true; }
            else if in_word && !is_word {
                return Some(i + 1);
            }
        }
        if in_word { Some(0) } else { None }
    }

    fn next_word_end(text: &str, from: usize) -> Option<usize> {
        let rest = &text[from..];
        let mut in_word = false;
        let mut end = None;
        for (i, c) in rest.char_indices() {
            let is_word = c.is_alphanumeric() || c == '_';
            if !in_word && is_word { in_word = true; }
            if in_word && is_word { end = Some(from + i); }
            if in_word && !is_word { break; }
        }
        end
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        // Count prefix: accumulate digits
        if let KeyCode::Char(c) = event.code {
            if let Some(d) = c.to_digit(10) {
                self.count_prefix = self.count_prefix * 10 + d as usize;
                return (AsyncTask::new_no_op(), None);
            }
        }
        if self.visual_mode {
            match event.code {
                KeyCode::Esc | KeyCode::Char('V') => {
                    self.visual_mode = false;
                    self.reset_count();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let n = self.count_prefix.max(1);
                    self.reset_count();
                    let max_line = self.total_lines().saturating_sub(1);
                    self.visual_end = self.visual_end.saturating_add(n).min(max_line);
                    self.cursor_line = self.visual_end;
                    self.cursor_col = 0;
                    self.cursor_to_scroll();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let n = self.count_prefix.max(1);
                    self.reset_count();
                    self.visual_end = self.visual_end.saturating_sub(n);
                    self.cursor_line = self.visual_end;
                    self.cursor_col = 0;
                    self.cursor_to_scroll();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('g') => {
                    self.reset_count();
                    self.visual_end = 0;
                    self.cursor_line = 0;
                    self.cursor_col = 0;
                    self.scroll_offset = 0;
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('G') => {
                    self.reset_count();
                    let max_line = self.total_lines().saturating_sub(1);
                    self.visual_end = max_line;
                    self.cursor_line = max_line;
                    self.cursor_col = 0;
                    self.cursor_to_scroll();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('d') if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    let n = self.count_prefix.max(1) * 10;
                    self.reset_count();
                    let max_line = self.total_lines().saturating_sub(1);
                    self.visual_end = self.visual_end.saturating_add(n).min(max_line);
                    self.cursor_line = self.visual_end;
                    self.cursor_col = 0;
                    self.cursor_to_scroll();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('u') if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    let n = self.count_prefix.max(1) * 10;
                    self.reset_count();
                    self.visual_end = self.visual_end.saturating_sub(n);
                    self.cursor_line = self.visual_end;
                    self.cursor_col = 0;
                    self.cursor_to_scroll();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('y') => {
                    self.reset_count();
                    let (start, end) = if self.visual_start <= self.visual_end {
                        (self.visual_start, self.visual_end)
                    } else {
                        (self.visual_end, self.visual_start)
                    };
                    let lines = self.lines[start..=end.min(self.lines.len().saturating_sub(1))]
                        .join("\n");
                    let _ = std::process::Command::new("wl-copy").arg(&lines).spawn();
                    self.visual_mode = false;
                    return (AsyncTask::new_no_op(), None);
                }
                _ => { self.reset_count(); }
            }
        }
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.reset_count();
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char('a') => {
                self.reset_count();
                self.show_annotations = !self.show_annotations;
                self.rebuild_lines();
                if self.show_annotations {
                    self.cursor_line = self.original_lyrics.lines().count().min(self.lines.len().saturating_sub(1));
                } else {
                    self.cursor_line = 0;
                }
                self.scroll_offset = 0;
                self.focus = if self.show_annotations { Focus::Annotations } else { Focus::Lyrics };
                tracing::info!("Toggle annotations: show={}, count={}", self.show_annotations, self.annotations.len());
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('V') => {
                self.reset_count();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('R') => {
                self.reset_count();
                self.romaji_mode = !self.romaji_mode;
                self.romaji_cache = None;
                self.scroll_offset = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Tab | KeyCode::Char('l') => {
                self.reset_count();
                if self.show_annotations && self.focus == Focus::Lyrics {
                    self.focus = Focus::Annotations;
                } else {
                    self.focus = Focus::Lyrics;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::BackTab | KeyCode::Char('h') => {
                self.reset_count();
                if self.focus == Focus::Annotations {
                    self.focus = Focus::Lyrics;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let n = self.count_prefix.max(1);
                self.reset_count();
                self.cursor_line = self.cursor_line.saturating_add(n).min(self.total_lines().saturating_sub(1));
                self.cursor_col = 0;
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let n = self.count_prefix.max(1);
                self.reset_count();
                self.cursor_line = self.cursor_line.saturating_sub(n);
                self.cursor_col = 0;
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('w') => {
                self.reset_count();
                if let Some(line) = self.lines.get(self.cursor_line) {
                    if let Some(pos) = Self::next_word_boundary(line, self.cursor_col) {
                        self.cursor_col = pos;
                    } else {
                        self.cursor_line = (self.cursor_line + 1).min(self.total_lines().saturating_sub(1));
                        self.cursor_col = 0;
                    }
                }
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('b') => {
                self.reset_count();
                if self.cursor_col > 0 {
                    if let Some(line) = self.lines.get(self.cursor_line) {
                        if let Some(pos) = Self::prev_word_boundary(line, self.cursor_col) {
                            self.cursor_col = pos;
                        } else {
                            self.cursor_col = 0;
                        }
                    }
                } else if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    if let Some(line) = self.lines.get(self.cursor_line) {
                        self.cursor_col = line.len();
                        if let Some(pos) = Self::prev_word_boundary(line, self.cursor_col) {
                            self.cursor_col = pos;
                        } else {
                            self.cursor_col = 0;
                        }
                    }
                }
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('e') => {
                self.reset_count();
                if let Some(line) = self.lines.get(self.cursor_line) {
                    if let Some(pos) = Self::next_word_end(line, self.cursor_col) {
                        self.cursor_col = pos;
                    } else {
                        self.cursor_line = (self.cursor_line + 1).min(self.total_lines().saturating_sub(1));
                        self.cursor_col = 0;
                    }
                }
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('g') => {
                self.reset_count();
                self.cursor_line = 0;
                self.cursor_col = 0;
                self.scroll_offset = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('G') => {
                self.reset_count();
                self.cursor_line = self.total_lines().saturating_sub(1);
                self.cursor_col = 0;
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('d') if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                let n = self.count_prefix.max(1) * 10;
                self.reset_count();
                let max_line = self.total_lines().saturating_sub(1);
                self.cursor_line = self.cursor_line.saturating_add(n).min(max_line);
                self.cursor_col = 0;
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('u') if event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                let n = self.count_prefix.max(1) * 10;
                self.reset_count();
                self.cursor_line = self.cursor_line.saturating_sub(n);
                self.cursor_col = 0;
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Enter => {
                self.reset_count();
                if let Some(line) = self.lines.get(self.cursor_line) {
                    let trimmed = line.trim();
                    // Parse [m:ss] or [mm:ss] at start of line
                    if trimmed.starts_with('[') {
                        let rest = trimmed.trim_start_matches('[');
                        if let Some(close) = rest.find(']') {
                            let time_str = &rest[..close];
                            let parts: Vec<&str> = time_str.split(':').collect();
                            if parts.len() == 2 {
                                if let (Ok(mins), Ok(secs)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                                    let dur = std::time::Duration::from_secs(mins * 60 + secs);
                                    return (AsyncTask::new_no_op(), Some(AppCallback::SeekTo(dur)));
                                }
                            }
                        }
                    }
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('[') => {
                self.reset_count();
                (AsyncTask::new_no_op(), Some(AppCallback::SeekBack))
            }
            KeyCode::Char(']') => {
                self.reset_count();
                (AsyncTask::new_no_op(), Some(AppCallback::SeekForward))
            }
            KeyCode::Char('}') => {
                self.reset_count();
                let total = self.total_lines();
                let mut line = self.cursor_line;
                while line < total && self.lines.get(line).map_or(true, |l| l.trim().is_empty()) { line += 1; }
                while line < total && self.lines.get(line).map_or(false, |l| !l.trim().is_empty()) { line += 1; }
                while line < total && self.lines.get(line).map_or(true, |l| l.trim().is_empty()) { line += 1; }
                if line < total {
                    self.cursor_line = line;
                    self.cursor_col = 0;
                    self.cursor_to_scroll();
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('{') => {
                self.reset_count();
                let mut line = self.cursor_line;
                while line > 0 && self.lines.get(line).map_or(true, |l| l.trim().is_empty()) { line -= 1; }
                while line > 0 && self.lines.get(line).map_or(false, |l| !l.trim().is_empty()) { line -= 1; }
                while line > 0 && self.lines.get(line).map_or(true, |l| l.trim().is_empty()) { line -= 1; }
                while line > 0 && self.lines.get(line.saturating_sub(1)).map_or(false, |l| !l.trim().is_empty()) { line -= 1; }
                self.cursor_line = line;
                self.cursor_col = 0;
                self.cursor_to_scroll();
                (AsyncTask::new_no_op(), None)
            }
            _ => {
                self.reset_count();
                (AsyncTask::new_no_op(), None)
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::top_anchored_rect(area);
        frame.render_widget(Clear, popup_area);
        match &self.state {
            LyricsPopupState::Loading => {
                let block = Block::default()
                    .title(" Lyrics ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));
                let inner = block.inner(popup_area);
                frame.render_widget(block, popup_area);
                let spinner = Paragraph::new("Loading lyrics...")
                    .style(Style::default().fg(Color::Yellow))
                    .alignment(Alignment::Center);
                let vert = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(50), Constraint::Length(1), Constraint::Percentage(50)])
                    .split(inner);
                frame.render_widget(spinner, vert[1]);
            }
            LyricsPopupState::Loaded(_) => {
                let ann_count = self.annotations.len();
                let has_jp = has_japanese(&self.original_lyrics);
                let romaji_tag = if self.romaji_mode && has_jp { " [Romaji]" } else { "" };
                let split_view = self.show_annotations && ann_count > 0;
                let title = if split_view {
                    format!(" Lyrics (a: {} annotations){} ", ann_count, romaji_tag)
                } else {
                    format!(" Lyrics{} ", romaji_tag)
                };
                let block = Block::default()
                    .title(title.as_str())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));
                let inner = block.inner(popup_area);
                frame.render_widget(block, popup_area);
                let (lyrics_area, ann_area, hint_area) = if split_view {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(inner);
                    let horiz = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                        .split(chunks[0]);
                    (horiz[0], Some(horiz[1]), chunks[1])
                } else {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(inner);
                    (chunks[0], None, chunks[1])
                };
                let line_count = self.total_lines();
                let visible_lines_count = (lyrics_area.height as usize).saturating_sub(1);
                let max_scroll = line_count.saturating_sub(visible_lines_count);
                if self.scroll_offset > max_scroll { self.scroll_offset = max_scroll; }
                let max_digits = line_count.max(1).to_string().len().max(3);
                let lyrics_lines: Vec<ratatui::text::Line> = self.lines.iter()
                    .enumerate()
                    .skip(self.scroll_offset).take(visible_lines_count)
                    .map(|(abs_line, line)| {
                        let rel = (abs_line as isize) - (self.cursor_line as isize);
                        let num_str = if rel == 0 {
                            format!("{:>width$} ", abs_line, width = max_digits)
                        } else {
                            format!("{:>+width$} ", rel, width = max_digits)
                        };
                        let num_span = ratatui::text::Span::styled(num_str, Style::default().fg(Color::DarkGray));
                        let base_style = if self.visual_mode
                            && abs_line >= self.visual_start.min(self.visual_end)
                            && abs_line <= self.visual_start.max(self.visual_end)
                        {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        let show_cursor = !self.visual_mode && abs_line == self.cursor_line;
                        if show_cursor {
                            let before: String = line.chars().take(self.cursor_col).collect();
                            let at_char: String = line.chars().skip(self.cursor_col).take(1).collect();
                            let after: String = line.chars().skip(self.cursor_col + 1).collect();
                            ratatui::text::Line::from(vec![
                                num_span,
                                ratatui::text::Span::styled(before, base_style),
                                ratatui::text::Span::styled(
                                    if at_char.is_empty() { " ".to_string() } else { at_char },
                                    Style::default().fg(Color::Black).bg(Color::White),
                                ),
                                ratatui::text::Span::styled(after, base_style),
                            ])
                        } else {
                            let mut spans = vec![num_span];
                            spans.push(ratatui::text::Span::styled(line.to_string(), base_style));
                            ratatui::text::Line::from(spans)
                        }
                    }).collect();
                frame.render_widget(Paragraph::new(lyrics_lines).wrap(Wrap { trim: false }), lyrics_area);
                if let Some(ann_area) = ann_area {
                    let ann_colour = if self.focus == Focus::Annotations { Color::Cyan } else { Color::DarkGray };
                    let ann_block = Block::default()
                        .title(" Annotations ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(ann_colour));
                    let ann_inner = ann_block.inner(ann_area);
                    frame.render_widget(ann_block, ann_area);
                    let mut ann_lines: Vec<ratatui::text::Line> = Vec::new();
                    for a in &self.annotations {
                        ann_lines.push(ratatui::text::Line::from(
                            ratatui::text::Span::styled(
                                format!("── {}", a.fragment),
                                Style::default().fg(Color::Cyan),
                            ),
                        ));
                        for expl_line in a.explanation.split('\n') {
                            ann_lines.push(ratatui::text::Line::from(
                                ratatui::text::Span::styled(
                                    expl_line.to_string(),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ));
                        }
                        ann_lines.push(ratatui::text::Line::from(""));
                    }
                    frame.render_widget(
                        Paragraph::new(ann_lines).wrap(Wrap { trim: false }),
                        ann_inner,
                    );
                }
                let has_more = self.scroll_offset + visible_lines_count < line_count;
                let scroll_hint = if has_more { " j/k scroll " } else { "" };
                let ann_hint = if ann_count > 0 { " | a: Toggle annotations" } else { "" };
                let hint = Paragraph::new(format!("Esc/q: Close{} {} {}", ann_hint, romaji_tag, scroll_hint))
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center);
                frame.render_widget(hint, hint_area);
            }
            LyricsPopupState::Error(err) => {
                let block = Block::default()
                    .title(" Lyrics Error ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red));
                let inner = block.inner(popup_area);
                frame.render_widget(block, popup_area);
                let err_widget = Paragraph::new(err.as_str())
                    .style(Style::default().fg(Color::Red))
                    .alignment(Alignment::Center);
                let vert = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(50), Constraint::Length(1), Constraint::Percentage(50)])
                    .split(inner);
                frame.render_widget(err_widget, vert[1]);
            }
        }
    }

    fn top_anchored_rect(r: Rect) -> Rect {
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(0),
                Constraint::Min(1),
                Constraint::Length(5), // leave room for footer
            ])
            .split(r);
        vert[1]
    }
}

/// Check if text contains Japanese characters (hiragana, katakana, or kanji)
pub fn has_japanese(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{3040}'..='\u{309F}' | // Hiragana
            '\u{30A0}'..='\u{30FF}' | // Katakana
            '\u{3400}'..='\u{4DBF}' | // CJK Extension A
            '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        )
    })
}

/// Convert Japanese text to romaji using lindera (kanji→kana) + ib-romaji (kana→latin)
pub fn japanese_to_romaji(text: &str) -> String {
    use std::sync::OnceLock;
    static TOKENIZER: OnceLock<lindera::tokenizer::Tokenizer> = OnceLock::new();
    let tokenizer = TOKENIZER.get_or_init(|| {
        lindera::tokenizer::TokenizerBuilder::new()
            .ok()
            .map(|mut b| {
                b.set_segmenter_dictionary("embedded://ipadic");
                b
            })
            .and_then(|b| b.build().ok())
            .expect("Failed to create lindera tokenizer")
    });
    let romaji = ib_romaji::HepburnRomanizer::builder().kana(true).build();
    text.lines()
        .map(|line| convert_line_to_romaji(line, tokenizer, &romaji))
        .collect::<Vec<_>>()
        .join("\n")
}

fn convert_line_to_romaji(
    line: &str,
    tokenizer: &lindera::tokenizer::Tokenizer,
    romaji: &ib_romaji::HepburnRomanizer,
) -> String {
    let mut out = String::with_capacity(line.len());
    let mut buf = String::new();
    let mut in_jp = false;
    for c in line.chars() {
        let is_jp = matches!(c,
            '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
        );
        if is_jp != in_jp {
            if !buf.is_empty() {
                if in_jp {
                    out.push_str(&convert_jp(&buf, tokenizer, romaji));
                } else {
                    out.push_str(&buf);
                }
                buf.clear();
            }
            in_jp = is_jp;
        }
        buf.push(c);
    }
    if !buf.is_empty() {
        if in_jp {
            out.push_str(&convert_jp(&buf, tokenizer, romaji));
        } else {
            out.push_str(&buf);
        }
    }
    out
}

fn convert_jp(
    text: &str,
    tokenizer: &lindera::tokenizer::Tokenizer,
    romaji: &ib_romaji::HepburnRomanizer,
) -> String {
    let tokens = tokenizer.tokenize(text).unwrap_or_default();
    let mut out = String::with_capacity(text.len());
    for mut token in tokens {
        let reading = token.get("reading").unwrap_or("").to_string();
        if reading.is_empty() || reading == token.surface.as_ref() {
            let surface = token.surface.as_ref();
            if let Some(r) = romaji.romanize_kana_str_all(surface) {
                out.push_str(&r);
            } else {
                out.push_str(surface);
            }
        } else {
            if let Some(r) = romaji.romanize_kana_str_all(&reading) {
                out.push_str(&r);
            } else {
                out.push_str(&reading);
            }
        }
    }
    out
}
