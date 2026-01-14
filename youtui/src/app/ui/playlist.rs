use super::action::AppAction;
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, KeyRouter, Scrollable, TextHandler, YoutuiEffect,
};
use crate::app::server::song_downloader::DownloadProgressUpdateType;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use crate::app::server::{
    AutoplayDecodedSong, DecodeSong, DownloadSong, GetSongThumbnail, IncreaseVolume, Pause,
    PausePlay, PlayDecodedSong, QueueDecodedSong, Resume, Seek, SeekTo, Stop, StopAll,
    TaskMetadata,
};
use crate::app::structures::{
    AlbumArtState, BrowserSongsList, DownloadStatus, ListSong, ListSongDisplayableField,
    ListSongID, Percentage, PlayState, SongListComponent,
};
use crate::app::ui::playlist::effect_handlers::{
    HandleAllStopped, HandleAutoplayUpdateOk, HandleGetSongThumbnailError,
    HandleGetSongThumbnailOk, HandlePausePlayResponse, HandlePausedResponse, HandlePlayUpdateError,
    HandlePlayUpdateOk, HandleQueueUpdateOk, HandleResumeResponse, HandleSetSongPlayProgress,
    HandleSongDownloadProgressUpdate, HandleStopped, HandleVolumeUpdate,
};
use crate::app::ui::{AppCallback, WindowContext};
use crate::app::view::draw::{draw_loadable, draw_panel_mut, draw_table};
use crate::app::view::{BasicConstraint, DrawableMut, HasTitle, Loadable, TableView};
use crate::async_rodio_sink::{
    AllStopped, AutoplayUpdate, PlayUpdate, QueueUpdate, SeekDirection, Stopped, VolumeUpdate,
};
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::widgets::ScrollingTableState;
use async_callback_manager::{AsyncTask, Constraint, TryBackendTaskExt};
use crossterm::event::{KeyCode, KeyEvent};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ratatui::Frame;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter;
use std::option::Option;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{error, info, warn};
use ytmapi_rs::common::Thumbnail;

mod effect_handlers;
#[cfg(test)]
mod tests;

const SONGS_AHEAD_TO_BUFFER: usize = 3;
const SONGS_BEHIND_TO_SAVE: usize = 1;
const GAPLESS_PLAYBACK_THRESHOLD: Duration = Duration::from_secs(1);
pub const DEFAULT_UI_VOLUME: Percentage = Percentage(50);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueueState {
    NotQueued,
    Queued(ListSongID),
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    _cancel_token: Arc<tokio_util::sync::CancellationToken>,
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub list: BrowserSongsList,
    pub cur_played_dur: Option<Duration>,
    pub play_status: PlayState,
    pub queue_status: QueueState,
    pub volume: Percentage,
    cur_selected: usize,
    pub widget_state: ScrollingTableState,
    pub shuffle_enabled: bool,
    shuffle_indices: Vec<usize>,
    shuffle_seed: u64,
    active_downloads: Arc<std::sync::Mutex<Vec<(ListSongID, DownloadTask)>>>,
    pub search_enabled: bool,
    pub search_text: String,
    search_indices: Vec<usize>,
    pre_search_selected: usize,
}

impl_youtui_component!(Playlist);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistAction {
    ViewBrowser,
    PlaySelected,
    DeleteSelected,
    DeleteAll,
    ToggleShuffle,
    ToggleSearch,
    ClearSearch,
}

impl Action for PlaylistAction {
    fn context(&self) -> std::borrow::Cow<'_, str> {
        "Playlist".into()
    }

    fn describe(&self) -> std::borrow::Cow<'_, str> {
        match self {
            PlaylistAction::ViewBrowser => "View Browser",
            PlaylistAction::PlaySelected => "Play Selected",
            PlaylistAction::DeleteSelected => "Delete Selected",
            PlaylistAction::DeleteAll => "Delete All",
            PlaylistAction::ToggleShuffle => "Toggle Shuffle",
            PlaylistAction::ToggleSearch => "Toggle Search",
            PlaylistAction::ClearSearch => "Clear Search",
        }
        .into()
    }
}

impl ActionHandler<PlaylistAction> for Playlist {
    fn apply_action(&mut self, action: PlaylistAction) -> impl Into<YoutuiEffect<Playlist>> {
        match action {
            PlaylistAction::ViewBrowser => (AsyncTask::new_no_op(), Some(self.view_browser())),
            PlaylistAction::PlaySelected => (self.play_selected(), None),
            PlaylistAction::DeleteSelected => (self.delete_selected(), None),
            PlaylistAction::DeleteAll => (self.delete_all(), None),
            PlaylistAction::ToggleShuffle => (self.toggle_shuffle(), None),
            PlaylistAction::ToggleSearch => (self.toggle_search(), None),
            PlaylistAction::ClearSearch => (self.clear_search(), None),
        }
    }
}

impl KeyRouter<AppAction> for Playlist {
    fn get_all_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        self.get_active_keybinds(config)
    }

    fn get_active_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        std::iter::once(&config.keybinds.playlist)
    }
}

impl TextHandler for Playlist {
    fn is_text_handling(&self) -> bool {
        self.search_enabled
    }

    fn get_text(&self) -> std::option::Option<&str> {
        if self.search_enabled {
            Some(&self.search_text)
        } else {
            None
        }
    }

    fn replace_text(&mut self, text: impl Into<String>) {
        self.search_text = text.into();
        self.update_search_indices();
    }

    fn clear_text(&mut self) -> bool {
        if !self.search_text.is_empty() {
            self.search_text.clear();
            self.update_search_indices();
            true
        } else {
            false
        }
    }

