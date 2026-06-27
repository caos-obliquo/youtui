use crate::app::component::actionhandler::ComponentEffect;
use vi_text_editor::{ViMode, ViTextEditor};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use std::borrow::Cow;

pub struct NotesPopup {
    pub editor: ViTextEditor,
    pub notes_path: std::path::PathBuf,
    command_mode: bool,
    command_editor: ViTextEditor,
}

impl_youtui_component!(NotesPopup);

impl NotesPopup {
    pub fn new(notes_path: std::path::PathBuf, content: String) -> Self {
        let mut editor = ViTextEditor::new_multiline();
        editor.set_text(&content);
        editor.cursor = 0; // start at first line
        editor.mode = ViMode::Normal;
        Self {
            editor,
            notes_path,
            command_mode: false,
            command_editor: ViTextEditor::new(),
        }
    }

    pub fn mode_char(&self) -> Cow<'static, str> {
        if self.command_mode { Cow::Borrowed(": ") } else { self.editor.mode_char() }
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

    fn execute_command(&mut self, cmd: &str) -> (ComponentEffect<Self>, Option<AppCallback>) {
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        match parts.first().copied().unwrap_or("") {
            "w" => { self.save(); (AsyncTask::new_no_op(), None) }
            "wq" => { self.save(); (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)) }
            "q" | "q!" => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if self.command_mode {
            match event.code {
                KeyCode::Esc => {
                    self.command_mode = false;
                    self.command_editor.clear();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Enter => {
                    let cmd = self.command_editor.get_text().trim().to_string();
                    self.command_mode = false;
                    self.command_editor.clear();
                    if !cmd.is_empty() {
                        return self.execute_command(&cmd);
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                _ => {
                    self.command_editor.handle_key(event.code, event.modifiers.contains(KeyModifiers::SHIFT), false);
                    return (AsyncTask::new_no_op(), None);
                }
            }
        }

        match event.code {
            KeyCode::Esc => {
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
            KeyCode::Char(':') if self.editor.mode == ViMode::Normal => {
                self.command_mode = true;
                self.command_editor = ViTextEditor::new();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('y') if self.editor.mode == ViMode::VisualLine || self.editor.mode == ViMode::VisualChar => {
                self.editor.handle_key(event.code, false, false);
                let text = self.editor.get_clipboard();
                if !text.is_empty() {
                    crate::app::structures::copy_to_clipboard(&text);
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('y') if self.editor.mode == ViMode::VisualBlock => {
                self.editor.handle_key(event.code, false, false);
                let text = self.editor.get_clipboard();
                if !text.is_empty() {
                    crate::app::structures::copy_to_clipboard(&text);
                }
                (AsyncTask::new_no_op(), None)
            }
            _ => {
                let before_line = self.editor.cursor_line();
                let before_mode = self.editor.mode.clone();
                self.editor.handle_key(event.code, event.modifiers.contains(KeyModifiers::SHIFT), event.modifiers.contains(KeyModifiers::CONTROL));
                let after_line = self.editor.cursor_line();
                if matches!(event.code, crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Char('k')) {
                    tracing::info!(
                        "notes j/k: before_line={}, after_line={}, mode={:?}, multiline={}",
                        before_line, after_line, before_mode, self.editor.multiline
                    );
                }
                (AsyncTask::new_no_op(), None)
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(85, 85, area);
        frame.render_widget(Clear, popup_area);
        let mode = self.mode_char();
        let block = Block::default()
            .title(format!(" Notes {mode} "))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let [text_area, footer_area] = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);

        if self.command_mode {
            let display = self.editor.render_simple("");
            frame.render_widget(
                Paragraph::new(display).style(Style::default().fg(Color::White)).wrap(Wrap { trim: false }),
                text_area,
            );
            let cmd_display = self.command_editor.render_simple(":");
            frame.render_widget(
                Paragraph::new(format!(":{}", cmd_display.trim_start_matches(':')))
                    .style(Style::default().fg(Color::Cyan))
                    .alignment(Alignment::Left),
                footer_area,
            );
        } else {
            let cur_line = self.editor.cursor_line();
            let cur_col = self.editor.cursor_col();
            let total_lines = self.editor.get_text().split('\n').count();
            let visible_rows = text_area.height.saturating_sub(1).max(1) as usize;

            // Calculate scroll offset: keep cursor in the middleish of screen
            let ideal = cur_line.saturating_sub(visible_rows / 2);
            let scroll_offset = ideal.min(total_lines.saturating_sub(visible_rows));

            // Take visible slice of lines
            let all_lines: Vec<&str> = self.editor.get_text().split('\n').collect();
            let visible_slice: &[&str] = if scroll_offset < all_lines.len() {
                &all_lines[scroll_offset..all_lines.len().min(scroll_offset + visible_rows)]
            } else {
                &[]
            };

            let line_num_width = (total_lines.max(1) as f64).log10().floor() as usize + 1;
            let visual_range = self.editor.visual_line_range();
            let block_range = self.editor.visual_block_range();

            let mut lines: Vec<ratatui::text::Line> = Vec::new();
            for (display_idx, line_text) in visible_slice.iter().enumerate() {
                let abs_line = scroll_offset + display_idx;
                let is_cursor = abs_line == cur_line;
                let line_num = format!("{:>width$} ", abs_line + 1, width = line_num_width);

                if let Some((top, left, bot, right)) = block_range {
                    if abs_line >= top && abs_line <= bot {
                        let cols = left.min(right);
                        let cole = right.max(left);
                        let before = &line_text[..cols.min(line_text.len())];
                        let mid_start = cols.min(line_text.len());
                        let mid_end = cole.min(line_text.len());
                        let mid = &line_text[mid_start..mid_end];
                        let after = &line_text[mid_end..];
                        lines.push(ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                            ratatui::text::Span::styled(before.to_string(), Style::default().fg(Color::White)),
                            ratatui::text::Span::styled(mid.to_string(), Style::default().fg(Color::White).bg(Color::Rgb(0x00, 0x5f, 0x5f))),
                            ratatui::text::Span::styled(after.to_string(), Style::default().fg(Color::White)),
                        ]));
                        continue;
                    }
                }

                let selected = visual_range.map_or(false, |(s, e)| abs_line >= s && abs_line <= e);
                let bg = if selected { Color::Rgb(0x00, 0x5f, 0x5f) } else { ratatui::style::Color::default() };

                if is_cursor {
                    let (before, after) = line_text.split_at(cur_col.min(line_text.len()));
                    let after_rest = after.chars().next().map(|c| &after[c.len_utf8()..]).unwrap_or(after);
                    // Show character under cursor with inverted colors (Black on White)
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                        ratatui::text::Span::styled(before.to_string(), Style::default().fg(Color::White).bg(bg)),
                        ratatui::text::Span::styled(
                            after.chars().next().map(|c| c.to_string()).unwrap_or_else(|| " ".to_string()),
                            Style::default().fg(Color::Black).bg(Color::White),
                        ),
                        ratatui::text::Span::styled(after_rest.to_string(), Style::default().fg(Color::White).bg(bg)),
                    ]));
                } else {
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                        ratatui::text::Span::styled(line_text.to_string(), Style::default().fg(Color::White).bg(bg)),
                    ]));
                }
            }

            // Show empty-state hint when buffer is empty
            let is_empty = self.editor.get_text().trim().is_empty();
            if is_empty {
                lines.push(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(
                        "  ~ Notes file is empty - press i to start typing",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }

            frame.render_widget(
                Paragraph::new(lines).wrap(Wrap { trim: false }),
                text_area,
            );

            // Footer: cursor position + key hints
            let pos_info = format!("Ln {}, Col {}", cur_line + 1, cur_col + 1);
            frame.render_widget(
                Paragraph::new(format!(
                    "{} | :w Save | :wq Save+Quit | :q Quit | Enter URL | i Insert | j/k Navigate",
                    pos_info
                ))
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Left),
                footer_area,
            );
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
