use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::server::{
    ArcServer, TaskMetadata, RemovePlaylistItems, AddSongsToPlaylist,
};
use crate::app::structures::ListSong;
use crate::app::ui::AppCallback;
use async_callback_manager::{AsyncTask, FrontendEffect};
use crossterm::event::KeyCode;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use ytmapi_rs::common::{VideoID, PlaylistID, LikeStatus};
use vi_text_editor::{ViMode, ViTextEditor};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum PlaylistEditorAction {
    Close,
}

impl Action for PlaylistEditorAction {
    fn context(&self) -> Cow<'_, str> {
        "PlaylistEditor".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            PlaylistEditorAction::Close => "Close Playlist Editor",
        }
        .into()
    }
}

pub struct PlaylistEditorPopup {
    pub playlist_id: PlaylistID<'static>,
    pub playlist_title: String,
    pub tracks: Vec<ListSong>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub command_mode: bool,
    pub command_editor: ViTextEditor,
    pub modified: bool,
    pub confirm_delete: bool,
    pub sort_column: usize,
}

impl PlaylistEditorPopup {
    pub fn new(playlist_id: PlaylistID<'static>, playlist_title: String, tracks: Vec<ListSong>) -> Self {
        Self {
            playlist_id,
            playlist_title,
            tracks,
            cursor: 0,
            scroll_offset: 0,
            command_mode: false,
            command_editor: ViTextEditor::new(),
            modified: false,
            confirm_delete: false,
            sort_column: 0,
        }
    }