    fn handle_text_event_impl(
        &mut self,
        event: &crossterm::event::Event,
    ) -> Option<ComponentEffect<Self>> {
        if !self.search_enabled {
            return None;
        }

        match event {
            crossterm::event::Event::Key(key_event) => match key_event.code {
                KeyCode::Char(c) => {
                    self.search_text.push(c);
                    self.update_search_indices();
                    self.cur_selected = self.cur_selected.min(self.get_max_visual_index());
                    Some(AsyncTask::new_no_op())
                }
                KeyCode::Backspace => {
                    if !self.search_text.is_empty() {
                        self.search_text.pop();
                        self.update_search_indices();
                        self.cur_selected = self.cur_selected.min(self.get_max_visual_index());
                        return Some(AsyncTask::new_no_op());
                    }
                    None
                }
                KeyCode::Esc | KeyCode::Enter => {
                    self.search_enabled = false;
                    Some(AsyncTask::new_no_op())
                }
                _ => None,
            },
            _ => None,
        }
    }
}

impl DrawableMut for Playlist {
    fn draw_mut_chunk(&mut self, f: &mut Frame, chunk: Rect, selected: bool, cur_tick: u64) {
        draw_panel_mut(f, self, chunk, selected, |t, f, chunk| {
            draw_loadable(f, t, chunk, |t, f, chunk| {
                Some(draw_table(f, t, chunk, cur_tick))
            })
        });
    }
}

impl Loadable for Playlist {
    fn is_loading(&self) -> bool {
        false
    }
}

impl Scrollable for Playlist {
    fn increment_list(&mut self, amount: isize) {
        let max_index = self.get_max_visual_index();
        self.cur_selected = self
            .cur_selected
            .saturating_add_signed(amount)
            .min(max_index);
    }

    fn is_scrollable(&self) -> bool {
        true
    }
}

impl TableView for Playlist {
    fn get_selected_item(&self) -> usize {
        self.cur_selected
    }

    fn get_state(&self) -> &ScrollingTableState {
        &self.widget_state
    }

    fn get_layout(&self) -> &[BasicConstraint] {
        &[
            BasicConstraint::Length(3),
            BasicConstraint::Length(6),
            BasicConstraint::Length(3),
            BasicConstraint::Percentage(Percentage(33)),
            BasicConstraint::Percentage(Percentage(33)),
            BasicConstraint::Percentage(Percentage(33)),
            BasicConstraint::Length(9),
            BasicConstraint::Length(4),
        ]
    }

    fn get_items(&self) -> impl ExactSizeIterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        let list_len = if !self.search_text.is_empty() {
            self.search_indices.len()
        } else {
            self.list.get_list_iter().count()
        };

        let cur_playing_visual = self
            .get_cur_playing_index()
            .and_then(|idx| self.actual_to_visual_index(idx));

        (0..list_len).map(move |visual_i| {
            let actual_i = self.visual_to_actual_index(visual_i);

            let ls = self
                .list
                .get_list_iter()
                .nth(actual_i)
                .expect("BUG: Visual index mapping failed");

            let first_field = if Some(visual_i) == cur_playing_visual {
                match self.play_status {
                    PlayState::NotPlaying => ">>>".to_string(),
                    PlayState::Playing(_) => "".to_string(),
                    PlayState::Paused(_) => "".to_string(),
                    PlayState::Stopped => ">>>".to_string(),
                    PlayState::Error(_) => ">>>".to_string(),
                    PlayState::Buffering(_) => "".to_string(),
                }
            } else {
                (visual_i + 1).to_string()
            };

            iter::once(first_field.to_string().into()).chain(ls.get_fields([
                ListSongDisplayableField::DownloadStatus,
                ListSongDisplayableField::TrackNo,
                ListSongDisplayableField::Artists,
                ListSongDisplayableField::Album,
                ListSongDisplayableField::Song,
                ListSongDisplayableField::Duration,
                ListSongDisplayableField::Year,
            ]))
        })
    }

    fn get_headings(&self) -> impl Iterator<Item = &'static str> {
        [
            "p#", "", "t#", "Artist", "Album", "Song", "Duration", "Year",
        ]
        .into_iter()
    }

    fn get_highlighted_row(&self) -> Option<usize> {
        self.get_cur_playing_index()
            .and_then(|idx| self.actual_to_visual_index(idx))
    }

    fn get_mut_state(&mut self) -> &mut ScrollingTableState {
        &mut self.widget_state
    }
}

impl HasTitle for Playlist {
    fn get_title(&self) -> Cow<'_, str> {
        let shuffle_indicator = if self.shuffle_enabled {
            " [SHUFFLE]"
        } else {
            ""
        };

        let search_indicator = if !self.search_text.is_empty() {
            format!(" [SEARCH: {}]", self.search_text)
        } else if self.search_enabled {
            " [SEARCH]".to_string()
        } else {
            "".to_string()
        };

        format!(
            "Local playlist - {} songs{}{}",
            self.list.get_list_iter().count(),
            shuffle_indicator,
            search_indicator
        )
        .into()
    }
}

impl SongListComponent for Playlist {
    fn get_song_from_idx(&self, idx: usize) -> Option<&ListSong> {
        self.list.get_list_iter().nth(idx)
    }
}

impl Playlist {
    pub fn new() -> (Self, ComponentEffect<Self>) {
        let task = AsyncTask::new_future_option(
            IncreaseVolume(0),
            HandleVolumeUpdate,
            Some(Constraint::new_block_same_type()),
        );

        let playlist = Playlist {
            volume: DEFAULT_UI_VOLUME,
            play_status: PlayState::NotPlaying,
            list: Default::default(),
            cur_played_dur: None,
            cur_selected: 0,
            queue_status: QueueState::NotQueued,
            widget_state: Default::default(),
            shuffle_enabled: false,
            shuffle_indices: Vec::new(),
            shuffle_seed: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            active_downloads: Arc::new(std::sync::Mutex::new(Vec::new())),
            search_enabled: false,
            search_text: String::new(),
            search_indices: Vec::new(),
            pre_search_selected: 0,
        };

        (playlist, task)
    }

