use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyModifiers};
use rat_text::text_area::{TextAreaState, TextArea};
use rat_text::event::TextOutcome;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigEditorAction {
    Close,
}

impl Action for ConfigEditorAction {
    fn context(&self) -> Cow<'_, str> {
        "Config Editor".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            ConfigEditorAction::Close => "Close",
        }
        .into()
    }
}

pub struct ConfigEditorPopup {
    pub state: TextAreaState,
    pub config_path: PathBuf,
}

impl_youtui_component!(ConfigEditorPopup);

impl ActionHandler<ConfigEditorAction> for ConfigEditorPopup {
    fn apply_action(&mut self, action: ConfigEditorAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            ConfigEditorAction::Close => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
        }
    }
}

impl ConfigEditorPopup {
    pub fn new(config_path: PathBuf, content: String) -> Self {
        let mut state = TextAreaState::new();
        state.set_text(&content);
        Self { state, config_path }
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char('s') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                let content = self.state.text();
                match std::fs::write(&self.config_path, content) {
                    Ok(_) => tracing::info!("Config saved to {:?}", self.config_path),
                    Err(e) => tracing::error!("Failed to save config: {}", e),
                }
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            _ => {
                let crossterm_event = crossterm::event::Event::Key(event);
                match rat_text::text_area::handle_events(&mut self.state, true, &crossterm_event) {
                    TextOutcome::Continue | TextOutcome::Unchanged => (AsyncTask::new_no_op(), None),
                    _ => (AsyncTask::new_no_op(), None),
                }
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(80, 80, area);
        frame.render_widget(Clear, popup_area);
        let block = Block::default()
            .title(" Config Editor (Ctrl+s: Save, Esc: Cancel) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        let text = TextArea::new()
            .style(Style::default().fg(Color::White));
        frame.render_stateful_widget(text, chunks[0], &mut self.state);
        let hint = Paragraph::new("Ctrl+s: Save | Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
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
