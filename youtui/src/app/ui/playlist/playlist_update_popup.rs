use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use ytmapi_rs::common::VideoID;
use ytmapi_rs::parse::LibraryPlaylist;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistUpdatePopupAction {
    MoveUp,
    MoveDown,
    Select,
    Cancel,
}

impl Action for PlaylistUpdatePopupAction {
    fn context(&self) -> Cow<'_, str> {
        "Playlist Update Popup".into()
    }

    fn describe(&self) -> Cow<'_, str> {
        match self {
            PlaylistUpdatePopupAction::MoveUp => "Move Up",
            PlaylistUpdatePopupAction::MoveDown => "Move Down",
            PlaylistUpdatePopupAction::Select => "Select Playlist",
            PlaylistUpdatePopupAction::Cancel => "Cancel",
        }
        .into()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum PlaylistUpdatePopupState {
    Loading,
    Loaded(Vec<LibraryPlaylist>),
    Error(String),
}

pub struct PlaylistUpdatePopup {
    video_ids: Vec<VideoID<'static>>,
    pub state: PlaylistUpdatePopupState,
    pub selected_idx: usize,
    list_state: ListState,
    search_text: String,
    search_active: bool,
    filtered_indices: Vec<usize>,
}

impl_youtui_component!(PlaylistUpdatePopup);

impl PlaylistUpdatePopup {
    pub fn refresh_filter(&mut self) {
        match &self.state {
            PlaylistUpdatePopupState::Loaded(playlists) => {
                if self.search_text.is_empty() || !self.search_active {
                    self.filtered_indices = (0..playlists.len()).collect();
                } else {
                    let lower = self.search_text.to_lowercase();
                    self.filtered_indices = playlists.iter().enumerate()
                        .filter(|(_, p)| p.title.to_lowercase().contains(&lower))
                        .map(|(i, _)| i)
                        .collect();
                }
            }
            _ => self.filtered_indices = Vec::new(),
        }
    }

    fn selected_playlist<'a>(&self, playlists: &'a [LibraryPlaylist]) -> Option<&'a LibraryPlaylist> {
        self.filtered_indices.get(self.selected_idx).and_then(|&i| playlists.get(i))
    }
}

impl ActionHandler<PlaylistUpdatePopupAction> for PlaylistUpdatePopup {
    fn apply_action(
        &mut self,
        action: PlaylistUpdatePopupAction,
    ) -> impl Into<YoutuiEffect<Self>> {
        use PlaylistUpdatePopupAction::*;

        let result: (ComponentEffect<Self>, Option<AppCallback>) = match action {
            MoveUp => {
                if self.selected_idx > 0 {
                    self.selected_idx -= 1;
                }
                self.list_state.select(Some(self.selected_idx));
                (AsyncTask::new_no_op(), None)
            }
            MoveDown => {
                let max = self.filtered_indices.len().saturating_sub(1);
                if self.selected_idx < max {
                    self.selected_idx += 1;
                }
                self.list_state.select(Some(self.selected_idx));
                (AsyncTask::new_no_op(), None)
            }
            Select => {
                if let PlaylistUpdatePopupState::Loaded(playlists) = &self.state {
                    self.list_state.select(Some(self.selected_idx));
                    if let Some(playlist) = self.selected_playlist(playlists) {
                        let playlist_id = playlist.playlist_id.clone();
                        if self.video_ids.is_empty() {
                            // Load mode: fetch playlist tracks into player
                            (
                                AsyncTask::new_no_op(),
                                Some(AppCallback::LoadPlaylistFromPopup(playlist_id)),
                            )
                        } else {
                            // Add to existing playlist mode
                            let video_ids = self.video_ids.clone();
                            (
                                AsyncTask::new_no_op(),
                                Some(AppCallback::AddVideosToPlaylistFromPopup {
                                    playlist_id,
                                    video_ids,
                                }),
                            )
                        }
                    } else {
                        (AsyncTask::new_no_op(), None)
                    }
                } else {
                    (AsyncTask::new_no_op(), None)
                }
            }
            Cancel => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
        };

        result
    }
}