    pub fn stop_song_id(&self, song_id: ListSongID) -> ComponentEffect<Self> {
        AsyncTask::new_future_option(
            Stop(song_id),
            HandleStopped,
            Some(Constraint::new_block_matching_metadata(
                TaskMetadata::PlayPause,
            )),
        )
    }

    pub fn play_song_id(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        self.drop_unscoped_from_id(id);
        self.cancel_out_of_scope_downloads();

        let mut effect = self.download_upcoming_from_id(id);

        self.cur_played_dur = None;

        if let Some(song_index) = self.get_index_from_id(id) {
            if let DownloadStatus::Downloaded(pointer) = &self
                .get_song_from_idx(song_index)
                .expect("Checked previously")
                .download_status
            {
                let task = DecodeSong(pointer.clone()).map_stream(PlayDecodedSong(id));
                let constraint = Some(Constraint::new_block_matching_metadata(
                    TaskMetadata::PlayingSong,
                ));
                let effect = effect.push(AsyncTask::new_stream_try(
                    task,
                    HandlePlayUpdateOk,
                    HandlePlayUpdateError(id),
                    constraint,
                ));
                self.play_status = PlayState::Playing(id);
                self.queue_status = QueueState::NotQueued;
                return effect;
            } else {
                let maybe_effect = self
                    .get_cur_playing_id()
                    .map(|cur_id| self.stop_song_id(cur_id));
                self.play_status = PlayState::Buffering(id);
                self.queue_status = QueueState::NotQueued;
                if let Some(stop_effect) = maybe_effect {
                    effect = effect.push(stop_effect);
                }
            }
        }
        effect
    }

    pub fn autoplay_song_id(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        self.drop_unscoped_from_id(id);
        self.cancel_out_of_scope_downloads();

        let mut effect = self.download_upcoming_from_id(id);

        self.cur_played_dur = None;

        if let Some(song_index) = self.get_index_from_id(id) {
            if let DownloadStatus::Downloaded(pointer) = &self
                .get_song_from_idx(song_index)
                .expect("Checked previously")
                .download_status
            {
                let task = DecodeSong(pointer.clone()).map_stream(AutoplayDecodedSong(id));
                let effect = effect.push(AsyncTask::new_stream_try(
                    task,
                    HandleAutoplayUpdateOk,
                    HandlePlayUpdateError(id),
                    None,
                ));
                self.play_status = PlayState::Playing(id);
                self.queue_status = QueueState::NotQueued;
                return effect;
            } else {
                let maybe_effect = self
                    .get_cur_playing_id()
                    .map(|cur_id| self.stop_song_id(cur_id));
                self.play_status = PlayState::Buffering(id);
                self.queue_status = QueueState::NotQueued;
                if let Some(stop_effect) = maybe_effect {
                    effect = effect.push(stop_effect);
                }
            }
        };
        effect
    }

    pub fn reset(&mut self) -> ComponentEffect<Self> {
        let mut effect = AsyncTask::new_no_op();

        if let Some(cur_id) = self.get_cur_playing_id() {
            effect = self.stop_song_id(cur_id);
        }

        self.cancel_all_downloads();
        self.clear();
        effect
    }

    pub fn clear(&mut self) {
        self.cur_played_dur = None;
        self.play_status = PlayState::NotPlaying;
        self.list.clear();
        self.shuffle_indices.clear();
        self.search_indices.clear();
        self.cur_selected = 0;
    }

    pub fn play_prev(&mut self) -> ComponentEffect<Self> {
        let cur = &self.play_status;
        match cur {
            PlayState::NotPlaying | PlayState::Stopped => {
                warn!("Asked to play prev, but not currently playing");
            }
            PlayState::Paused(id)
            | PlayState::Playing(id)
            | PlayState::Buffering(id)
            | PlayState::Error(id) => {
                if let Some(prev_song_id) = self.get_prev_song_id(*id) {
                    return self.play_song_id(prev_song_id);
                }
            }
        }
        AsyncTask::new_no_op()
    }

    pub fn handle_song_downloaded(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        if let PlayState::Buffering(target_id) = self.play_status
            && target_id == id
        {
            info!("Received downloaded song {id:?}, now trying to play it.");
            return self.play_song_id(id);
        }
        AsyncTask::new_no_op()
    }

    pub fn download_song_if_exists(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        let Some(song_index) = self.get_index_from_id(id) else {
            return AsyncTask::new_no_op();
        };

        let song = self
            .list
            .get_list_iter_mut()
            .nth(song_index)
            .expect("We got the index from the id, so song must exist");

        match song.download_status {
            DownloadStatus::Downloading(_)
            | DownloadStatus::Downloaded(_)
            | DownloadStatus::Queued => return AsyncTask::new_no_op(),
            _ => (),
        };

        let cancel_token = Arc::new(tokio_util::sync::CancellationToken::new());

        let effect = AsyncTask::new_stream(
            DownloadSong(song.video_id.clone(), id),
            HandleSongDownloadProgressUpdate,
            None,
        );

        let mut downloads = self.active_downloads.lock().unwrap();
        downloads.retain(|(song_id, _)| *song_id != id);
        downloads.push((
            id,
            DownloadTask {
                _cancel_token: cancel_token,
            },
        ));

        song.download_status = DownloadStatus::Queued;
        effect
    }

    pub fn increase_volume(&mut self, inc: i8) {
        self.volume.0 = self.volume.0.saturating_add_signed(inc).clamp(0, 100);
    }

