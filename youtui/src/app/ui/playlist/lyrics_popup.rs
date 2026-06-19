use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

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
    ann_scroll_offset: usize,
    pub annotations: Vec<Annotation>,
    pub show_annotations: bool,
    pub romaji_mode: bool,
    romaji_cache: Option<String>,
    original_lyrics: String,
    focus: Focus,
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
            ann_scroll_offset: 0,
            annotations: Vec::new(),
            show_annotations: false,
            romaji_mode: false,
            romaji_cache: None,
            original_lyrics: String::new(),
            focus: Focus::Lyrics,
        }
    }

    pub fn set_lyrics(&mut self, lyrics: String) {
        self.original_lyrics = lyrics.clone();
        self.state = LyricsPopupState::Loaded(lyrics);
        self.romaji_cache = None;
        self.scroll_offset = 0;
    }

    #[allow(dead_code)]
    pub fn set_annotations(&mut self, annotations: Vec<Annotation>) {
        self.annotations = annotations;
    }

    pub fn set_error(&mut self, error: String) {
        self.state = LyricsPopupState::Error(error);
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char('a') => {
                self.show_annotations = !self.show_annotations;
                self.ann_scroll_offset = 0;
                self.focus = if self.show_annotations { Focus::Annotations } else { Focus::Lyrics };
                tracing::info!("Toggle annotations: show={}, count={}", self.show_annotations, self.annotations.len());
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('R') => {
                self.romaji_mode = !self.romaji_mode;
                self.romaji_cache = None;
                self.scroll_offset = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Tab | KeyCode::Char('l') => {
                if self.show_annotations && self.focus == Focus::Lyrics {
                    self.focus = Focus::Annotations;
                } else {
                    self.focus = Focus::Lyrics;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::BackTab | KeyCode::Char('h') => {
                if self.focus == Focus::Annotations {
                    self.focus = Focus::Lyrics;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                match self.focus {
                    Focus::Lyrics => self.scroll_offset = self.scroll_offset.saturating_add(1),
                    Focus::Annotations => self.ann_scroll_offset = self.ann_scroll_offset.saturating_add(1),
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.focus {
                    Focus::Lyrics => self.scroll_offset = self.scroll_offset.saturating_sub(1),
                    Focus::Annotations => self.ann_scroll_offset = self.ann_scroll_offset.saturating_sub(1),
                }
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
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
                let ann_tag = if ann_count > 0 { format!(" (a: {})", ann_count) } else { String::new() };

                if self.show_annotations && ann_count > 0 {
                    // Side-by-side: lyrics | annotations
                    let block_title = format!(" Lyrics{} | Annotations{}", romaji_tag, ann_tag);
                    let block = Block::default()
                        .title(block_title.as_str())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan));
                    let inner = block.inner(popup_area);
                    frame.render_widget(block, popup_area);

                    let horiz = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                        .split(inner);

                    // Left: lyrics
                    let l_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(horiz[0]);

                    let lyrics_text = if self.romaji_mode {
                        self.romaji_cache.get_or_insert_with(|| {
                            japanese_to_romaji(&self.original_lyrics)
                        }).clone()
                    } else {
                        self.original_lyrics.clone()
                    };

                    let l_line_count = lyrics_text.lines().count();
                    let l_visible = (l_chunks[0].height as usize).saturating_sub(1);
                    let l_max = l_line_count.saturating_sub(l_visible);
                    if self.scroll_offset > l_max { self.scroll_offset = l_max; }
                    let l_visible_text: String = lyrics_text.lines().skip(self.scroll_offset).take(l_visible).collect::<Vec<_>>().join("\n");

                    let l_style = if self.focus == Focus::Lyrics {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    frame.render_widget(Paragraph::new(l_visible_text).style(l_style).wrap(Wrap { trim: false }), l_chunks[0]);

                    // Right: annotations
                    let r_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(horiz[1]);

                    // Annotation text with padding
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

                    let a_line_count = ann_text.lines().count();
                    let a_visible = (r_chunks[0].height as usize).saturating_sub(1);
                    let a_max = a_line_count.saturating_sub(a_visible);
                    if self.ann_scroll_offset > a_max { self.ann_scroll_offset = a_max; }
                    let a_visible_text: String = ann_text.lines().skip(self.ann_scroll_offset).take(a_visible).collect::<Vec<_>>().join("\n");

                    let r_style = if self.focus == Focus::Annotations {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    // Border left to separate annotations from lyrics
                    let ann_block = Block::default()
                        .borders(Borders::LEFT)
                        .border_style(Style::default().fg(Color::DarkGray));
                    let ann_inner = ann_block.inner(r_chunks[0]);
                    frame.render_widget(ann_block, r_chunks[0]);
                    frame.render_widget(Paragraph::new(a_visible_text).style(r_style).wrap(Wrap { trim: false }), ann_inner);

                    // Single footer below annotations panel
                    let l_scroll = if self.scroll_offset + l_visible < l_line_count { " j/k scroll" } else { "" };
                    let romaji_opt = if has_jp { " | R: Romaji" } else { "" };
                    let focus_tag = if self.focus == Focus::Lyrics { "[Lyrics]" } else { "[Ann]" };
                    let hint = Paragraph::new(format!("{} Esc/q: Close | Tab: Focus | a: Hide{} {}", focus_tag, romaji_opt, l_scroll))
                        .style(Style::default().fg(Color::DarkGray))
                        .alignment(Alignment::Center);
                    frame.render_widget(hint, r_chunks[1]);
                } else {
                    // Lyrics only (full width)
                    let ann_count = self.annotations.len();
                    let title = if ann_count > 0 {
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
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(inner);

                    let display_text: String = if self.romaji_mode {
                        self.romaji_cache.get_or_insert_with(|| {
                            japanese_to_romaji(&self.original_lyrics)
                        }).clone()
                    } else {
                        self.original_lyrics.clone()
                    };

                    let line_count = display_text.lines().count();
                    let visible_lines = (chunks[0].height as usize).saturating_sub(1);
                    let max_scroll = line_count.saturating_sub(visible_lines);
                    if self.scroll_offset > max_scroll { self.scroll_offset = max_scroll; }

                    let visible: String = display_text.lines().skip(self.scroll_offset).take(visible_lines).collect::<Vec<_>>().join("\n");
                    let has_more = self.scroll_offset + visible_lines < line_count;
                    let scroll_hint = if has_more { " j/k scroll " } else { "" };

                    frame.render_widget(Paragraph::new(visible).style(Style::default().fg(Color::White)).wrap(Wrap { trim: false }), chunks[0]);
                    let has_jp = has_japanese(&self.original_lyrics);
                    let romaji_option = if has_jp { " | R: Romaji" } else { "" };
                    let ann_option = if ann_count > 0 { " | a: Annotations sidebar" } else { "" };
                    let hint = Paragraph::new(format!("Esc/q: Close{}{}{}", ann_option, romaji_option, scroll_hint))
                        .style(Style::default().fg(Color::DarkGray))
                        .alignment(Alignment::Center);
                    frame.render_widget(hint, chunks[1]);
                }
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
