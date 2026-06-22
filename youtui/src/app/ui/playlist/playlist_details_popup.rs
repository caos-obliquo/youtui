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
use ytmapi_rs::parse::GetPlaylistDetails;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailsAction {
    Close,
}

impl Action for DetailsAction {
    fn context(&self) -> Cow<'_, str> {
        "Playlist Details".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            DetailsAction::Close => "Close",
        }
        .into()
    }
}

pub struct PlaylistDetailsPopup {
    pub loading_title: String,
    pub details: Option<GetPlaylistDetails>,
    pub loaded: bool,
    pub error: Option<String>,
}

impl_youtui_component!(PlaylistDetailsPopup);

impl ActionHandler<DetailsAction> for PlaylistDetailsPopup {
    fn apply_action(&mut self, action: DetailsAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            DetailsAction::Close => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
        }
    }
}

impl PlaylistDetailsPopup {
    pub fn new(loading_title: Option<String>) -> Self {
        Self {
            loading_title: loading_title.unwrap_or_default(),
            details: None,
            loaded: false,
            error: None,
        }
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(55, 45, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Playlist Details ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let display = if let Some(err) = &self.error {
            format!("Error: {}", err)
        } else if let Some(ref details) = self.details {
            let privacy_str = details.privacy
                .as_ref()
                .map(|p| format!("{:?}", p))
                .unwrap_or_else(|| "Unknown".to_string());
            let desc = details.description.as_deref().unwrap_or("-");
            let views = details.views.as_deref().unwrap_or("-");

            format!(
                "Title: {}\n\
                 Author: {}\n\
                 Tracks: {}\n\
                 Duration: {}\n\
                 Year: {}\n\
                 Privacy: {}\n\
                 Views: {}\n\
                 \n\
                 Description:\n{}",
                details.title,
                details.author,
                details.track_count_text,
                details.duration,
                details.year,
                privacy_str,
                views,
                desc,
            )
        } else {
            format!("Loading details for: {}", self.loading_title)
        };

        let info_widget = Paragraph::new(display)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });
        frame.render_widget(info_widget, chunks[0]);

        let hint = Paragraph::new("q: Close")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[1]);
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
