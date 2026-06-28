use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use ytmapi_rs::common::VideoID;
use ytmapi_rs::query::playlist::PrivacyStatus;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistSavePopupAction {
    MoveUp,
    MoveDown,
    Save,
    Cancel,
}

impl Action for PlaylistSavePopupAction {
    fn context(&self) -> Cow<'_, str> {
        "Playlist Save Popup".into()
    }

    fn describe(&self) -> Cow<'_, str> {
        match self {
            PlaylistSavePopupAction::MoveUp => "Move Up",
            PlaylistSavePopupAction::MoveDown => "Move Down",
            PlaylistSavePopupAction::Save => "Save",
            PlaylistSavePopupAction::Cancel => "Cancel",
        }
        .into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedField {
    NameInput,
    DescriptionInput,
    PrivacyField,
    SaveButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopupMode {
    CreateNew,
    EditingName,
    EditingDescription,
}

pub struct PlaylistSavePopup {
    video_ids: Vec<VideoID<'static>>,
    mode: PopupMode,
    focused_field: FocusedField,
    playlist_name: String,
    playlist_description: String,
    playlist_privacy: PrivacyStatus,
}

impl_youtui_component!(PlaylistSavePopup);

impl ActionHandler<PlaylistSavePopupAction> for PlaylistSavePopup {
    fn apply_action(&mut self, action: PlaylistSavePopupAction) -> impl Into<YoutuiEffect<Self>> {
        use PlaylistSavePopupAction::*;

        let result: (ComponentEffect<Self>, Option<AppCallback>) = match action {
            MoveUp => {
                self.focused_field = match self.focused_field {
                    FocusedField::DescriptionInput => FocusedField::NameInput,
                    FocusedField::PrivacyField => FocusedField::DescriptionInput,
                    FocusedField::SaveButton => FocusedField::PrivacyField,
                    _ => self.focused_field,
                };
                (AsyncTask::new_no_op(), None)
            }
            MoveDown => {
                self.focused_field = match self.focused_field {
                    FocusedField::NameInput => FocusedField::DescriptionInput,
                    FocusedField::DescriptionInput => FocusedField::PrivacyField,
                    FocusedField::PrivacyField => FocusedField::SaveButton,
                    _ => self.focused_field,
                };
                (AsyncTask::new_no_op(), None)
            }
            Save => {
                if self.playlist_name.trim().is_empty() {
                    return (AsyncTask::new_no_op(), None);
                }

                let title = self.playlist_name.trim().to_string();
                let description = if self.playlist_description.trim().is_empty() {
                    None
                } else {
                    Some(self.playlist_description.trim().to_string())
                };
                let video_ids = self.video_ids.clone();

                let privacy = Some(self.playlist_privacy.clone());
                (
                    AsyncTask::new_no_op(),
                    Some(AppCallback::CreatePlaylistFromPopup {
                        title,
                        description,
                        privacy,
                        video_ids,
                    }),
                )
            }
            Cancel => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
        };

        result
    }
}

impl PlaylistSavePopup {
    pub fn new(video_ids: Vec<VideoID<'static>>) -> Self {
        Self {
            video_ids,
            mode: PopupMode::CreateNew,
            focused_field: FocusedField::NameInput,
            playlist_name: String::new(),
            playlist_description: String::new(),
            playlist_privacy: PrivacyStatus::Private,
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if event.code == KeyCode::Esc {
            match self.mode {
                PopupMode::EditingName | PopupMode::EditingDescription => {
                    self.mode = PopupMode::CreateNew;
                    return (AsyncTask::new_no_op(), None);
                }
                PopupMode::CreateNew => {
                    return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
                }
            }
        }

        if event.code == KeyCode::Enter {
            match (self.mode, self.focused_field) {
                (PopupMode::EditingName, _) | (PopupMode::EditingDescription, _) => {
                    self.mode = PopupMode::CreateNew;
                }
                (PopupMode::CreateNew, FocusedField::SaveButton) => {
                    let effect: YoutuiEffect<Self> =
                        self.apply_action(PlaylistSavePopupAction::Save).into();
                    return (effect.effect, effect.callback);
                }
                (PopupMode::CreateNew, FocusedField::NameInput) => {
                    self.mode = PopupMode::EditingName;
                }
                (PopupMode::CreateNew, FocusedField::DescriptionInput) => {
                    self.mode = PopupMode::EditingDescription;
                }
                (PopupMode::CreateNew, FocusedField::PrivacyField) => {
                    self.playlist_privacy = match self.playlist_privacy {
                        PrivacyStatus::Private => PrivacyStatus::Public,
                        PrivacyStatus::Public => PrivacyStatus::Unlisted,
                        PrivacyStatus::Unlisted => PrivacyStatus::Private,
                    };
                }
            }
            return (AsyncTask::new_no_op(), None);
        }

        match event.code {
            KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => match self.mode
            {
                PopupMode::EditingName => {
                    self.playlist_name.push(c);
                    return (AsyncTask::new_no_op(), None);
                }
                PopupMode::EditingDescription => {
                    self.playlist_description.push(c);
                    return (AsyncTask::new_no_op(), None);
                }
                _ => {}
            },
            KeyCode::Backspace => match self.mode {
                PopupMode::EditingName => {
                    self.playlist_name.pop();
                    return (AsyncTask::new_no_op(), None);
                }
                PopupMode::EditingDescription => {
                    self.playlist_description.pop();
                    return (AsyncTask::new_no_op(), None);
                }
                _ => {}
            },
            _ => {}
        }

        if self.mode == PopupMode::CreateNew {
            match event.code {
                KeyCode::Char('k') => {
                    let effect: YoutuiEffect<Self> =
                        self.apply_action(PlaylistSavePopupAction::MoveUp).into();
                    return (effect.effect, effect.callback);
                }
                KeyCode::Char('j') => {
                    let effect: YoutuiEffect<Self> =
                        self.apply_action(PlaylistSavePopupAction::MoveDown).into();
                    return (effect.effect, effect.callback);
                }
                KeyCode::Char('i') => {
                    match self.focused_field {
                        FocusedField::NameInput => {
                            self.mode = PopupMode::EditingName;
                        }
                        FocusedField::DescriptionInput => {
                            self.mode = PopupMode::EditingDescription;
                        }
                        _ => {}
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                _ => {}
            }
        }

        (AsyncTask::new_no_op(), None)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(55, 50, area);
        frame.render_widget(Clear, popup_area);
        self.draw_create_form(frame, popup_area);
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

    fn draw_create_form(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(" Create Playlist ({} songs) ", self.video_ids.len());
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Length(2),
            ])
            .split(inner);

        let name_text = if self.mode == PopupMode::EditingName {
            format!("{}█", self.playlist_name)
        } else {
            self.playlist_name.clone()
        };
        let name_style = if self.focused_field == FocusedField::NameInput {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let name_widget = Paragraph::new(name_text)
            .block(Block::default().title("Name").borders(Borders::ALL))
            .style(name_style);
        frame.render_widget(name_widget, chunks[0]);

        let desc_text = if self.mode == PopupMode::EditingDescription {
            format!("{}█", self.playlist_description)
        } else {
            self.playlist_description.clone()
        };
        let desc_style = if self.focused_field == FocusedField::DescriptionInput {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let desc_widget = Paragraph::new(desc_text)
            .block(
                Block::default()
                    .title("Description (optional)")
                    .borders(Borders::ALL),
            )
            .style(desc_style)
            .wrap(Wrap { trim: false });
        frame.render_widget(desc_widget, chunks[1]);

        let button_text = if self.focused_field == FocusedField::SaveButton {
            "[ CREATE PLAYLIST ]"
        } else {
            "  Create Playlist  "
        };
        let button_style = if self.focused_field == FocusedField::SaveButton {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        let privacy_label = format!("Privacy: {}", self.playlist_privacy);
        let privacy_style = if self.focused_field == FocusedField::PrivacyField {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let privacy_widget = Paragraph::new(privacy_label)
            .block(Block::default().title("Privacy").borders(Borders::ALL))
            .style(privacy_style)
            .alignment(Alignment::Center);
        frame.render_widget(privacy_widget, chunks[2]);

        let save_button = Paragraph::new(button_text)
            .style(button_style)
            .alignment(Alignment::Center);
        frame.render_widget(save_button, chunks[4]);

        let instructions = match self.mode {
            PopupMode::EditingName | PopupMode::EditingDescription => {
                "Type to edit | Enter: Done | Esc: Cancel"
            }
            _ => "j/k: Navigate | Enter: Edit/Save | Esc: Cancel",
        };
        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(instructions_widget, chunks[5]);
    }
}
