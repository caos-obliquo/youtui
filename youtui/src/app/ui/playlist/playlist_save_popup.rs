// Fixed implementation for youtui/src/app/ui/playlist/playlist_save_popup.rs

use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use std::borrow::Cow;
use ytmapi_rs::common::VideoID;

// ============================================================================
// Action enum for PlaylistSavePopup
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaylistSavePopupAction {
    // Navigation
    MoveUp,
    MoveDown,

    // Mode switching
    EditName,
    EditDescription,

    // Actions
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
            PlaylistSavePopupAction::EditName => "Edit Name",
            PlaylistSavePopupAction::EditDescription => "Edit Description",
            PlaylistSavePopupAction::Save => "Save",
            PlaylistSavePopupAction::Cancel => "Cancel",
        }
        .into()
    }
}

// ============================================================================
// Supporting structures
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopupMode {
    CreateNew,
    EditingName,
    EditingDescription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedField {
    NameInput,
    DescriptionInput,
    SaveButton,
}

// ============================================================================
// Main PlaylistSavePopup structure
// ============================================================================

pub struct PlaylistSavePopup {
    // Songs to add to playlist
    video_ids: Vec<VideoID<'static>>,

    // Current mode
    mode: PopupMode,
    focused_field: FocusedField,

    // Input fields
    playlist_name: String,
    playlist_description: String,
}

// ============================================================================
// Component trait implementation using macro
// ============================================================================

impl_youtui_component!(PlaylistSavePopup);

// ============================================================================
// ActionHandler implementation
// ============================================================================

impl ActionHandler<PlaylistSavePopupAction> for PlaylistSavePopup {
    fn apply_action(&mut self, action: PlaylistSavePopupAction) -> impl Into<YoutuiEffect<Self>> {
        use PlaylistSavePopupAction::*;

        let result: (ComponentEffect<Self>, Option<AppCallback>) = match action {
            MoveUp => {
                // Cycle focus backwards
                self.focused_field = match self.focused_field {
                    FocusedField::DescriptionInput => FocusedField::NameInput,
                    FocusedField::SaveButton => FocusedField::DescriptionInput,
                    _ => self.focused_field,
                };
                (AsyncTask::new_no_op(), None)
            }
            MoveDown => {
                // Cycle focus forward
                self.focused_field = match self.focused_field {
                    FocusedField::NameInput => FocusedField::DescriptionInput,
                    FocusedField::DescriptionInput => FocusedField::SaveButton,
                    _ => self.focused_field,
                };
                (AsyncTask::new_no_op(), None)
            }
            EditName => {
                self.mode = PopupMode::EditingName;
                self.focused_field = FocusedField::NameInput;
                (AsyncTask::new_no_op(), None)
            }
            EditDescription => {
                self.mode = PopupMode::EditingDescription;
                self.focused_field = FocusedField::DescriptionInput;
                (AsyncTask::new_no_op(), None)
            }
            Save => self.save(),
            Cancel => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
        };

        result
    }
}

// ============================================================================
// Implementation methods
// ============================================================================

impl PlaylistSavePopup {
    pub fn new(video_ids: Vec<VideoID<'static>>) -> Self {
        Self {
            video_ids,
            mode: PopupMode::CreateNew,
            focused_field: FocusedField::NameInput,
            playlist_name: String::new(),
            playlist_description: String::new(),
        }
    }

    // Create the async task to save the playlist
    fn save(&self) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if self.playlist_name.trim().is_empty() {
            // Can't save without a name
            return (AsyncTask::new_no_op(), None);
        }

        let title = self.playlist_name.trim().to_string();
        let description = if self.playlist_description.trim().is_empty() {
            None
        } else {
            Some(self.playlist_description.trim().to_string())
        };
        let _video_ids = self.video_ids.clone();

        // TODO: Implement the actual API call here
        // For now, just log what would be created
        tracing::info!(
            "Creating playlist '{}' with {} videos",
            title,
            self.video_ids.len()
        );
        if let Some(desc) = &description {
            tracing::info!("Description: {}", desc);
        }

        (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
    }

    // Handle keyboard input directly - returns tuple for ui.rs
    pub fn handle_key(&mut self, event: KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        // Handle Escape to cancel/go back
        if event.code == KeyCode::Esc {
            match self.mode {
                PopupMode::EditingName | PopupMode::EditingDescription => {
                    // Exit editing mode
                    self.mode = PopupMode::CreateNew;
                    return (AsyncTask::new_no_op(), None);
                }
                PopupMode::CreateNew => {
                    // Close the popup
                    return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
                }
            }
        }

        // Handle Enter
        if event.code == KeyCode::Enter {
            match (self.mode, self.focused_field) {
                (PopupMode::EditingName, _) => {
                    self.mode = PopupMode::CreateNew;
                }
                (PopupMode::EditingDescription, _) => {
                    self.mode = PopupMode::CreateNew;
                }
                (PopupMode::CreateNew, FocusedField::SaveButton) => {
                    return self.save();
                }
                (PopupMode::CreateNew, FocusedField::NameInput) => {
                    self.mode = PopupMode::EditingName;
                }
                (PopupMode::CreateNew, FocusedField::DescriptionInput) => {
                    self.mode = PopupMode::EditingDescription;
                }
                _ => {}
            }
            return (AsyncTask::new_no_op(), None);
        }

        // Handle navigation keys (only in CreateNew mode)
        if self.mode == PopupMode::CreateNew {
            match event.code {
                // Arrow keys
                KeyCode::Up => {
                    let effect: YoutuiEffect<Self> = self.apply_action(PlaylistSavePopupAction::MoveUp).into();
                    return (effect.effect, effect.callback);
                }
                KeyCode::Down => {
                    let effect: YoutuiEffect<Self> = self.apply_action(PlaylistSavePopupAction::MoveDown).into();
                    return (effect.effect, effect.callback);
                }
                // Tab = forward/Down
                KeyCode::Tab => {
                    let effect: YoutuiEffect<Self> = self.apply_action(PlaylistSavePopupAction::MoveDown).into();
                    return (effect.effect, effect.callback);
                }
                // Shift+Tab = backward/Up
                KeyCode::BackTab => {
                    let effect: YoutuiEffect<Self> = self.apply_action(PlaylistSavePopupAction::MoveUp).into();
                    return (effect.effect, effect.callback);
                }
                // Ctrl+K = Up (vim-style, works in your terminal)
                KeyCode::Char('k') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    let effect: YoutuiEffect<Self> = self.apply_action(PlaylistSavePopupAction::MoveUp).into();
                    return (effect.effect, effect.callback);
                }
                // Ctrl+N = Down (Next, standard)
                KeyCode::Char('n') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    let effect: YoutuiEffect<Self> = self.apply_action(PlaylistSavePopupAction::MoveDown).into();
                    return (effect.effect, effect.callback);
                }
                _ => {}
            }
        }

        // Handle text input (only in editing modes)
        match event.code {
            KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => {
                match self.mode {
                    PopupMode::EditingName => {
                        self.playlist_name.push(c);
                    }
                    PopupMode::EditingDescription => {
                        self.playlist_description.push(c);
                    }
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                match self.mode {
                    PopupMode::EditingName => {
                        self.playlist_name.pop();
                    }
                    PopupMode::EditingDescription => {
                        self.playlist_description.pop();
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        (AsyncTask::new_no_op(), None)
    }
}

// ============================================================================
// Drawing implementation
// ============================================================================

impl PlaylistSavePopup {
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Create centered popup
        let popup_area = centered_rect(60, 40, area);

        // Clear background
        frame.render_widget(Clear, popup_area);

        self.draw_create_form(frame, popup_area);
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
                Constraint::Length(3),  // Name
                Constraint::Length(5),  // Description
                Constraint::Min(1),     // Spacer
                Constraint::Length(3),  // Save button
                Constraint::Length(2),  // Instructions
            ])
            .split(inner);

        // Name input
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

        // Description input
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
            .block(Block::default().title("Description (optional)").borders(Borders::ALL))
            .style(desc_style)
            .wrap(Wrap { trim: false });
        frame.render_widget(desc_widget, chunks[1]);

        // Save button
        let button_text = if self.focused_field == FocusedField::SaveButton {
            "[ CREATE PLAYLIST ]"
        } else {
            "  Create Playlist  "
        };
        let button_style = if self.focused_field == FocusedField::SaveButton {
            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        let save_button = Paragraph::new(button_text)
            .style(button_style)
            .alignment(Alignment::Center);
        frame.render_widget(save_button, chunks[3]);

        // Instructions
        let instructions = match self.mode {
            PopupMode::EditingName | PopupMode::EditingDescription => {
                "Type to edit | Enter: Done | Esc: Cancel"
            }
            _ => {
                "Tab(↓)/Shift+Tab(↑)/Ctrl+K(↑)/N(↓): Navigate | Enter: Edit/Save | Esc: Cancel"
            }
        };
        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(instructions_widget, chunks[4]);
    }
}

// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
