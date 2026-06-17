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
    Loaded(String),
    Error(String),
}

pub struct LyricsPopup {
    pub state: LyricsPopupState,
    scroll_offset: usize,
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
        }
    }

    pub fn set_lyrics(&mut self, lyrics: String) {
        self.state = LyricsPopupState::Loaded(lyrics);
        self.scroll_offset = 0;
    }

    pub fn set_error(&mut self, error: String) {
        self.state = LyricsPopupState::Error(error);
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
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
            LyricsPopupState::Loaded(lyrics) => {
                let block = Block::default()
                    .title(" Lyrics ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));
                let inner = block.inner(popup_area);
                frame.render_widget(block, popup_area);
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(inner);

                let line_count = lyrics.lines().count();
                let visible_lines = (chunks[0].height as usize).saturating_sub(1);
                let max_scroll = line_count.saturating_sub(visible_lines);
                if self.scroll_offset > max_scroll {
                    self.scroll_offset = max_scroll;
                }

                let visible: String = lyrics.lines().skip(self.scroll_offset).take(visible_lines).collect::<Vec<_>>().join("\n");
                let has_more = self.scroll_offset + visible_lines < line_count;
                let scroll_hint = if has_more { " j/k scroll " } else { "" };

                let lyrics_widget = Paragraph::new(visible)
                    .style(Style::default().fg(Color::White))
                    .wrap(Wrap { trim: false })
                    .alignment(Alignment::Left);
                frame.render_widget(lyrics_widget, chunks[0]);
                let hint = Paragraph::new(format!("Esc/q: Close{}", scroll_hint))
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
