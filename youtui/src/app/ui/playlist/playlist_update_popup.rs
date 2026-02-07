use crate::app::component::actionhandler::{Action, ActionHandler, ComponentEffect, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyEvent};
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
    state: PlaylistUpdatePopupState,
    selected_idx: usize,
    list_state: ListState,
}

impl_youtui_component!(PlaylistUpdatePopup);

impl ActionHandler<PlaylistUpdatePopupAction> for PlaylistUpdatePopup {
    fn apply_action(&mut self, action: PlaylistUpdatePopupAction) -> impl Into<YoutuiEffect<Self>> {
        use PlaylistUpdatePopupAction::*;

        let result: (ComponentEffect<Self>, Option<AppCallback>) = match action {
            MoveUp => {
                if self.selected_idx > 0 {
                    self.selected_idx -= 1;
                    self.list_state.select(Some(self.selected_idx));
                }
                (AsyncTask::new_no_op(), None)
            }
            MoveDown => {
                if let PlaylistUpdatePopupState::Loaded(playlists) = &self.state {
                    if self.selected_idx < playlists.len().saturating_sub(1) {
                        self.selected_idx += 1;
                        self.list_state.select(Some(self.selected_idx));
                    }
                }
                (AsyncTask::new_no_op(), None)
            }
            Select => {
                if let PlaylistUpdatePopupState::Loaded(playlists) = &self.state {
                    if let Some(playlist) = playlists.get(self.selected_idx) {
                        let playlist_id = playlist.playlist_id.clone();
                        let video_ids = self.video_ids.clone();

                        tracing::info!(
                            "Popup: Requesting to add {} videos to playlist '{}'",
                            video_ids.len(),
                            playlist.title
                        );

                        (
                            AsyncTask::new_no_op(),
                            Some(AppCallback::AddVideosToPlaylistFromPopup {
                                playlist_id,
                                video_ids,
                            }),
                        )
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
        }
    }

    pub fn set_playlists(&mut self, playlists: Vec<LibraryPlaylist>) {
        self.state = PlaylistUpdatePopupState::Loaded(playlists);
        self.selected_idx = 0;
        if !self.playlists().is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn set_error(&mut self, message: String) {
        self.state = PlaylistUpdatePopupState::Error(message);
    }

    pub fn playlists(&self) -> &[LibraryPlaylist] {
        match &self.state {
            PlaylistUpdatePopupState::Loaded(playlists) => playlists,
            _ => &[],
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if event.code == KeyCode::Esc {
            return (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup));
        }

        if event.code == KeyCode::Enter {
            let effect: YoutuiEffect<Self> = self.apply_action(PlaylistUpdatePopupAction::Select).into();
            return (effect.effect, effect.callback);
        }

        match event.code {
            KeyCode::Char('k') | KeyCode::Up => {
                let effect: YoutuiEffect<Self> = self.apply_action(PlaylistUpdatePopupAction::MoveUp).into();
                return (effect.effect, effect.callback);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let effect: YoutuiEffect<Self> = self.apply_action(PlaylistUpdatePopupAction::MoveDown).into();
                return (effect.effect, effect.callback);
            }
            _ => {}
        }

        (AsyncTask::new_no_op(), None)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect_fixed(60, 70, area);
        frame.render_widget(Clear, popup_area);

        match &self.state {
            PlaylistUpdatePopupState::Loading => self.draw_loading(frame, popup_area),
            PlaylistUpdatePopupState::Error(msg) => self.draw_error(frame, popup_area, msg),
            PlaylistUpdatePopupState::Loaded(_) => self.draw_playlist_list(frame, popup_area),
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

    fn draw_loading(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Loading Playlists... ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let text = Paragraph::new("Fetching your library playlists from YouTube Music...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));

        frame.render_widget(text, inner);
    }

    fn draw_error(&self, frame: &mut Frame, area: Rect, msg: &str) {
        let block = Block::default()
            .title(" Error ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let text = Paragraph::new(format!("Failed to load playlists:\n{}", msg))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Red));

        frame.render_widget(text, inner);
    }

    fn draw_playlist_list(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(" Add {} Songs to Playlist ", self.video_ids.len());
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(2),
            ])
            .split(inner);

        let playlists = self.playlists();
        
        if playlists.is_empty() {
            let empty_msg = Paragraph::new("No playlists found in your library.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(empty_msg, chunks[0]);
        } else {
            let items: Vec<ListItem> = playlists
                .iter()
                .enumerate()
                .map(|(idx, playlist)| {
                    let content = if let Some(count) = &playlist.count {
                        format!("{} ({} songs)", playlist.title, count)
                    } else {
                        playlist.title.clone()
                    };

                    let style = if idx == self.selected_idx {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    ListItem::new(content).style(style)
                })
                .collect();

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );

            frame.render_stateful_widget(list, chunks[0], &mut self.list_state);
        }

        let instructions = "↑/↓ or j/k: Navigate | Enter: Add to Playlist | Esc: Cancel";
        let instructions_widget = Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(instructions_widget, chunks[1]);
    }
}