    pub fn set_volume(&mut self, new_vol: u8) {
        self.volume.0 = new_vol.clamp(0, 100);
    }

    pub fn push_song_list(
        &mut self,
        mut song_list: Vec<ListSong>,
    ) -> (ListSongID, ComponentEffect<Self>) {
        let get_largest_thumbnails_url = |thumbs: &Vec<Thumbnail>| {
            thumbs
                .iter()
                .max_by_key(|thumbs| thumbs.height * thumbs.width)
                .map(|thumb| thumb.url.clone())
        };

        let albums = song_list
            .iter_mut()
            .filter_map(|song| {
                let Some(thumb_url) = get_largest_thumbnails_url(song.thumbnails.as_ref()) else {
                    song.album_art = AlbumArtState::None;
                    return None;
                };
                let thumbnail_id = SongThumbnailID::from(song as &ListSong).into_owned();
                Some((thumbnail_id, thumb_url))
            })
            .collect::<HashMap<SongThumbnailID, String>>();

        let mut effect: ComponentEffect<Self> = albums
            .into_iter()
            .map(|(thumbnail_id, thumbnail_url)| {
                AsyncTask::new_future_try(
                    GetSongThumbnail {
                        thumbnail_url,
                        thumbnail_id: thumbnail_id.clone(),
                    },
                    HandleGetSongThumbnailOk,
                    HandleGetSongThumbnailError(thumbnail_id),
                    None,
                )
            })
            .collect();

        let current_visual_pos = self
            .get_cur_playing_index()
            .and_then(|idx| self.actual_to_visual_index(idx));

        let first_id = self.list.push_song_list(song_list);

        if self.shuffle_enabled {
            self.generate_shuffle_indices();

            if let Some(visual_pos) = current_visual_pos {
                self.cur_selected =
                    visual_pos.min(self.list.get_list_iter().count().saturating_sub(1));
            }

            if let Some(current_id) = self.get_cur_playing_id() {
                self.drop_unscoped_from_id(current_id);
                effect = effect.push(self.download_upcoming_from_id(current_id));
            }
        }

        if !self.search_text.is_empty() {
            self.update_search_indices();
            self.cur_selected = self.cur_selected.min(self.get_max_visual_index());
        }

        (first_id, effect)
    }

    pub fn play_next_or_stop(&mut self, prev_id: ListSongID) -> ComponentEffect<Self> {
        let cur = &self.play_status;
        match cur {
            PlayState::NotPlaying | PlayState::Stopped => {
                warn!("Asked to play next, but not currently playing");
                AsyncTask::new_no_op()
            }
            PlayState::Paused(id)
            | PlayState::Playing(id)
            | PlayState::Buffering(id)
            | PlayState::Error(id) => {
                if id > &prev_id {
                    return AsyncTask::new_no_op();
                }

                if let Some(next_song_id) = self.get_next_song_id(*id) {
                    self.play_song_id(next_song_id)
                } else {
                    info!("No next song - finishing playback");
                    self.queue_status = QueueState::NotQueued;
                    self.stop_song_id(*id)
                }
            }
        }
    }

    pub fn autoplay_next_or_stop(&mut self, prev_id: ListSongID) -> ComponentEffect<Self> {
        let cur = &self.play_status;
        match cur {
            PlayState::NotPlaying | PlayState::Stopped => {
                warn!("Asked to play next, but not currently playing");
                AsyncTask::new_no_op()
            }
            PlayState::Paused(id)
            | PlayState::Playing(id)
            | PlayState::Buffering(id)
            | PlayState::Error(id) => {
                if id > &prev_id {
                    return AsyncTask::new_no_op();
                }

                if let Some(next_song_id) = self.get_next_song_id(*id) {
                    self.autoplay_song_id(next_song_id)
                } else {
                    info!("No next song - resetting play status");
                    self.queue_status = QueueState::NotQueued;
                    self.stop_song_id(*id)
                }
            }
        }
    }

    pub fn download_upcoming_from_id(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        let Some(song_index) = self.get_index_from_id(id) else {
            return AsyncTask::new_no_op();
        };

        let mut song_ids_list = Vec::new();
        song_ids_list.push(id);

        if self.shuffle_enabled {
            let Some(visual_index) = self.actual_to_visual_index(song_index) else {
                return AsyncTask::new_no_op();
            };

            for offset in 1..SONGS_AHEAD_TO_BUFFER {
                let next_visual = visual_index.saturating_add(offset);
                if next_visual < self.shuffle_indices.len() {
                    let next_actual = self.shuffle_indices[next_visual];
                    if let Some(next_id) = self.get_id_from_index(next_actual) {
                        song_ids_list.push(next_id);
                    }
                }
            }
        } else {
            for i in 1..SONGS_AHEAD_TO_BUFFER {
                if let Some(next_id) = self.get_song_from_idx(song_index + i).map(|s| s.id) {
                    song_ids_list.push(next_id);
                }
            }
        }

        song_ids_list
            .into_iter()
            .map(|song_id| self.download_song_if_exists(song_id))
            .collect()
    }

    pub fn drop_unscoped_from_id(&mut self, id: ListSongID) {
        let Some(song_index) = self.get_index_from_id(id) else {
            return;
        };

        let forward_limit = song_index.saturating_add(SONGS_AHEAD_TO_BUFFER);
        let backwards_limit = song_index.saturating_sub(SONGS_BEHIND_TO_SAVE);

        for (idx, song) in self.list.get_list_iter_mut().enumerate() {
            if idx < backwards_limit || idx >= forward_limit {
                song.download_status = DownloadStatus::None;
            }
        }
    }

