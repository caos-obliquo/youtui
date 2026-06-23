use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use vi_text_editor::{ViMode, ViTextEditor};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
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
    pub editor: ViTextEditor,
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
        let mut editor = ViTextEditor::new_multiline();
        editor.set_text(&content);
        Self { editor, config_path }
    }

    pub fn mode_char(&self) -> &'static str {
        self.editor.mode_char()
    }

    fn save(&self) {
        match std::fs::write(&self.config_path, self.editor.get_text()) {
            Ok(_) => tracing::info!("Config saved to {:?}", self.config_path),
            Err(e) => tracing::error!("Failed to save config: {}", e),
        }
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc => {
                if self.editor.mode != ViMode::Insert {
                    return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
                }
                // Esc in Insert mode → Normal
                self.editor.handle_key(KeyCode::Esc, false, false);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('s') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save();
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            _ => {
                // ZZ: save and quit
                if event.code == KeyCode::Char('Z')
                    && self.editor.mode == ViMode::Normal
                {
                    // Wait for next key via key stack? No — handle key chords here.
                    // We can't do ZZ in a single handle_key call.
                    // We'll handle it via the key_stack mechanism.
                }
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
            .title(format!(" Config Editor {mode} (Ctrl+s: Save, Esc: Cancel) "))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let [text_area, footer_area] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        let mark = self.editor.cursor_marker();
        let cur_line = self.editor.cursor_line();
        let cur_col = self.editor.cursor_col();
        let mut lines: Vec<ratatui::text::Line> = Vec::new();
        for (i, line_text) in self.editor.get_text().split('\n').enumerate() {
            if i == cur_line {
                let (before, after) = line_text.split_at(cur_col.min(line_text.len()));
                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(before.to_string(), Style::default().fg(Color::White)),
                    ratatui::text::Span::styled(mark.to_string(), Style::default().fg(Color::White).bg(Color::Rgb(0x00, 0x5f, 0x5f))),
                    ratatui::text::Span::styled(after.to_string(), Style::default().fg(Color::White)),
                ]));
            } else {
                lines.push(ratatui::text::Line::from(
                    ratatui::text::Span::styled(line_text.to_string(), Style::default().fg(Color::White)),
                ));
            }
        }
        frame.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            text_area,
        );
        let hint = Paragraph::new("Ctrl+s: Save | Esc: Cancel | i: Insert | h/j/k/l: Move")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(hint, footer_area);
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
