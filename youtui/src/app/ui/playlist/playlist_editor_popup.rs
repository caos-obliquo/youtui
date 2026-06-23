use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::structures::ListSong;
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use ytmapi_rs::common::{VideoID, PlaylistID};
use vi_text_editor::ViTextEditor;

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
    pub delete_mode: bool,
    pub yank_mode: bool,
    pub pending_count: usize,
    pub undo_stack: Vec<Vec<ListSong>>,
    pub redo_stack: Vec<Vec<ListSong>>,
    pub yank_buffer: Vec<ListSong>,
    pub visual_mode: bool,
    pub visual_start: usize,
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
            delete_mode: false,
            yank_mode: false,
            pending_count: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            yank_buffer: Vec::new(),
            visual_mode: false,
            visual_start: 0,
        }
    }

    fn save_state(&mut self) {
        self.undo_stack.push(self.tracks.clone());
        self.redo_stack.clear();
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    fn remove_range(&mut self, start: usize, end: usize) {
        let end = end.min(self.tracks.len());
        if start >= end { return; }
        self.save_state();
        for _ in start..end {
            self.tracks.remove(start);
        }
        self.cursor = start.min(self.tracks.len().saturating_sub(1));
        self.modified = true;
    }

    fn delete_cursor_to_start(&mut self) {
        if self.cursor == 0 { return; }
        self.save_state();
        for _ in 0..self.cursor {
            self.tracks.remove(0);
        }
        self.cursor = 0;
        self.modified = true;
    }

    fn delete_cursor_to_end(&mut self) {
        if self.cursor >= self.tracks.len().saturating_sub(1) { return; }
        self.save_state();
        let len = self.tracks.len();
        for _ in self.cursor..len {
            self.tracks.remove(self.cursor);
        }
        self.modified = true;
    }

    fn run_yank_op(&mut self, mode: &str, count: usize) {
        match mode {
            "line" => {
                let end = (self.cursor + count).min(self.tracks.len());
                self.yank_buffer = self.tracks[self.cursor..end].to_vec();
            }
            "down" => {
                let end = (self.cursor + count).min(self.tracks.len());
                if end > self.cursor {
                    self.yank_buffer = self.tracks[self.cursor + 1..=end].to_vec();
                }
            }
            "up" => {
                let start = self.cursor.saturating_sub(count);
                if self.cursor > start {
                    self.yank_buffer = self.tracks[start..self.cursor].to_vec();
                }
            }
            "to_start" => {
                if self.cursor > 0 {
                    self.yank_buffer = self.tracks[0..self.cursor].to_vec();
                }
            }
            "to_end" => {
                if self.cursor + 1 < self.tracks.len() {
                    self.yank_buffer = self.tracks[self.cursor + 1..].to_vec();
                }
            }
            _ => {}
        }
    }

    fn insert_paste(&mut self, above: bool) {
        if self.yank_buffer.is_empty() { return; }
        self.save_state();
        let pos = if above {
            self.cursor
        } else {
            (self.cursor + 1).min(self.tracks.len())
        };
        for (i, song) in self.yank_buffer.iter().enumerate() {
            self.tracks.insert(pos + i, song.clone());
        }
        self.cursor = if above {
            pos
        } else {
            pos + self.yank_buffer.len().saturating_sub(1)
        };
        self.modified = true;
    }

    fn get_visual_range(&self) -> (usize, usize) {
        let start = self.visual_start.min(self.cursor);
        let end = self.visual_start.max(self.cursor);
        (start, end)
    }

    fn visual_delete(&mut self) {
        let (start, end) = self.get_visual_range();
        self.visual_mode = false;
        self.remove_range(start, end + 1);
    }

    fn visual_yank(&mut self) {
        let (start, end) = self.get_visual_range();
        self.yank_buffer = self.tracks[start..=end].to_vec();
        self.visual_mode = false;
    }

    pub fn mode_char(&self) -> String {
        if self.command_mode {
            ": ".to_string()
        } else if self.visual_mode {
            format!("[V{}]", if self.pending_count > 0 { self.pending_count.to_string() } else { String::new() })
        } else if self.delete_mode {
            format!("[DELETE{}]", if self.pending_count > 0 { format!(" {}", self.pending_count) } else { String::new() })
        } else if self.yank_mode {
            format!("[YANK{}]", if self.pending_count > 0 { format!(" {}", self.pending_count) } else { String::new() })
        } else if self.pending_count > 0 {
            format!("[{}]", self.pending_count)
        } else {
            "[N]".to_string()
        }
    }

    pub fn capacity_blocks(&self) -> String {
        const MAX_TRACKS: usize = 5000;
        const BLOCKS: usize = 4;
        let block_size = MAX_TRACKS / BLOCKS;
        let total = self.tracks.len();
        let mut s = String::from("Tracks: ");
        s.push_str(&format!("{}/{} ", total, MAX_TRACKS));
        for b in 0..BLOCKS {
            let filled = total.saturating_sub(b * block_size).min(block_size);
            let pct = filled as f64 / block_size as f64;
            let bar_len = 4;
            let filled_chars = (pct * bar_len as f64).round() as usize;
            s.push('[');
            for i in 0..bar_len {
                if i < filled_chars {
                    s.push('■');
                } else {
                    s.push('□');
                }
            }
            s.push(']');
            if b < BLOCKS - 1 {
                s.push(' ');
            }
        }
        s
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
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            "q!" => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
            "d" | "delete" => {
                if parts.len() >= 2 {
                    if let Ok(n) = parts[1].parse::<usize>() {
                        let idx = n.saturating_sub(1);
                        if idx < self.tracks.len() {
                            self.save_state();
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
                            self.save_state();
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

        // Visual mode: motions extend selection, d/y act on selection
        if self.visual_mode {
            match event.code {
                KeyCode::Esc | KeyCode::Char('V') => {
                    self.visual_mode = false;
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let c = self.pending_count.max(1);
                    self.pending_count = 0;
                    let max = self.tracks.len().saturating_sub(1);
                    self.cursor = (self.cursor + c).min(max);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let c = self.pending_count.max(1);
                    self.pending_count = 0;
                    self.cursor = self.cursor.saturating_sub(c);
                }
                KeyCode::Char('g') => {
                    self.pending_count = 0;
                    self.cursor = 0;
                }
                KeyCode::Char('G') => {
                    self.pending_count = 0;
                    self.cursor = self.tracks.len().saturating_sub(1);
                }
                KeyCode::Char('d') | KeyCode::Char('x') => {
                    self.visual_delete();
                }
                KeyCode::Char('y') => {
                    self.visual_yank();
                }
                KeyCode::Char('p') => {
                    self.visual_yank();
                    self.insert_paste(false);
                }
                KeyCode::Char('P') => {
                    self.visual_yank();
                    self.insert_paste(true);
                }
                _ => {}
            }
            return (AsyncTask::new_no_op(), None);
        }

        // Operator modes (d, y) - waiting for motion
        if self.delete_mode {
            match event.code {
                KeyCode::Char('0') | KeyCode::Char('1') | KeyCode::Char('2') | KeyCode::Char('3')
                    | KeyCode::Char('4') | KeyCode::Char('5') | KeyCode::Char('6')
                    | KeyCode::Char('7') | KeyCode::Char('8') | KeyCode::Char('9') => {
                    if let Some(d) = event.code.to_string().parse::<usize>().ok() {
                        self.pending_count = self.pending_count * 10 + d;
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                _ => {}
            }
            self.delete_mode = false;
            let c = self.pending_count.max(1);
            self.pending_count = 0;
            match event.code {
                KeyCode::Char('d') => self.remove_range(self.cursor, self.cursor + c),
                KeyCode::Char('j') | KeyCode::Down => self.remove_range(self.cursor, self.cursor + c),
                KeyCode::Char('k') | KeyCode::Up => {
                    let start = self.cursor.saturating_sub(c);
                    self.remove_range(start, self.cursor);
                }
                KeyCode::Char('g') => self.delete_cursor_to_start(),
                KeyCode::Char('G') => self.delete_cursor_to_end(),
                KeyCode::Esc => {}
                _ => {}
            }
            return (AsyncTask::new_no_op(), None);
        }

        if self.yank_mode {
            match event.code {
                KeyCode::Char('0') | KeyCode::Char('1') | KeyCode::Char('2') | KeyCode::Char('3')
                    | KeyCode::Char('4') | KeyCode::Char('5') | KeyCode::Char('6')
                    | KeyCode::Char('7') | KeyCode::Char('8') | KeyCode::Char('9') => {
                    if let Some(d) = event.code.to_string().parse::<usize>().ok() {
                        self.pending_count = self.pending_count * 10 + d;
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                _ => {}
            }
            self.yank_mode = false;
            let c = self.pending_count.max(1);
            self.pending_count = 0;
            match event.code {
                KeyCode::Char('y') => self.run_yank_op("line", c),
                KeyCode::Char('j') | KeyCode::Down => self.run_yank_op("down", c),
                KeyCode::Char('k') | KeyCode::Up => self.run_yank_op("up", c),
                KeyCode::Char('g') => self.run_yank_op("to_start", 0),
                KeyCode::Char('G') => self.run_yank_op("to_end", 0),
                KeyCode::Esc => {}
                _ => {}
            }
            return (AsyncTask::new_no_op(), None);
        }

        // Normal mode
        match event.code {
            KeyCode::Esc => {
                if self.visual_mode {
                    self.visual_mode = false;
                } else if !self.modified {
                    return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
                }
            }
            KeyCode::Char('q') => {
                return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char(':') => {
                self.command_mode = true;
                self.command_editor.clear();
            }
            KeyCode::Char('0') | KeyCode::Char('1') | KeyCode::Char('2') | KeyCode::Char('3')
                | KeyCode::Char('4') | KeyCode::Char('5') | KeyCode::Char('6')
                | KeyCode::Char('7') | KeyCode::Char('8') | KeyCode::Char('9') => {
                if let Some(d) = event.code.to_string().parse::<usize>().ok() {
                    self.pending_count = self.pending_count * 10 + d;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                let max = self.tracks.len().saturating_sub(1);
                self.cursor = (self.cursor + c).min(max);
                self.scroll_offset = self.scroll_offset.max(self.cursor.saturating_sub(10));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                self.cursor = self.cursor.saturating_sub(c);
                self.scroll_offset = self.scroll_offset.min(self.cursor);
            }
            KeyCode::Char('g') => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                self.cursor = c.saturating_sub(1).min(self.tracks.len().saturating_sub(1));
                self.scroll_offset = 0;
            }
            KeyCode::Char('G') => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                if c > 1 {
                    self.cursor = (c.saturating_sub(1)).min(self.tracks.len().saturating_sub(1));
                } else {
                    self.cursor = self.tracks.len().saturating_sub(1);
                }
            }
            KeyCode::Char('d') => {
                self.delete_mode = true;
            }
            KeyCode::Char('y') => {
                self.yank_mode = true;
            }
            KeyCode::Char('Y') => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                self.run_yank_op("line", c);
            }
            KeyCode::Char('D') => {
                self.pending_count = 0;
                self.delete_cursor_to_end();
            }
            KeyCode::Char('p') => {
                self.pending_count = 0;
                self.insert_paste(false);
            }
            KeyCode::Char('P') => {
                self.pending_count = 0;
                self.insert_paste(true);
            }
            KeyCode::Char('V') => {
                self.visual_mode = true;
                self.visual_start = self.cursor;
                self.pending_count = 0;
            }
            KeyCode::Char('J') => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                self.save_state();
                for _ in 0..c {
                    if self.cursor + 1 < self.tracks.len() {
                        self.tracks.swap(self.cursor, self.cursor + 1);
                        self.cursor += 1;
                        self.modified = true;
                    }
                }
            }
            KeyCode::Char('K') => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                self.save_state();
                for _ in 0..c {
                    if self.cursor > 0 {
                        self.tracks.swap(self.cursor, self.cursor - 1);
                        self.cursor -= 1;
                        self.modified = true;
                    }
                }
            }
            KeyCode::Char('u') => {
                if let Some(prev) = self.undo_stack.pop() {
                    self.redo_stack.push(self.tracks.clone());
                    self.tracks = prev;
                    self.cursor = self.cursor.min(self.tracks.len().saturating_sub(1));
                }
            }
            KeyCode::Char('o') => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                if self.tracks.is_empty() { return (AsyncTask::new_no_op(), None); }
                self.save_state();
                let blank = {
                    let mut s = self.tracks[self.cursor].clone();
                    s.title = String::new();
                    s
                };
                let pos = (self.cursor + 1).min(self.tracks.len());
                for _ in 0..c {
                    self.tracks.insert(pos, blank.clone());
                }
                self.cursor = pos + c.saturating_sub(1);
                self.modified = true;
            }
            KeyCode::Char('O') => {
                let c = self.pending_count.max(1);
                self.pending_count = 0;
                if self.tracks.is_empty() { return (AsyncTask::new_no_op(), None); }
                self.save_state();
                let blank = {
                    let mut s = self.tracks[self.cursor].clone();
                    s.title = String::new();
                    s
                };
                for _ in 0..c {
                    self.tracks.insert(self.cursor, blank.clone());
                }
                self.modified = true;
            }
            KeyCode::Char('/') => {
                tracing::info!("Playlist editor: search not yet implemented");
            }
            KeyCode::Char('E') => {
                let cb = self.save_tracks_callback();
                if cb.is_some() { self.modified = false; }
                return (AsyncTask::new_no_op(), cb);
            }
            _ => {}
        }
        (AsyncTask::new_no_op(), None)
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
            .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        let capacity_text = self.capacity_blocks();
        frame.render_widget(
            Paragraph::new(capacity_text)
                .style(Style::default().fg(Color::DarkGray)),
            chunks[0],
        );
        let visible = (chunks[1].height as usize).saturating_sub(1);
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
                let style = if self.visual_mode {
                    let (vs, ve) = self.get_visual_range();
                    let in_visual = i >= vs && i <= ve;
                    if i == self.cursor {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else if in_visual {
                        Style::default().fg(Color::White).bg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::White)
                    }
                } else if i == self.cursor {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                ratatui::text::Line::from(ratatui::text::Span::styled(line, style))
            })
            .collect();
        frame.render_widget(Paragraph::new(list_lines).wrap(Wrap { trim: false }), chunks[1]);
        let hint = if self.command_mode {
            let display = self.command_editor.render_simple(":");
            Paragraph::new(display)
                .style(Style::default().fg(Color::Yellow))
        } else if self.visual_mode {
            Paragraph::new("j/k: Extend | d/x: Delete | y: Yank | p/P: Paste | Esc: Exit Visual")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
        } else if self.delete_mode {
            Paragraph::new("j/k: Delete down/up | d: dd | g: top | G: bottom | Esc: Cancel")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
        } else if self.yank_mode {
            Paragraph::new("j/k: Yank down/up | y: yy | g: top | G: bottom | Esc: Cancel")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
        } else {
            let hint_text = if self.modified {
                "j/k: Move | gg/G: Jump | dd/y: Del/Yank | d+y+motion | p/P: Paste | u: Undo | J/K: Reorder | V: Visual | :: Cmd | q: Close [Modified]"
            } else {
                "j/k: Move | gg/G: Jump | dd/y: Del/Yank | d+y+motion | p/P: Paste | u: Undo | J/K: Reorder | V: Visual | :: Cmd | q: Close"
            };
            Paragraph::new(hint_text)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
        };
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