    pub fn get_cur_playing_id(&self) -> Option<ListSongID> {
        match self.play_status {
            PlayState::Error(id)
            | PlayState::Playing(id)
            | PlayState::Paused(id)
            | PlayState::Buffering(id) => Some(id),
            PlayState::NotPlaying | PlayState::Stopped => None,
        }
    }

    pub fn get_cur_playing_song(&self) -> Option<&ListSong> {
        self.get_cur_playing_id()
            .and_then(|id| self.get_song_from_id(id))
    }

    pub fn get_next_song(&self) -> Option<&ListSong> {
        self.get_cur_playing_id()
            .and_then(|id| self.get_next_song_id(id))
            .and_then(|next_id| self.get_song_from_id(next_id))
    }

    pub fn get_index_from_id(&self, id: ListSongID) -> Option<usize> {
        self.list.get_list_iter().position(|s| s.id == id)
    }

    pub fn get_id_from_index(&self, index: usize) -> Option<ListSongID> {
        self.get_song_from_idx(index).map(|s| s.id)
    }

    pub fn get_mut_song_from_id(&mut self, id: ListSongID) -> Option<&mut ListSong> {
        self.list.get_list_iter_mut().find(|s| s.id == id)
    }

    pub fn get_song_from_id(&self, id: ListSongID) -> Option<&ListSong> {
        self.list.get_list_iter().find(|s| s.id == id)
    }

    pub fn check_id_is_cur(&self, check_id: ListSongID) -> bool {
        self.get_cur_playing_id().is_some_and(|id| id == check_id)
    }

    pub fn get_cur_playing_index(&self) -> Option<usize> {
        self.get_cur_playing_id()
            .and_then(|id| self.get_index_from_id(id))
    }
}

impl Playlist {
    pub async fn handle_tick(&mut self) {}

    pub fn handle_seek(
        &mut self,
        duration: Duration,
        direction: SeekDirection,
    ) -> ComponentEffect<Self> {
        AsyncTask::new_future_option(
            Seek {
                duration,
                direction,
            },
            HandleSetSongPlayProgress,
            None,
        )
    }

    pub fn handle_seek_to(&mut self, position: Duration) -> ComponentEffect<Self> {
        let id = match self.play_status {
            PlayState::Playing(id) => {
                self.play_status = PlayState::Paused(id);
                id
            }
            PlayState::Paused(id) => {
                self.play_status = PlayState::Playing(id);
                id
            }
            _ => return AsyncTask::new_no_op(),
        };

        AsyncTask::new_future_option(SeekTo { position, id }, HandleSetSongPlayProgress, None)
    }

    pub fn handle_next(&mut self) -> ComponentEffect<Self> {
        match self.play_status {
            PlayState::NotPlaying | PlayState::Stopped => {
                warn!("Asked to play next, but not currently playing");
                AsyncTask::new_no_op()
            }
            PlayState::Paused(id)
            | PlayState::Playing(id)
            | PlayState::Buffering(id)
            | PlayState::Error(id) => self.play_next_or_stop(id),
        }
    }

    pub fn handle_previous(&mut self) -> ComponentEffect<Self> {
        self.play_prev()
    }

    pub fn pauseplay(&mut self) -> ComponentEffect<Self> {
        let id = match self.play_status {
            PlayState::Playing(id) => {
                self.play_status = PlayState::Paused(id);
                id
            }
            PlayState::Paused(id) => {
                self.play_status = PlayState::Playing(id);
                id
            }
            _ => return AsyncTask::new_no_op(),
        };

        AsyncTask::new_future_option(
            PausePlay(id),
            HandlePausePlayResponse,
            Some(Constraint::new_block_matching_metadata(
                TaskMetadata::PlayPause,
            )),
        )
    }

    pub fn resume(&mut self) -> ComponentEffect<Self> {
        let id = match self.play_status {
            PlayState::Paused(id) => {
                self.play_status = PlayState::Playing(id);
                id
            }
            _ => return AsyncTask::new_no_op(),
        };

        AsyncTask::new_future_option(
            Resume(id),
            HandleResumeResponse,
            Some(Constraint::new_block_matching_metadata(
                TaskMetadata::PlayPause,
            )),
        )
    }

    pub fn pause(&mut self) -> ComponentEffect<Self> {
        let id = match self.play_status {
            PlayState::Playing(id) => {
                self.play_status = PlayState::Paused(id);
                id
            }
            _ => return AsyncTask::new_no_op(),
        };

        AsyncTask::new_future_option(
            Pause(id),
            HandlePausedResponse,
            Some(Constraint::new_block_matching_metadata(
                TaskMetadata::PlayPause,
            )),
        )
    }

    pub fn stop(&mut self) -> ComponentEffect<Self> {
        self.play_status = PlayState::Stopped;
        AsyncTask::new_future_option(
            StopAll,
            HandleAllStopped,
            Some(Constraint::new_block_matching_metadata(
                TaskMetadata::PlayPause,
            )),
        )
    }

    pub fn play_selected(&mut self) -> ComponentEffect<Self> {
        let actual_index = self.visual_to_actual_index(self.cur_selected);
        let Some(id) = self.get_id_from_index(actual_index) else {
            return AsyncTask::new_no_op();
        };
        self.play_song_id(id)
    }