    pub fn mode_char(&self) -> &'static str {
        if self.command_mode { ": " } else { "[N]" }
    }

    fn save_tracks_callback(&self) -> Option<AppCallback> {
        let video_ids: Vec<VideoID<'static>> = self.tracks.iter()
            .map(|t| t.video_id.clone())
            .collect();
        if video_ids.is_empty() {
            return None;
        }
        Some(AppCallback::OpenPlaylistUpdatePopup(video_ids))
    }

    fn execute_command(&mut self, cmd: &str) -> (ComponentEffect<Self>, Option<AppCallback>) {
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        match parts.first().copied().unwrap_or("") {
            "w" => {
                self.modified = false;
                (AsyncTask::new_no_op(), self.save_tracks_callback())
            }
            "wq" => {
                self.modified = false;
                (AsyncTask::new_no_op(), self.save_tracks_callback())
            }
            "q" => {
                let cb = if self.modified { AppCallback::ClosePopup } else { AppCallback::ClosePopup };
                (AsyncTask::new_no_op(), Some(cb))
            }
            "q!" => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
            "d" | "delete" => {
                if parts.len() >= 2 {
                    if let Ok(n) = parts[1].parse::<usize>() {
                        let idx = n.saturating_sub(1);
                        if idx < self.tracks.len() {
                            self.tracks.remove(idx);
                            self.cursor = self.cursor.min(self.tracks.len().saturating_sub(1));
                            self.modified = true;
                        }
                    }
                }
                (AsyncTask::new_no_op(), None)
            }
            "m" | "move" => {
                if parts.len() >= 3 {
                    if let (Ok(from), Ok(to)) = (parts[1].parse::<usize>(), parts[2].parse::<usize>()) {
                        let fi = from.saturating_sub(1);
                        let ti = to.saturating_sub(1);
                        if fi < self.tracks.len() && ti < self.tracks.len() {
                            let song = self.tracks.remove(fi);
                            self.tracks.insert(ti, song);
                            self.cursor = ti;
                            self.modified = true;
                        }
                    }
                }
                (AsyncTask::new_no_op(), None)
            }
            "a" | "add" => {
                if parts.len() >= 2 {
                    let url = parts[1..].join(" ");
                    tracing::info!("Playlist editor: add URL: {}", url);
                }
                (AsyncTask::new_no_op(), None)
            }
            "rename" => {
                if parts.len() >= 2 {
                    let new_name = parts[1..].join(" ");
                    let pid = self.playlist_id.clone();
                    return (AsyncTask::new_no_op(), Some(AppCallback::RenamePlaylistFromLibrary {
                        playlist_id: pid,
                        new_title: new_name,
                    }));
                }
                (AsyncTask::new_no_op(), None)
            }
            "privacy" => {
                if parts.len() >= 2 {
                    use ytmapi_rs::query::playlist::PrivacyStatus;
                    let privacy = match parts[1] {
                        "public" => Some(PrivacyStatus::Public),
                        "private" => Some(PrivacyStatus::Private),
                        "unlisted" => Some(PrivacyStatus::Unlisted),
                        _ => None,
                    };
                    if let Some(privacy) = privacy {
                        let pid = self.playlist_id.clone();
                        return (AsyncTask::new_no_op(), Some(AppCallback::EditPlaylistDetailsFromLibrary {
                            playlist_id: pid,
                            title: None,
                            description: None,
                            privacy: Some(privacy),
                        }));
                    }
                }
                (AsyncTask::new_no_op(), None)
            }
            "rate" => {
                if parts.len() >= 2 {
                    let rating = match parts[1] {
                        "like" => Some(ytmapi_rs::common::LikeStatus::Liked),
                        "dislike" => Some(ytmapi_rs::common::LikeStatus::Disliked),
                        "none" => Some(ytmapi_rs::common::LikeStatus::Indifferent),
                        _ => None,
                    };
                    if let Some(rating) = rating {
                        let pid = self.playlist_id.clone();
                        return (AsyncTask::new_no_op(), Some(AppCallback::RatePlaylistFromLibrary(pid, rating)));
                    }
                }
                (AsyncTask::new_no_op(), None)
            }
            "h" | "help" => {
                tracing::info!("Commands: :w save, :wq save+quit, :q quit, :q! force quit, :d N delete, :m N M move, :rename <name>, :privacy public|private|unlisted, :rate like|dislike|none, :h help");
                (AsyncTask::new_no_op(), None)
            }
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
                    self.command_editor.handle_key(event.code, event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT), false);
                    return (AsyncTask::new_no_op(), None);
                }
            }
        }

        match event.code {
            KeyCode::Esc => {
                if !self.modified {
                    return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('q') => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char(':') => {
                self.command_mode = true;
                self.command_editor.clear();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let n = 1;
                let max = self.tracks.len().saturating_sub(1);
                self.cursor = (self.cursor + n).min(max);
                self.scroll_offset = self.scroll_offset.max(self.cursor.saturating_sub(10));
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let n = 1;
                self.cursor = self.cursor.saturating_sub(n);
                self.scroll_offset = self.scroll_offset.min(self.cursor);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('g') => {
                self.cursor = 0;
                self.scroll_offset = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('G') => {
                self.cursor = self.tracks.len().saturating_sub(1);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('d') if self.confirm_delete => {
                self.confirm_delete = false;
                if !self.tracks.is_empty() && self.cursor < self.tracks.len() {
                    self.tracks.remove(self.cursor);
                    self.cursor = self.cursor.min(self.tracks.len().saturating_sub(1));
                    self.modified = true;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('d') => {
                self.confirm_delete = true;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('J') => {
                if self.cursor + 1 < self.tracks.len() {
                    self.tracks.swap(self.cursor, self.cursor + 1);
                    self.cursor += 1;
                    self.modified = true;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('K') => {
                if self.cursor > 0 {
                    self.tracks.swap(self.cursor, self.cursor - 1);
                    self.cursor -= 1;
                    self.modified = true;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('/') => {
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('o') => {
                // Context menu: currently just o.E for save
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('E') => {
                // Save to existing playlist (same as :w)
                let cb = self.save_tracks_callback();
                if cb.is_some() { self.modified = false; }
                (AsyncTask::new_no_op(), cb)
            }
            KeyCode::Char('u') => {
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(90, 90, area);
        frame.render_widget(Clear, popup_area);
        let mode = self.mode_char();
        let title = format!(" Playlist Editor: \"{}\" {} ", self.playlist_title, mode);
        let block = Block::default()
            .title(title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        let visible = (chunks[0].height as usize).saturating_sub(1);
        let max_digits = self.tracks.len().max(1).to_string().len().max(2);
        let list_lines: Vec<ratatui::text::Line> = self.tracks.iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(visible)
            .map(|(i, song)| {
                let num = i + 1;
                let cursor_mark = if i == self.cursor { ">" } else { " " };
                let artist_str = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                let line = format!("{}{:>width$}  {:<40} {:<25} {}",
                    cursor_mark, num, song.title, artist_str, song.duration_string,
                    width = max_digits);
                let style = if i == self.cursor {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                ratatui::text::Line::from(ratatui::text::Span::styled(line, style))
            })
            .collect();
        frame.render_widget(Paragraph::new(list_lines).wrap(Wrap { trim: false }), chunks[0]);
        let hint = if self.command_mode {
            let display = self.command_editor.render_simple(":");
            Paragraph::new(display)
                .style(Style::default().fg(Color::Yellow))
        } else {
            let hint_text = if self.confirm_delete {
                "Press d again to confirm delete"
            } else if self.modified {
                "j/k: Move | dd: Delete | J/K: Reorder | :: Command | u: Undo | q: Close [Modified]"
            } else {
                "j/k: Move | dd: Delete | J/K: Reorder | :: Command | u: Undo | q: Close"
            };
            Paragraph::new(hint_text)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
        };
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

impl_youtui_component!(PlaylistEditorPopup);

impl ActionHandler<PlaylistEditorAction> for PlaylistEditorPopup {
    fn apply_action(&mut self, action: PlaylistEditorAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            PlaylistEditorAction::Close => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
        }
    }
}
