use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use vi_text_editor::{ViMode, ViTextEditor};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NotesAction {
    Close,
}

impl Action for NotesAction {
    fn context(&self) -> Cow<'_, str> {
        "Notes".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            NotesAction::Close => "Close",
        }
        .into()
    }
}

pub struct NotesPopup {
    pub editor: ViTextEditor,
    pub notes_path: std::path::PathBuf,
}

impl_youtui_component!(NotesPopup);

impl ActionHandler<NotesAction> for NotesPopup {
    fn apply_action(&mut self, action: NotesAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            NotesAction::Close => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
        }
    }
}

impl NotesPopup {
    pub fn new(notes_path: std::path::PathBuf, content: String) -> Self {
        let mut editor = ViTextEditor::new_multiline();
        editor.set_text(&content);
        Self { editor, notes_path }
    }

    pub fn mode_char(&self) -> &'static str {
        self.editor.mode_char()
    }

    fn save(&self) {
        match std::fs::write(&self.notes_path, self.editor.get_text()) {
            Ok(_) => tracing::info!("Notes saved to {:?}", self.notes_path),
            Err(e) => tracing::error!("Failed to save notes: {}", e),
        }
    }

    fn open_url_at_line(&self) -> Option<AppCallback> {
        let line = self.editor.cursor_line();
        let text = self.editor.get_text();
        let url = text.lines().nth(line)
            .map(|l| l.trim())
            .filter(|l| l.starts_with("http://") || l.starts_with("https://"))
            .map(|l| l.to_string());
        url.map(|u| AppCallback::OpenUrl(u))
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc => {
                if self.editor.mode != ViMode::Insert {
                    return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
                }
                self.editor.handle_key(KeyCode::Esc, false, false);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Enter => {
                if self.editor.mode == ViMode::Normal {
                    if let Some(callback) = self.open_url_at_line() {
                        return (AsyncTask::new_no_op(), Some(callback));
                    }
                }
                self.editor.handle_key(KeyCode::Enter, false, false);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('s') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save();
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            _ => {
                self.editor.handle_key(event.code, event.modifiers.contains(KeyModifiers::SHIFT), false);
                (AsyncTask::new_no_op(), None)
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(80, 80, area);
        frame.render_widget(Clear, popup_area);
        let mode = self.editor.mode_char();
        let block = Block::default()
            .title(format!(" Notes {mode} (Ctrl+s: Save, Esc: Cancel, Enter on URL to open) "))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        let display = self.editor.render_simple("");
        let text = Paragraph::new(display)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });
        frame.render_widget(text, chunks[0]);
        let hint = Paragraph::new("Ctrl+s: Save | Esc: Cancel | Enter on URL: Open | i: Insert | j/k: Navigate")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(hint, chunks[1]);
        let cur_col = self.editor.cursor_col() as u16;
        let cur_line = self.editor.cursor_line() as u16;
        frame.set_cursor_position((
            inner.x + 1 + cur_col,
            inner.y + 1 + cur_line,
        ));
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