    pub fn delete_selected(&mut self) -> ComponentEffect<Self> {
        let mut return_task = AsyncTask::new_no_op();
        let actual_index = self.visual_to_actual_index(self.cur_selected);

        let list_len = self.list.get_list_iter().count();
        if actual_index >= list_len {
            error!(
                "Attempted to delete invalid index {} (list len {})",
                actual_index, list_len
            );
            return return_task;
        }

        if let Some(cur_playing_id) = self.get_cur_playing_id() {
            if Some(actual_index) == self.get_cur_playing_index() {
                self.play_status = PlayState::NotPlaying;
                return_task = self.stop_song_id(cur_playing_id);
            }
        }

        if self.shuffle_enabled {
            if let Some(pos_in_shuffle) =
                self.shuffle_indices.iter().position(|&i| i == actual_index)
            {
                self.shuffle_indices.remove(pos_in_shuffle);
            }
            for idx in &mut self.shuffle_indices {
                if *idx > actual_index {
                    *idx = idx.saturating_sub(1);
                }
            }
        }

        if self.search_enabled {
            if let Some(pos_in_search) = self.search_indices.iter().position(|&i| i == actual_index)
            {
                self.search_indices.remove(pos_in_search);
            }
            for idx in &mut self.search_indices {
                if *idx > actual_index {
                    *idx = idx.saturating_sub(1);
                }
            }
            self.update_search_indices();
        }

        self.list.remove_song_index(actual_index);

        let new_max = self.get_max_visual_index();
        if self.cur_selected > new_max {
            self.cur_selected = new_max;
        }

        return_task
    }

    pub fn delete_all(&mut self) -> ComponentEffect<Self> {
        self.reset()
    }

    pub fn view_browser(&mut self) -> AppCallback {
        AppCallback::ChangeContext(WindowContext::Browser)
    }

    pub fn toggle_shuffle(&mut self) -> ComponentEffect<Self> {
        self.shuffle_enabled = !self.shuffle_enabled;

        if self.shuffle_enabled {
            self.shuffle_seed = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            self.generate_shuffle_indices();

            if self.get_cur_playing_id().is_some() {
                self.cur_selected = 0;
            }
        } else {
            self.shuffle_indices.clear();
            if let Some(playing_idx) = self.get_cur_playing_index() {
                self.cur_selected =
                    playing_idx.min(self.list.get_list_iter().count().saturating_sub(1));
            }
        }

        self.regenerate_downloads_for_current()
    }

    fn generate_shuffle_indices(&mut self) {
        let len = self.list.get_list_iter().count();
        if len == 0 {
            self.shuffle_indices.clear();
            return;
        }

        let mut indices: Vec<usize> = (0..len).collect();

        let mut rng = StdRng::seed_from_u64(self.shuffle_seed);
        for i in (1..len).rev() {
            let j = rng.random_range(0..=i);
            indices.swap(i, j);
        }

        if let Some(current_index) = self.get_cur_playing_index() {
            if let Some(pos) = indices.iter().position(|&i| i == current_index) {
                indices.swap(0, pos);
            }
        }

        self.shuffle_indices = indices;
    }

    pub fn toggle_search(&mut self) -> ComponentEffect<Self> {
        self.search_enabled = !self.search_enabled;

        if self.search_enabled {
            self.pre_search_selected = self.cur_selected;
            self.search_text.clear();
            self.update_search_indices();
        } else {
            let _ = self.clear_search();
            self.cur_selected = self.pre_search_selected.min(self.get_max_visual_index());
        }

        AsyncTask::new_no_op()
    }

    pub fn clear_search(&mut self) -> ComponentEffect<Self> {
        self.search_text.clear();
        self.update_search_indices();
        self.cur_selected = self.cur_selected.min(self.get_max_visual_index());
        AsyncTask::new_no_op()
    }

    fn update_search_indices(&mut self) {
        let search_lower = self.search_text.to_lowercase();

        if search_lower.is_empty() {
            self.search_indices = (0..self.list.get_list_iter().count()).collect();
            return;
        }

        self.search_indices = self
            .list
            .get_list_iter()
            .enumerate()
            .filter_map(|(actual_idx, song)| {
                let title = song
                    .get_fields([ListSongDisplayableField::Song])
                    .into_iter()
                    .next()
                    .unwrap_or_default()
                    .to_lowercase();

                let album = song
                    .get_fields([ListSongDisplayableField::Album])
                    .into_iter()
                    .next()
                    .unwrap_or_default()
                    .to_lowercase();

                let artist = song
                    .get_fields([ListSongDisplayableField::Artists])
                    .into_iter()
                    .next()
                    .unwrap_or_default()
                    .to_lowercase();

                let searchable = format!("{} {} {}", title, album, artist);

                if searchable.contains(&search_lower) {
                    Some(actual_idx)
                } else {
                    None
                }
            })
            .collect();
    }

    fn visual_to_actual_index(&self, visual_index: usize) -> usize {
        let list_len = self.list.get_list_iter().count();
        if list_len == 0 {
            return 0;
        }

        let base_index = if !self.search_text.is_empty() && !self.search_indices.is_empty() {
            self.search_indices
                .get(visual_index)
                .copied()
                .unwrap_or_else(|| {
                    if self.search_indices.is_empty() {
                        0
                    } else {
                        self.search_indices[self.search_indices.len() - 1]
                    }
                })
        } else {
            visual_index.min(list_len - 1)
        };

        if self.shuffle_enabled
            && base_index < self.shuffle_indices.len()
            && !self.shuffle_indices.is_empty()
        {
            self.shuffle_indices[base_index]
        } else {
            base_index.min(list_len - 1)
        }
    }

    fn actual_to_visual_index(&self, actual_index: usize) -> Option<usize> {
        let shuffled_pos = if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices
                .iter()
                .position(|&i| i == actual_index)?
        } else {
            actual_index
        };

