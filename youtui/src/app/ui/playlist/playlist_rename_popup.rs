use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use ytmapi_rs::common::PlaylistID;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameAction {
    Confirm,
    Cancel,
}

impl Action for RenameAction {
    fn context(&self) -> Cow<'_, str> {
        "Rename Playlist".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            RenameAction::Confirm => "Confirm Rename",
            RenameAction::Cancel => "Cancel",
        }
        .into()
    }
}

pub struct PlaylistRenamePopup {
    pub playlist_id: PlaylistID<'static>,
    pub current_title: String,
    edit_buffer: String,
}

impl_youtui_component!(PlaylistRenamePopup);

impl ActionHandler<RenameAction> for PlaylistRenamePopup {
    fn apply_action(&mut self, action: RenameAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            RenameAction::Confirm => {
                let new_title = self.edit_buffer.trim().to_string();
                if new_title.is_empty() || new_title == self.current_title {
                    return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
                }
                let pid = self.playlist_id.clone();
                (AsyncTask::new_no_op(), Some(AppCallback::RenamePlaylistFromLibrary {
                    playlist_id: pid,
                    new_title,
                }))
            }
            RenameAction::Cancel => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
        }
    }
}

impl PlaylistRenamePopup {
    pub fn new(playlist_id: PlaylistID<'static>, current_title: String) -> Self {
        Self {
            playlist_id,
            current_title: current_title.clone(),
            edit_buffer: current_title,
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc => {
                let YoutuiEffect { effect, callback } = self.apply_action(RenameAction::Cancel).into();
                return (effect, callback);
            }
            KeyCode::Enter => {
                let YoutuiEffect { effect, callback } = self.apply_action(RenameAction::Confirm).into();
                return (effect, callback);
            }
            KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            _ => {}
        }
        (AsyncTask::new_no_op(), None)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(50, 30, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Rename Playlist ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);

        let name_text = format!("{}█", self.edit_buffer);
        let name_widget = Paragraph::new(name_text)
            .block(Block::default().title("Name").borders(Borders::ALL))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(name_widget, chunks[0]);

        let hint = Paragraph::new("Enter: Confirm | Esc: Cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[2]);
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
