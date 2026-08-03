use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::structures::{ListSong, ListSongArtist, MaybeRc, ListSongAlbum, AlbumOrUploadAlbumID};
use crate::app::ui::AppCallback;
use ytmapi_rs::common::YoutubeID;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::rc::Rc;
use metadata_provider::genre_map;
use crate::app::structures::copy_to_clipboard;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SongInfoAction {
    Close,
}

impl Action for SongInfoAction {
    fn context(&self) -> Cow<'_, str> {
        "Song Info".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            SongInfoAction::Close => "Close",
        }
        .into()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Title,
    Artist,
    Album,
    Year,
    Genre,
}

const FIELDS: &[Field] = &[Field::Title, Field::Artist, Field::Album, Field::Year, Field::Genre];
const DISPLAY_LINES: usize = 8; // Title, Artist, Album, Year, Genre, Track, Time, ID

#[derive(Clone, Copy, PartialEq)]
enum PopupMode {
    Normal,
    VisualLine,
}

pub struct SongInfoPopup {
    pub song: ListSong,
    selected_field: usize,
    editing: bool,
    edit_buffer: String,
    genre_scroll: usize,
    mode: PopupMode,
    visual_start: usize,
}

impl_youtui_component!(SongInfoPopup);

impl ActionHandler<SongInfoAction> for SongInfoPopup {
    fn apply_action(&mut self, action: SongInfoAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            SongInfoAction::Close => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
        }
    }
}

impl SongInfoPopup {
    pub fn new(song: ListSong) -> Self {
        Self {
            song,
            selected_field: 0,
            editing: false,
            edit_buffer: String::new(),
            genre_scroll: 0,
            mode: PopupMode::Normal,
            visual_start: 0,
        }
    }

    fn visual_range(&self) -> (usize, usize) {
        let s = self.visual_start.min(self.selected_field);
        let e = self.visual_start.max(self.selected_field);
        (s, e)
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if self.editing {
            return self.handle_edit_key(event);
        }
        // q always closes, regardless of mode
        if event.code == KeyCode::Char('q') {
            return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
        }
        match self.mode {
            PopupMode::Normal => self.handle_normal_key(event),
            PopupMode::VisualLine => self.handle_visual_key(event),
        }
    }