        if !self.search_text.is_empty() && !self.search_indices.is_empty() {
            self.search_indices.iter().position(|&i| i == shuffled_pos)
        } else {
            Some(shuffled_pos)
        }
    }

    fn get_next_song_id(&self, current_id: ListSongID) -> Option<ListSongID> {
        let current_actual_index = self.get_index_from_id(current_id)?;

        let current_shuffled_pos = if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices
                .iter()
                .position(|&i| i == current_actual_index)?
        } else {
            current_actual_index
        };

        let current_visual_pos = if !self.search_text.is_empty() && !self.search_indices.is_empty()
        {
            self.search_indices
                .iter()
                .position(|&i| i == current_shuffled_pos)?
        } else {
            current_shuffled_pos
        };

        let next_visual_pos = current_visual_pos.saturating_add(1);

        let next_shuffled_pos = if !self.search_text.is_empty() && !self.search_indices.is_empty() {
            self.search_indices.get(next_visual_pos).copied()?
        } else {
            next_visual_pos
        };

        let next_actual_index = if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices.get(next_shuffled_pos).copied()?
        } else {
            next_shuffled_pos
        };

        self.get_id_from_index(next_actual_index)
    }

    fn get_prev_song_id(&self, current_id: ListSongID) -> Option<ListSongID> {
        let current_actual_index = self.get_index_from_id(current_id)?;

        let current_shuffled_pos = if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices
                .iter()
                .position(|&i| i == current_actual_index)?
        } else {
            current_actual_index
        };

        let current_visual_pos = if !self.search_text.is_empty() && !self.search_indices.is_empty()
        {
            self.search_indices
                .iter()
                .position(|&i| i == current_shuffled_pos)?
        } else {
            current_shuffled_pos
        };

        if current_visual_pos == 0 {
            return None;
        }

        let prev_visual_pos = current_visual_pos.saturating_sub(1);

        let prev_shuffled_pos = if !self.search_text.is_empty() && !self.search_indices.is_empty() {
            self.search_indices.get(prev_visual_pos).copied()?
        } else {
            prev_visual_pos
        };

        let prev_actual_index = if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices.get(prev_shuffled_pos).copied()?
        } else {
            prev_shuffled_pos
        };

        self.get_id_from_index(prev_actual_index)
    }

    fn get_max_visual_index(&self) -> usize {
        let base_count = self.list.get_list_iter().count();

        if base_count == 0 {
            return 0;
        }

        if !self.search_text.is_empty() {
            if self.search_indices.is_empty() {
                0
            } else {
                self.search_indices.len() - 1
            }
        } else if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices.len() - 1
        } else {
            base_count - 1
        }
    }

    fn cancel_all_downloads(&self) {
        let mut downloads = self.active_downloads.lock().unwrap();
        downloads.clear();
    }

    fn cancel_out_of_scope_downloads(&self) {
        let current_scope = self.get_current_download_scope();
        let mut downloads = self.active_downloads.lock().unwrap();

        downloads.retain(|(song_id, _task)| {
            current_scope
                .iter()
                .any(|(scope_id, _)| *scope_id == *song_id)
        });
    }

    fn regenerate_downloads_for_current(&mut self) -> ComponentEffect<Self> {
        if let Some(current_id) = self.get_cur_playing_id() {
            self.drop_unscoped_from_id(current_id);
            self.cancel_out_of_scope_downloads();
            self.download_upcoming_from_id(current_id)
        } else {
            AsyncTask::new_no_op()
        }
    }

    fn get_current_download_scope(&self) -> Vec<(ListSongID, DownloadTask)> {
        let mut scope = Vec::new();

        if let Some(current_id) = self.get_cur_playing_id() {
            if let Some(song_index) = self.get_index_from_id(current_id) {
                let forward_limit = song_index.saturating_add(SONGS_AHEAD_TO_BUFFER);
                let backwards_limit = song_index.saturating_sub(SONGS_BEHIND_TO_SAVE);

                let downloads = self.active_downloads.lock().unwrap();
                for (idx, song) in self.list.get_list_iter().enumerate() {
                    if idx >= backwards_limit && idx < forward_limit {
                        if let Some(task) = downloads.iter().find(|(id, _)| *id == song.id) {
                            scope.push(task.clone());
                        }
                    }
                }
            }
        }

        scope
    }

    pub fn handle_song_download_progress_update(
        &mut self,
        update: DownloadProgressUpdateType,
        id: ListSongID,
    ) -> ComponentEffect<Self> {
        if let Some(song) = self.get_song_from_id(id) {
            match song.download_status {
                DownloadStatus::None | DownloadStatus::Downloaded(_) | DownloadStatus::Failed => {
                    return AsyncTask::new_no_op();
                }
                _ => (),
            }
        } else {
            return AsyncTask::new_no_op();
        }

        let mut effect = AsyncTask::new_no_op();

        match update {
            DownloadProgressUpdateType::Started => {
                if let Some(song) = self.list.get_list_iter_mut().find(|x| x.id == id) {
                    song.download_status = DownloadStatus::Queued;
                }
            }
            DownloadProgressUpdateType::Completed(song_buf) => {
                if let Some(s) = self.get_mut_song_from_id(id) {
                    s.download_status = DownloadStatus::Downloaded(Arc::new(song_buf));
                }
                self.active_downloads
                    .lock()
                    .unwrap()
                    .retain(|(song_id, _)| *song_id != id);
                effect = self.handle_song_downloaded(id);
            }
            DownloadProgressUpdateType::Error => {
                if let Some(song) = self.list.get_list_iter_mut().find(|x| x.id == id) {
                    song.download_status = DownloadStatus::Failed;
                }
                self.active_downloads
                    .lock()
                    .unwrap()
                    .retain(|(song_id, _)| *song_id != id);
            }
            DownloadProgressUpdateType::Retrying { times_retried } => {
                if let Some(song) = self.list.get_list_iter_mut().find(|x| x.id == id) {
                    song.download_status = DownloadStatus::Retrying { times_retried };
                }
            }
            DownloadProgressUpdateType::Downloading(p) => {
                if let Some(song) = self.list.get_list_iter_mut().find(|x| x.id == id) {
                    song.download_status = DownloadStatus::Downloading(p);
                }
            }
        }

        effect
    }

    pub fn handle_volume_update(&mut self, response: VolumeUpdate) {
        self.volume = Percentage(response.0.into())
    }

    pub fn handle_play_update(&mut self, update: PlayUpdate<ListSongID>) -> ComponentEffect<Self> {
        match update {
            PlayUpdate::PlayProgress(duration, id) => {
                return self.handle_set_song_play_progress(duration, id);
            }
            PlayUpdate::Playing(duration, id) => self.handle_playing(duration, id),
            PlayUpdate::DonePlaying(id) => return self.handle_done_playing(id),
            PlayUpdate::Error(e) => error!("{e}"),
        }
        AsyncTask::new_no_op()
    }

    pub fn handle_queue_update(
        &mut self,
        update: QueueUpdate<ListSongID>,
    ) -> ComponentEffect<Self> {
        match update {
            QueueUpdate::PlayProgress(duration, id) => {
                return self.handle_set_song_play_progress(duration, id);
            }
            QueueUpdate::Queued(duration, id) => self.handle_queued(duration, id),
            QueueUpdate::DonePlaying(id) => return self.handle_done_playing(id),
            QueueUpdate::Error(e) => error!("{e}"),
        }
        AsyncTask::new_no_op()
    }

    pub fn handle_autoplay_update(
        &mut self,
        update: AutoplayUpdate<ListSongID>,
    ) -> ComponentEffect<Self> {
        match update {
            AutoplayUpdate::PlayProgress(duration, id) => {
                return self.handle_set_song_play_progress(duration, id);
            }
            AutoplayUpdate::Playing(duration, id) => self.handle_playing(duration, id),
            AutoplayUpdate::DonePlaying(id) => return self.handle_done_playing(id),
            AutoplayUpdate::AutoplayQueued(id) => self.handle_autoplay_queued(id),
            AutoplayUpdate::Error(e) => error!("{e}"),
        }
        AsyncTask::new_no_op()
    }

    pub fn handle_set_song_play_progress(
        &mut self,
        d: Duration,
        id: ListSongID,
    ) -> ComponentEffect<Self> {
        if !self.check_id_is_cur(id) {
            return AsyncTask::new_no_op();
        }

        self.cur_played_dur = Some(d);

        if let Some(duration_dif) = {
            let cur_dur = self
                .get_cur_playing_song()
                .and_then(|song| song.actual_duration);
            self.cur_played_dur
                .as_ref()
                .zip(cur_dur)
                .map(|(d1, d2)| d2.saturating_sub(*d1))
        } {
            if duration_dif
                .saturating_sub(GAPLESS_PLAYBACK_THRESHOLD)
                .is_zero()
                && !matches!(self.queue_status, QueueState::Queued(_))
                && let Some(next_song) = self.get_next_song()
                && let DownloadStatus::Downloaded(song) = &next_song.download_status
            {
                let task = DecodeSong(song.clone()).map_stream(QueueDecodedSong(id));
                info!("Queuing up song!");
                let effect = AsyncTask::new_stream_try(
                    task,
                    HandleQueueUpdateOk,
                    HandlePlayUpdateError(id),
                    None,
                );
                self.queue_status = QueueState::Queued(next_song.id);
                return effect;
            }
        }

        AsyncTask::new_no_op()
    }

    pub fn handle_done_playing(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        if self.queue_status == QueueState::Queued(id) {
            self.queue_status = QueueState::NotQueued;
            return AsyncTask::new_no_op();
        }

        if !self.check_id_is_cur(id) {
            return AsyncTask::new_no_op();
        }

        self.autoplay_next_or_stop(id)
    }

    pub fn handle_queued(&mut self, duration: Option<Duration>, id: ListSongID) {
        if let Some(song) = self.get_mut_song_from_id(id) {
            song.actual_duration = duration;
        }
    }

    pub fn handle_autoplay_queued(&mut self, id: ListSongID) {
        if let QueueState::Queued(q_id) = self.queue_status {
            if id == q_id {
                self.queue_status = QueueState::NotQueued
            }
        }
    }

    pub fn handle_playing(&mut self, duration: Option<Duration>, id: ListSongID) {
        if let Some(song) = self.get_mut_song_from_id(id) {
            song.actual_duration = duration;
        }

        if let PlayState::Paused(p_id) = self.play_status
            && p_id == id
        {
            self.play_status = PlayState::Playing(id)
        }
    }

    pub fn handle_set_to_error(&mut self, id: ListSongID) {
        info!("Received message that song had a playback error {:?}", id);
        if self.check_id_is_cur(id) {
            info!("Setting song state to Error {:?}", id);
            self.play_status = PlayState::Error(id)
        }
    }

    pub fn handle_paused(&mut self, s_id: ListSongID) {
        if let PlayState::Playing(p_id) = self.play_status
            && p_id == s_id
        {
            self.play_status = PlayState::Paused(s_id)
        }
    }

    pub fn handle_resumed(&mut self, id: ListSongID) {
        if let PlayState::Paused(p_id) = self.play_status
            && p_id == id
        {
            self.play_status = PlayState::Playing(id)
        }
    }

    pub fn handle_stopped(&mut self, id: Stopped<ListSongID>) {
        let Stopped(id) = id;
        info!("Received message that playback {:?} has been stopped", id);
        if self.check_id_is_cur(id) {
            info!("Stopping {:?}", id);
            self.play_status = PlayState::Stopped
        }
    }

    pub fn handle_all_stopped(&mut self, _: AllStopped) {
        self.play_status = PlayState::Stopped
    }
}
