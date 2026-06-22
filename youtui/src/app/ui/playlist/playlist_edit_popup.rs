use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ytmapi_rs::common::PlaylistID;
use ytmapi_rs::query::playlist::PrivacyStatus;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedField {
    Title,
    Description,
    Privacy,
    SaveButton,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditMode {
    Idle,
    EditingTitle,
    EditingDescription,
}

pub struct PlaylistEditPopup {
    pub playlist_id: PlaylistID<'static>,
    title: String,
    description: String,
    privacy: PrivacyStatus,
    focused_field: FocusedField,
    edit_mode: EditMode,
}

impl_youtui_component!(PlaylistEditPopup);

impl PlaylistEditPopup {
    pub fn new(playlist_id: PlaylistID<'static>, title: String) -> Self {
        Self {
            playlist_id,
            title,
            description: String::new(),
            privacy: PrivacyStatus::Private,
            focused_field: FocusedField::Title,
            edit_mode: EditMode::Idle,
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> (async_callback_manager::AsyncTask<Self, crate::app::server::ArcServer, crate::app::server::TaskMetadata>, Option<AppCallback>) {
        match self.edit_mode {
            EditMode::EditingTitle | EditMode::EditingDescription => {
                match event.code {
                    KeyCode::Esc => {
                        self.edit_mode = EditMode::Idle;
                    }
                    KeyCode::Enter => {
                        self.edit_mode = EditMode::Idle;
                    }
                    KeyCode::Backspace => {
                        let buf = match self.edit_mode {
                            EditMode::EditingTitle => &mut self.title,
                            EditMode::EditingDescription => &mut self.description,
                            _ => unreachable!(),
                        };
                        buf.pop();
                    }
                    KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => {
                        let buf = match self.edit_mode {
                            EditMode::EditingTitle => &mut self.title,
                            EditMode::EditingDescription => &mut self.description,
                            _ => unreachable!(),
                        };
                        buf.push(c);
                    }
                    _ => {}
                }
                return (AsyncTask::new_no_op(), None);
            }
            EditMode::Idle => {}
        }

        match event.code {
            KeyCode::Esc => {
                return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
            }
            KeyCode::Enter => {
                match self.focused_field {
                    FocusedField::Title => {
                        self.edit_mode = EditMode::EditingTitle;
                    }
                    FocusedField::Description => {
                        self.edit_mode = EditMode::EditingDescription;
                    }
                    FocusedField::Privacy => {
                        self.privacy = match self.privacy {
                            PrivacyStatus::Private => PrivacyStatus::Public,
                            PrivacyStatus::Public => PrivacyStatus::Unlisted,
                            PrivacyStatus::Unlisted => PrivacyStatus::Private,
                        };
                    }
                    FocusedField::SaveButton => {
                        let pid = self.playlist_id.clone();
                        let title = if self.title.trim().is_empty() {
                            None
                        } else {
                            Some(self.title.trim().to_string())
                        };
                        let description = if self.description.trim().is_empty() {
                            None
                        } else {
                            Some(self.description.trim().to_string())
                        };
                        return (AsyncTask::new_no_op(), Some(AppCallback::EditPlaylistDetailsFromLibrary {
                            playlist_id: pid,
                            title,
                            description,
                            privacy: Some(self.privacy.clone()),
                        }));
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.focused_field = match self.focused_field {
                    FocusedField::Title => FocusedField::Description,
                    FocusedField::Description => FocusedField::Privacy,
                    FocusedField::Privacy => FocusedField::SaveButton,
                    FocusedField::SaveButton => FocusedField::Title,
                };
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.focused_field = match self.focused_field {
                    FocusedField::Title => FocusedField::SaveButton,
                    FocusedField::Description => FocusedField::Title,
                    FocusedField::Privacy => FocusedField::Description,
                    FocusedField::SaveButton => FocusedField::Privacy,
                };
            }
            KeyCode::Char('i') => {
                match self.focused_field {
                    FocusedField::Title => self.edit_mode = EditMode::EditingTitle,
                    FocusedField::Description => self.edit_mode = EditMode::EditingDescription,
                    _ => {}
                }
            }
            _ => {}
        }
        (AsyncTask::new_no_op(), None)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(55, 45, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Edit Playlist ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

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

        let title_text = if self.edit_mode == EditMode::EditingTitle {
            format!("{}█", self.title)
        } else {
            self.title.clone()
        };
        let title_style = if self.focused_field == FocusedField::Title {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let title_widget = Paragraph::new(title_text)
            .block(Block::default().title("Name").borders(Borders::ALL))
            .style(title_style);
        frame.render_widget(title_widget, chunks[0]);

        let desc_text = if self.edit_mode == EditMode::EditingDescription {
            format!("{}█", self.description)
        } else {
            self.description.clone()
        };
        let desc_style = if self.focused_field == FocusedField::Description {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let desc_widget = Paragraph::new(desc_text)
            .block(Block::default().title("Description").borders(Borders::ALL))
            .style(desc_style)
            .wrap(Wrap { trim: false });
        frame.render_widget(desc_widget, chunks[1]);

        let privacy_label = format!("Privacy: {}", self.privacy);
        let privacy_style = if self.focused_field == FocusedField::Privacy {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let privacy_widget = Paragraph::new(privacy_label)
            .block(Block::default().title("Privacy").borders(Borders::ALL))
            .style(privacy_style)
            .alignment(Alignment::Center);
        frame.render_widget(privacy_widget, chunks[2]);

        let button_text = if self.focused_field == FocusedField::SaveButton {
            "[ SAVE CHANGES ]"
        } else {
            "  Save Changes  "
        };
        let button_style = if self.focused_field == FocusedField::SaveButton {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        let save_button = Paragraph::new(button_text)
            .style(button_style)
            .alignment(Alignment::Center);
        frame.render_widget(save_button, chunks[4]);

        let instructions = match self.edit_mode {
            EditMode::EditingTitle | EditMode::EditingDescription => {
                "Type to edit | Enter: Done | Esc: Cancel"
            }
            EditMode::Idle => {
                "j/k: Navigate | i: Edit | Enter: Toggle/Confirm | Esc: Cancel"
            }
        };
        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(instructions_widget, chunks[5]);
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