    fn handle_normal_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char('e') => {
                self.editing = true;
                self.edit_buffer = self.field_value(self.selected_field);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('V') => {
                self.mode = PopupMode::VisualLine;
                self.visual_start = self.selected_field;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Tab => {
                self.selected_field = (self.selected_field + 1) % FIELDS.len();
                self.genre_scroll = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::BackTab => {
                self.selected_field = if self.selected_field == 0 { FIELDS.len() - 1 } else { self.selected_field - 1 };
                self.genre_scroll = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected_field == 4 {
                    let page = if event.modifiers.contains(KeyModifiers::CONTROL) { 10 } else { 1 };
                    self.genre_scroll = self.genre_scroll.saturating_add(page);
                } else {
                    self.selected_field = (self.selected_field + 1) % FIELDS.len();
                    self.genre_scroll = 0;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_field == 4 {
                    let page = if event.modifiers.contains(KeyModifiers::CONTROL) { 10 } else { 1 };
                    self.genre_scroll = self.genre_scroll.saturating_sub(page);
                } else {
                    self.selected_field = if self.selected_field == 0 { FIELDS.len() - 1 } else { self.selected_field - 1 };
                    self.genre_scroll = 0;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Home => {
                if self.selected_field == 4 {
                    self.genre_scroll = 0;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::End => {
                if self.selected_field == 4 {
                    self.genre_scroll = 9999;
                }
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    fn handle_visual_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc | KeyCode::Char('V') => {
                self.mode = PopupMode::Normal;
                self.selected_field = self.selected_field.min(FIELDS.len() - 1);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected_field = (self.selected_field + 1).min(DISPLAY_LINES - 1);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_field = self.selected_field.saturating_sub(1);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('y') => {
                let (vs, ve) = self.visual_range();
                let raw = self.build_display_lines();
                let yanked: Vec<&str> = raw[vs..=ve].iter().map(|l| l.as_str()).collect();
                copy_to_clipboard(&yanked.join("\n"));
                self.mode = PopupMode::Normal;
                self.selected_field = self.selected_field.min(FIELDS.len() - 1);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('d') => {
                let (vs, ve) = self.visual_range();
                for i in vs..=ve {
                    if i < FIELDS.len() {
                        self.clear_field(i);
                    }
                }
                self.selected_field = vs.min(FIELDS.len() - 1);
                self.mode = PopupMode::Normal;
                (AsyncTask::new_no_op(), Some(AppCallback::UpdateSongInfo {
                    id: self.song.id,
                    song: self.song.clone(),
                }))
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    fn clear_field(&mut self, idx: usize) {
        match FIELDS[idx] {
            Field::Title => self.song.title.clear(),
            Field::Artist => {
                self.song.artists = MaybeRc::Owned(Vec::new());
            }
            Field::Album => self.song.album = None,
            Field::Year => self.song.year = None,
            Field::Genre => {
                self.song.genres.clear();
                self.song.styles.clear();
            }
        }
    }

    fn build_display_lines(&self) -> Vec<String> {
        let artist = self.song.artists.iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let album = self.song.album.as_ref()
            .map(|a| a.name.as_str())
            .unwrap_or("-");
        let year = self.song.year.as_ref().map(|y| y.as_str()).unwrap_or("-");
        let track_no = self.song.track_no.map(|t| t.to_string()).unwrap_or_else(|| "-".to_string());
        let genre_str = self.build_genre_str();
        vec![
            format!("Title: {}", self.song.title),
            format!("Artist: {}", artist),
            format!("Album: {}", album),
            format!("Year: {}", year),
            format!("Genre: {}", genre_str),
            format!("Track: {}", track_no),
            format!("Time: {}", self.song.duration_string),
            format!("ID: {}", self.song.video_id.get_raw()),
        ]
    }

    fn build_genre_str(&self) -> String {
        let g: Vec<&str> = self.song.styles.iter().map(|s| s.as_str()).collect();
        if g.is_empty() || (g.len() == 1 && g[0].is_empty()) {
            let filtered: Vec<&str> = self.song.genres.iter().map(|s| s.as_str()).filter(|s| !s.is_empty()).collect();
            if filtered.is_empty() { "-".to_string() } else { filtered.join(", ") }
        } else {
            g.join(", ")
        }
    }

    fn handle_edit_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Enter => {
                self.commit_edit();
                self.editing = false;
                (AsyncTask::new_no_op(), Some(AppCallback::UpdateSongInfo {
                    id: self.song.id,
                    song: self.song.clone(),
                }))
            }
            KeyCode::Esc => {
                self.editing = false;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    fn field_value(&self, idx: usize) -> String {
        match FIELDS[idx] {
            Field::Title => self.song.title.clone(),
            Field::Artist => self.song.artists.iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            Field::Album => self.song.album.as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            Field::Year => self.song.year.as_ref()
                .map(|y| y.as_str().to_string())
                .unwrap_or_default(),
            Field::Genre => {
                let parts: Vec<&str> = {
                    let g: Vec<&str> = self.song.styles.iter().map(|s| s.as_str()).collect();
                    if g.is_empty() || (g.len() == 1 && g[0].is_empty()) {
                        self.song.genres.iter().map(|s| s.as_str()).filter(|s| !s.is_empty()).collect()
                    } else { g }
                };
                parts.join(", ")
            }
        }
    }

    fn commit_edit(&mut self) {
        let val = self.edit_buffer.trim().to_string();
        match FIELDS[self.selected_field] {
            Field::Title => self.song.title = val,
            Field::Artist => {
                self.song.artists = MaybeRc::Owned(
                    val.split(',')
                        .map(|s| ListSongArtist {
                            name: s.trim().to_string(),
                            id: None,
                        })
                        .collect()
                );
            }
            Field::Album => {
                if val.is_empty() {
                    self.song.album = None;
                } else {
                    self.song.album = Some(MaybeRc::Owned(ListSongAlbum {
                        name: val,
                        id: AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
                    }));
                }
            }
            Field::Year => {
                self.song.year = if val.is_empty() { None } else { Some(Rc::new(val)) };
            }
            Field::Genre => {
                let parts: Vec<String> = val.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                // Store both genres and styles (user intent wins)
                self.song.genres = parts.clone();
                self.song.styles = parts;
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(60, 50, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Song Info ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let is_genre_edit = self.editing && self.selected_field == 4 && !self.edit_buffer.is_empty();
        let constraints: &[Constraint] = if is_genre_edit {
            &[Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)]
        } else {
            &[Constraint::Min(1), Constraint::Length(1)]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        let artist = self.song.artists.iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let album = self.song.album.as_ref()
            .map(|a| a.name.as_str())
            .unwrap_or("-");
        let year = self.song.year.as_ref().map(|y| y.as_str()).unwrap_or("-");
        let track_no = self.song.track_no.map(|t| t.to_string()).unwrap_or_else(|| "-".to_string());
        let duration = &self.song.duration_string;
        let source = self.song.video_id.get_raw();
        let genre_list: Vec<&str> = {
            let g = self.song.styles.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            if g.is_empty() || (g.len() == 1 && g[0].is_empty()) {
                self.song.genres.iter().map(|s| s.as_str()).filter(|s| !s.is_empty()).collect()
            } else { g }
        };
        let genre_str = if genre_list.is_empty() {
            "-".to_string()
        } else {
            genre_list.join(", ")
        };
        let raw_lines = vec![
            ("Title", self.song.title.as_str()),
            ("Artist", &artist),
            ("Album", album),
            ("Year", year),
            ("Genre", genre_str.as_str()),
            ("Track", &track_no),
            ("Time", duration),
            ("ID", source),
        ];

        let (vs, ve) = if self.mode == PopupMode::VisualLine { self.visual_range() } else { (0, 0) };
        let mut display = String::new();
        for (i, (label, value)) in raw_lines.iter().enumerate() {
            let is_editable = i < FIELDS.len();
            let marker = match self.mode {
                PopupMode::VisualLine => {
                    let in_sel = i >= vs && i <= ve;
                    if i == self.selected_field && in_sel {
                        format!("\u{2588} {}: {}", label, value)
                    } else if in_sel {
                        format!("\u{2590} {}: {}", label, value)
                    } else {
                        format!("  {}: {}", label, value)
                    }
                }
                PopupMode::Normal => {
                    if is_editable && i == self.selected_field {
                        if self.editing {
                            format!("\u{258c} {}: {}\u{2588}", label, self.edit_buffer)
                        } else {
                            format!("\u{258c} {}: {}", label, value)
                        }
                    } else {
                        format!("  {}: {}", label, value)
                    }
                }
            };
            display.push_str(&marker);
            display.push('\n');

            // Just show the raw genre string from metadata. RYM tree expansion
            // adds noise — no per-song relevance signal available.
        }

        let info_widget = Paragraph::new(display)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false })
            .scroll((self.genre_scroll as u16, 0));
        frame.render_widget(info_widget, chunks[0]);

        // Genre auto-suggest: show matching canonical genres when editing Genre field
        if is_genre_edit {
            let all = genre_map::all_genres();
            let query = self.edit_buffer.to_lowercase();
            let last_word = query.split(',').last().unwrap_or("").trim().to_string();
            let matches: Vec<&String> = all.iter()
                .filter(|g| {
                    if last_word.is_empty() {
                        query.split(',').any(|w| {
                            let w = w.trim();
                            !w.is_empty() && g.to_lowercase().contains(w)
                        })
                    } else {
                        g.to_lowercase().contains(&last_word)
                    }
                })
                .take(5)
                .collect();
            if !matches.is_empty() {
                let suggest_text: String = matches.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" | ");
                let suggest_widget = Paragraph::new(suggest_text)
                    .style(Style::default().fg(Color::Cyan))
                    .wrap(Wrap { trim: false });
                frame.render_widget(suggest_widget, chunks[2]);
            }
        }

        let hint = match self.mode {
            PopupMode::VisualLine => "[V] j/k: Extend | y: Yank | d: Clear | Esc/V: Normal",
            PopupMode::Normal if self.editing => "Enter: Save | Esc: Cancel",
            PopupMode::Normal if self.selected_field == 4 => "j/k: Scroll | e: Edit | V: Visual | q: Close",
            PopupMode::Normal => "j/k: Select | e: Edit | V: Visual | Tab: Next | q: Close",
        };
        let hint_widget = Paragraph::new(hint)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(hint_widget, chunks[1]);
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
