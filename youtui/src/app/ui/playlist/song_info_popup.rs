use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::structures::{
    AlbumOrUploadAlbumID, ListSong, ListSongAlbum, ListSongArtist, MaybeRc,
};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use metadata_provider::genre_map;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashSet;
use std::rc::Rc;
use ytmapi_rs::common::YoutubeID;

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

const FIELDS: &[Field] = &[
    Field::Title,
    Field::Artist,
    Field::Album,
    Field::Year,
    Field::Genre,
];

pub struct SongInfoPopup {
    pub song: ListSong,
    selected_field: usize,
    editing: bool,
    edit_buffer: String,
}

impl_youtui_component!(SongInfoPopup);

impl ActionHandler<SongInfoAction> for SongInfoPopup {
    fn apply_action(&mut self, action: SongInfoAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            SongInfoAction::Close => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
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
        }
    }

    pub fn handle_key(
        &mut self,
        event: crossterm::event::KeyEvent,
    ) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if self.editing {
            return self.handle_edit_key(event);
        }
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char('e') => {
                self.editing = true;
                self.edit_buffer = self.field_value(self.selected_field);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Tab => {
                self.selected_field = (self.selected_field + 1) % FIELDS.len();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::BackTab => {
                self.selected_field = if self.selected_field == 0 {
                    FIELDS.len() - 1
                } else {
                    self.selected_field - 1
                };
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected_field = (self.selected_field + 1) % FIELDS.len();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_field = if self.selected_field == 0 {
                    FIELDS.len() - 1
                } else {
                    self.selected_field - 1
                };
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    fn handle_edit_key(
        &mut self,
        event: crossterm::event::KeyEvent,
    ) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Enter => {
                self.commit_edit();
                self.editing = false;
                (
                    AsyncTask::new_no_op(),
                    Some(AppCallback::UpdateSongInfo {
                        id: self.song.id,
                        song: self.song.clone(),
                    }),
                )
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
            Field::Artist => self
                .song
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            Field::Album => self
                .song
                .album
                .as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            Field::Year => self
                .song
                .year
                .as_ref()
                .map(|y| y.as_str().to_string())
                .unwrap_or_default(),
            Field::Genre => {
                let mut parts = self.song.styles.clone();
                if parts.is_empty() {
                    parts = self.song.genres.clone();
                }
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
                        .collect(),
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
                self.song.year = if val.is_empty() {
                    None
                } else {
                    Some(Rc::new(val))
                };
            }
            Field::Genre => {
                let parts: Vec<String> = val
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
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

        let is_genre_edit =
            self.editing && self.selected_field == 4 && !self.edit_buffer.is_empty();
        let constraints: &[Constraint] = if is_genre_edit {
            &[
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ]
        } else {
            &[Constraint::Min(1), Constraint::Length(1)]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        let artist = self
            .song
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let album = self
            .song
            .album
            .as_ref()
            .map(|a| a.name.as_str())
            .unwrap_or("-");
        let year = self.song.year.as_ref().map(|y| y.as_str()).unwrap_or("-");
        let track_no = self
            .song
            .track_no
            .map(|t| t.to_string())
            .unwrap_or_else(|| "-".to_string());
        let duration = &self.song.duration_string;
        let source = self.song.video_id.get_raw();
        let genre_str = {
            let g = self.song.styles.join(", ");
            if g.is_empty() {
                self.song.genres.join(", ")
            } else {
                g
            }
        };
        let genre_display = if genre_str.is_empty() {
            "-"
        } else {
            genre_str.as_str()
        };

        // Look up RYM genre descriptions
        let genre_descriptions: Vec<String> = if !genre_str.is_empty() {
            let mut seen = HashSet::new();
            genre_str
                .split(',')
                .map(|g| g.trim())
                .filter_map(|g| rym_genre_data::find_genre(g))
                .filter_map(|g| g.description.clone())
                .filter(|d| seen.insert(d.clone()))
                .map(|d| {
                    if d.len() > 120 {
                        format!("{}...", &d[..117])
                    } else {
                        d
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let raw_lines = [
            ("Title", self.song.title.as_str()),
            ("Artist", &artist),
            ("Album", album),
            ("Year", year),
            ("Genre", genre_display),
            ("Track", &track_no),
            ("Time", duration),
            ("ID", source),
        ];

        let mut display = String::new();
        for (i, (label, value)) in raw_lines.iter().enumerate() {
            let is_editable = i < FIELDS.len();
            let marker = if is_editable && i == self.selected_field {
                if self.editing {
                    format!("> {}: {}█", label, self.edit_buffer)
                } else {
                    format!("> {}: {}", label, value)
                }
            } else {
                format!("  {}: {}", label, value)
            };
            display.push_str(&marker);
            display.push('\n');
            // Show RYM descriptions when Genre field is selected
            if i == 4 && !genre_descriptions.is_empty() && self.selected_field == 4 {
                for desc in &genre_descriptions {
                    display.push_str(&format!("         {}\n", desc));
                }
            }
        }

        let info_widget = Paragraph::new(display)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });
        frame.render_widget(info_widget, chunks[0]);

        // Genre auto-suggest: show matching canonical genres when editing Genre field
        if is_genre_edit {
            let all = genre_map::all_genres();
            let query = self.edit_buffer.to_lowercase();
            let last_word = query
                .split(',')
                .next_back()
                .unwrap_or("")
                .trim()
                .to_string();
            let matches: Vec<&String> = all
                .iter()
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
                let suggest_text: String = matches
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ");
                let suggest_widget = Paragraph::new(suggest_text)
                    .style(Style::default().fg(Color::Cyan))
                    .wrap(Wrap { trim: false });
                frame.render_widget(suggest_widget, chunks[2]);
            }
        }

        let hint = if self.editing {
            "Enter: Save | Esc: Cancel"
        } else {
            "j/k: Select | e: Edit | Tab: Next | q: Close"
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
