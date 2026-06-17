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

pub struct LyricsPopup {
    pub state: LyricsPopupState,
    scroll_offset: usize,
    pub annotations: Vec<Annotation>,
    pub show_annotations: bool,
    pub romaji_mode: bool,
    romaji_cache: Option<String>,
    original_lyrics: String,
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
                self.scroll_offset = 0;
                tracing::info!("Toggle annotations: show={}, count={}", self.show_annotations, self.annotations.len());
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('R') => {
                self.romaji_mode = !self.romaji_mode;
                self.romaji_cache = None;
                self.scroll_offset = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(70, 70, area);
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
                let romaji_tag = if self.romaji_mode { " [Romaji]" } else { "" };
                let title = if self.show_annotations {
                    format!(" Annotations{} ", romaji_tag)
                } else if ann_count > 0 {
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

                let display_text: String = if self.show_annotations {
                    self.annotations.iter()
                        .flat_map(|a| {
                            let indent = "  ";
                            let wrapped: Vec<String> = a.explanation
                                .split('\n')
                                .map(|l| format!("{}{}", indent, l))
                                .collect();
                            vec![
                                format!(" ┌ {}", a.fragment),
                                wrapped.join("\n"),
                                String::new(),
                            ]
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else if self.romaji_mode {
                    self.romaji_cache.get_or_insert_with(|| {
                        japanese_to_romaji(&self.original_lyrics)
                    }).clone()
                } else {
                    self.original_lyrics.clone()
                };

                let line_count = display_text.lines().count();
                let visible_lines = (chunks[0].height as usize).saturating_sub(1);
                let max_scroll = line_count.saturating_sub(visible_lines);
                if self.scroll_offset > max_scroll {
                    self.scroll_offset = max_scroll;
                }

                let visible: String = display_text.lines().skip(self.scroll_offset).take(visible_lines).collect::<Vec<_>>().join("\n");
                let has_more = self.scroll_offset + visible_lines < line_count;
                let scroll_hint = if has_more { " j/k scroll " } else { "" };

                let lyrics_widget = Paragraph::new(visible)
                    .style(Style::default().fg(Color::White))
                    .wrap(Wrap { trim: false })
                    .alignment(Alignment::Left);
                frame.render_widget(lyrics_widget, chunks[0]);
                let hint = Paragraph::new(format!("Esc/q: Close | a: Toggle Annotations | R: Romaji{}", scroll_hint))
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center);
                frame.render_widget(hint, chunks[1]);
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

    fn centered_rect_fixed(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
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

    // Process line by line to preserve line breaks
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
    // Split line into Japanese and non-Japanese segments
    let mut out = String::with_capacity(line.len());
    let mut buf = String::new();
    let mut in_jp = false;

    for c in line.chars() {
        let is_jp = matches!(c,
            '\u{3040}'..='\u{309F}'  // Hiragana
            | '\u{30A0}'..='\u{30FF}' // Katakana
            | '\u{4E00}'..='\u{9FFF}' // CJK Unified Ideographs (kanji)
            | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        );
        if is_jp != in_jp {
            // Flush buffer
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
    // Flush remaining buffer
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

// japanese_to_romaji uses lindera + ib-romaji for full conversion