impl PlaylistUpdatePopup {
    pub fn new(video_ids: Vec<VideoID<'static>>) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            video_ids,
            state: PlaylistUpdatePopupState::Loading,
            selected_idx: 0,
            list_state,
            search_text: String::new(),
            search_active: false,
            filtered_indices: Vec::new(),
        }
    }

    fn filtered_playlists(&self) -> Vec<&LibraryPlaylist> {
        match &self.state {
            PlaylistUpdatePopupState::Loaded(playlists) => {
                if self.search_text.is_empty() || !self.search_active {
                    playlists.iter().collect()
                } else {
                    let lower = self.search_text.to_lowercase();
                    playlists.iter().filter(|p| p.title.to_lowercase().contains(&lower)).collect()
                }
            }
            _ => Vec::new(),
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        use crossterm::event::KeyCode;
        if self.search_active {
            match event.code {
                KeyCode::Esc => {
                    self.search_active = false;
                    self.search_text.clear();
                    self.refresh_filter();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Enter => {
                    self.search_active = false;
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Char(c) => {
                    self.search_text.push(c);
                    self.refresh_filter();
                    return (AsyncTask::new_no_op(), None);
                }
                KeyCode::Backspace => {
                    self.search_text.pop();
                    self.refresh_filter();
                    return (AsyncTask::new_no_op(), None);
                }
                _ => {}
            }
        }
        match event.code {
            KeyCode::Esc => {
                return (
                    AsyncTask::new_no_op(),
                    Some(AppCallback::ClosePopup),
                );
            }
            KeyCode::Char('/') => {
                self.search_active = true;
                self.search_text.clear();
                self.refresh_filter();
                return (AsyncTask::new_no_op(), None);
            }
            KeyCode::Enter => {
                let effect: YoutuiEffect<Self> =
                    self.apply_action(PlaylistUpdatePopupAction::Select).into();
                return (effect.effect, effect.callback);
            }
            crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                let effect: YoutuiEffect<Self> =
                    self.apply_action(PlaylistUpdatePopupAction::MoveUp).into();
                return (effect.effect, effect.callback);
            }
            crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                let effect: YoutuiEffect<Self> =
                    self.apply_action(PlaylistUpdatePopupAction::MoveDown).into();
                return (effect.effect, effect.callback);
            }
            _ => {}
        }
        (AsyncTask::new_no_op(), None)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(60, 60, area);
        frame.render_widget(Clear, popup_area);
        self.draw_list(frame, popup_area);
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

    fn draw_list(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.video_ids.is_empty() {
            " Load YouTube Music Playlist ".to_string()
        } else if self.search_active {
            format!(" Search Playlists [{}] ", self.search_text)
        } else {
            format!(" Select Playlist ({} songs) /:Search ", self.video_ids.len())
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(block.inner(area));

        frame.render_widget(block, area);

        match &self.state {
            PlaylistUpdatePopupState::Loading => {
                let loading = Paragraph::new("Loading playlists...")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Gray));
                frame.render_widget(loading, chunks[0]);
            }
            PlaylistUpdatePopupState::Loaded(playlists) => {
                let filtered: Vec<&LibraryPlaylist> = if self.search_active || !self.search_text.is_empty() {
                    let lower = self.search_text.to_lowercase();
                    playlists.iter().filter(|p| p.title.to_lowercase().contains(&lower)).collect()
                } else {
                    playlists.iter().collect()
                };
                let items: Vec<ListItem> = filtered
                    .iter()
                    .map(|p| {
                        let content = format!(" {} ", p.title);
                        ListItem::new(content)
                    })
                    .collect();

                let list = List::new(items)
                    .highlight_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(" ");
                frame.render_stateful_widget(list, chunks[0], &mut self.list_state);
            }
            PlaylistUpdatePopupState::Error(msg) => {
                let error = Paragraph::new(msg.as_str())
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Red));
                frame.render_widget(error, chunks[0]);
            }
        }

        let instructions = Paragraph::new("/: Search | j/k: Navigate | Enter: Select | Esc: Cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(instructions, chunks[1]);
    }
}
