use super::action::AppAction;
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, KeyRouter, Scrollable, TextHandler, YoutuiEffect,
};
use crate::app::queue_persistence;
use crate::app::server::song_downloader::DownloadProgressUpdateType;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use crate::app::server::{
    AutoplayDecodedSong, DecodeSong, DownloadSong, GetSongThumbnail, IncreaseVolume, Pause,
    PausePlay, PlayDecodedSong, QueueDecodedSong, Resume, Seek, SeekTo, Stop, StopAll,
    TaskMetadata,
};
use crate::app::structures::{
    AlbumArtState, AudioQuality, BrowserSongsList, DownloadStatus, ListSong, ListSongDisplayableField,
    ListSongID, Percentage, PlayState, SongListComponent, Thumbnail,
};
use std::collections::VecDeque;
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
use ytmapi_rs::common::VideoID;
use crossterm::event::KeyCode;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ratatui::Frame;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::iter;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, warn};
use ytmapi_rs::common::YoutubeID;

pub mod playlist_save_popup;
pub mod playlist_update_popup;
mod effect_handlers;
pub mod effect_handlers_playlist;
#[cfg(test)]
mod tests;

const SONGS_AHEAD_TO_BUFFER: usize = 2;
const SONGS_BEHIND_TO_SAVE: usize = 1;
const GAPLESS_PLAYBACK_THRESHOLD: Duration = Duration::from_secs(1);
pub const DEFAULT_UI_VOLUME: Percentage = Percentage(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueState {
    NotQueued,
    Queued(ListSongID),
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    cancel_token: Arc<tokio_util::sync::CancellationToken>,
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub list: BrowserSongsList,
    pub cur_played_dur: Option<Duration>,
    pub play_status: PlayState,
    pub queue_status: QueueState,
    pub volume: Percentage,
    pub audio_quality: AudioQuality,
    cur_selected: usize,
    pub widget_state: ScrollingTableState,
    pub shuffle_enabled: bool,
    shuffle_indices: Vec<usize>,
    shuffle_seed: u64,
    active_downloads: Arc<std::sync::Mutex<Vec<(ListSongID, DownloadTask)>>>,
    download_queue: VecDeque<ListSongID>,
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
    SaveQueue,
    LoadQueue,
    DeleteQueue,
    ClearSearch,
    SetBestQuality,
    SaveToNewPlaylist,
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
            PlaylistAction::SaveQueue => "Save Queue",
            PlaylistAction::LoadQueue => "Load Queue",
            PlaylistAction::DeleteQueue => "Delete Queue",
            PlaylistAction::SetBestQuality => "Set Best Quality",
            PlaylistAction::SaveToNewPlaylist => "Save Queue to New Playlist",
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
            PlaylistAction::SaveQueue => {
                let _ = queue_persistence::auto_save(self);
                (AsyncTask::new_no_op(), None)
            }
            PlaylistAction::LoadQueue => {
                let _ = queue_persistence::auto_load(self);
                (AsyncTask::new_no_op(), None)
            }
            PlaylistAction::DeleteQueue => (AsyncTask::new_no_op(), None),
            PlaylistAction::SetBestQuality => {
                self.audio_quality = AudioQuality::Best;
                info!("Audio quality set to: {:?}", self.audio_quality);
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::SaveToNewPlaylist => {
                let video_ids: Vec<VideoID<'static>> = self.list.get_list_iter()
                    .map(|song| song.video_id.clone())
                    .collect();
                if video_ids.is_empty() {
                    return (AsyncTask::new_no_op(), None);
                }
                (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistSavePopup(video_ids)))
            },
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
        // FIX: Use search indices directly when search is active (ignore shuffle)
        let list_len = if !self.search_text.is_empty() {
            self.search_indices.len()
        } else {
            self.list.get_list_iter().len()
        };

        let cur_playing_visual = self
            .get_cur_playing_index()
            .and_then(|idx| self.actual_to_visual_index(idx));

        (0..list_len).map(move |visual_i| {
            let actual_i = self.visual_to_actual_index(visual_i);

            let ls = self
                .list
                .get_song_from_idx(actual_i)
                .expect("BUG: Visual index mapping failed");

            let first_field: Cow<'_, str> = if Some(visual_i) == cur_playing_visual {
                match self.play_status {
                    PlayState::NotPlaying => Cow::Borrowed(">>>"),
                    PlayState::Playing(_) => Cow::Borrowed(""),
                    PlayState::Paused(_) => Cow::Borrowed(""),
                    PlayState::Stopped => Cow::Borrowed(">>>"),
                    PlayState::Error(_) => Cow::Borrowed(">>>"),
                    PlayState::Buffering(_) => Cow::Borrowed(""),
                }
            } else {
                Cow::Owned((visual_i + 1).to_string())
            };

            iter::once(first_field).chain(ls.get_fields([
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

        let quality_indicator = match self.audio_quality {
            AudioQuality::Best => " [Q:Best]",
            AudioQuality::High => " [Q:High]",
            AudioQuality::Medium => " [Q:Medium]",
            AudioQuality::Low => " [Q:Low]",
        };

        let search_indicator = if !self.search_text.is_empty() {
            format!(" [SEARCH: {}]", self.search_text)
        } else if self.search_enabled {
            " [SEARCH]".to_string()
        } else {
            "".to_string()
        };

        format!(
            "Local playlist - {} songs{}{}{}",
            self.list.get_list_iter().len(),
            quality_indicator,
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
            audio_quality: AudioQuality::default(),
            widget_state: Default::default(),
            shuffle_enabled: false,
            shuffle_indices: Vec::new(),
            shuffle_seed: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            active_downloads: Arc::new(std::sync::Mutex::new(Vec::new())),
            download_queue: VecDeque::new(),
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
        self.download_queue.clear();
        self.cur_selected = 0;
    }

    pub fn play_prev(&mut self) -> ComponentEffect<Self> {
        let cur = &self.play_status;
        match cur {
            PlayState::NotPlaying | PlayState::Stopped => {
                warn!("Asked to play prev, but not currently playing");
                AsyncTask::new_no_op()
            }
            PlayState::Paused(id)
            | PlayState::Playing(id)
            | PlayState::Buffering(id)
            | PlayState::Error(id) => {
                if let Some(prev_song_id) = self.get_prev_song_id(*id) {
                    self.play_song_id(prev_song_id)
                } else {
                    AsyncTask::new_no_op()
                }
            }
        }
    }

    pub fn handle_song_downloaded(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        let start = std::time::Instant::now();
        if let PlayState::Buffering(target_id) = self.play_status {
            if target_id == id {
                info!("play_attempt: song_id={:?}, state=Buffering, ms_since_download={}", 
                    id, start.elapsed().as_millis());
                return if matches!(self.queue_status, QueueState::Queued(_)) {
                    let effect = self.autoplay_song_id(id);
                    info!("autoplay_started: song_id={:?}, ms_to_start={}", id, start.elapsed().as_millis());
                    effect
                } else {
                    let effect = self.play_song_id(id);
                    info!("play_started: song_id={:?}, ms_to_start={}", id, start.elapsed().as_millis());
                    effect
                };
            }
        }
        info!("download_handled_not_playing: song_id={:?}, state={:?}", id, self.play_status);
        AsyncTask::new_no_op()
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

        let effect: ComponentEffect<Self> = albums
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

        let was_playing = self.get_cur_playing_id();
        let first_id = self.list.push_song_list(song_list);

        if self.shuffle_enabled {
            self.generate_shuffle_indices();

            if let (Some(_current_id), Some(playing_idx)) =
                (was_playing, self.get_cur_playing_index())
            {
                if let Some(shuffled_pos) =
                    self.shuffle_indices.iter().position(|&i| i == playing_idx)
                {
                    self.cur_selected = shuffled_pos;
                }
            } else {
                self.cur_selected = 0.min(self.get_max_visual_index());
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

        info!("download_upcoming_from_id: START for id={:?}, index={}", id, song_index);

        // Build list of song IDs in scope: [current, N-1, +1, +2]
        // Current song downloads first, rest queue naturally
        let mut song_ids: Vec<ListSongID> = vec![id];

        // Add N-1 (previous) if exists
        if song_index > 0 {
            if let Some(prev_id) = self.get_id_from_index(song_index - 1) {
                song_ids.push(prev_id);
            }
        }

        if self.shuffle_enabled {
            let Some(visual_index) = self.actual_to_visual_index(song_index) else {
                return AsyncTask::new_no_op();
            };

            for offset in 1..=SONGS_AHEAD_TO_BUFFER {
                let next_pos = visual_index.saturating_add(offset);
                if next_pos < self.shuffle_indices.len() {
                    let next_actual = self.shuffle_indices[next_pos];
                    if let Some(next_id) = self.get_id_from_index(next_actual) {
                        if !song_ids.contains(&next_id) {
                            song_ids.push(next_id);
                        }
                    }
                }
            }
        } else {
            for offset in 1..=SONGS_AHEAD_TO_BUFFER {
                if let Some(next_song) = self.get_song_from_idx(song_index.saturating_add(offset)) {
                    if !song_ids.contains(&next_song.id) {
                        song_ids.push(next_song.id);
                    }
                }
            }
        }

        // Log what songs are in scope with their current status
        for &sid in &song_ids {
            if let Some(idx) = self.get_index_from_id(sid) {
                if let Some(s) = self.list.get_list_iter().nth(idx) {
                    info!("  scope_song: id={:?}, video_id={}, status={:?}", sid, s.video_id.get_raw(), s.download_status);
                }
            }
        }

        // Cancel downloads not in scope
        self.cancel_out_of_scope_downloads(&song_ids);

        info!("download_upcoming_from_id: queue BEFORE clear: {:?}", self.download_queue);
        
        // Clear existing download queue and add new songs
        // BUT: preserve songs that are already downloaded - they don't need to be re-queued
        self.download_queue.clear();
        for song_id in &song_ids {
            let is_downloaded = if let Some(idx) = self.get_index_from_id(*song_id) {
                if let Some(s) = self.get_song_from_idx(idx) {
                    matches!(s.download_status, DownloadStatus::Downloaded(_))
                } else { false }
            } else { false };
            
            if is_downloaded {
                info!("download_upcoming_from_id: skipping {:?} (already downloaded)", song_id);
            } else {
                self.download_queue.push_back(*song_id);
            }
        }

        info!("download_upcoming_from_id: queue AFTER filtering: {:?}", self.download_queue);

        // Start only the first download (current song)
        // Others will be started sequentially as downloads complete
        let mut combined_effect = AsyncTask::new_no_op();
        if let Some(first_id) = self.download_queue.pop_front() {
            info!("download_upcoming_from_id: STARTING FIRST DOWNLOAD: {:?}", first_id);
            combined_effect = combined_effect.push(self.download_song(first_id));
        } else {
            info!("download_upcoming_from_id: no download needed (all in scope already downloaded)");
        }

        let thumbnail_effects = self.prefetch_thumbnails_for_indices(&song_ids);
        combined_effect = combined_effect.push(thumbnail_effects);
        combined_effect
    }

    pub fn download_song(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        let Some(song_index) = self.get_index_from_id(id) else {
            debug!("download_song: song id {:?} not found", id);
            return AsyncTask::new_no_op();
        };

        let song = self
            .list
            .get_list_iter_mut()
            .nth(song_index)
            .expect("Checked previously");

        let video_id = song.video_id.get_raw().to_string();
        debug!("download_song: {}", video_id);
        
        match &song.download_status {
            DownloadStatus::Downloading(_) => {
                debug!("download_song: {} already downloading", video_id);
                return AsyncTask::new_no_op();
            }
            DownloadStatus::Downloaded(_) => {
                debug!("download_song: {} already downloaded", video_id);
                return AsyncTask::new_no_op();
            }
            DownloadStatus::Queued => {
                debug!("download_song: {} already queued", video_id);
                return AsyncTask::new_no_op();
            }
            _ => (),
        };

        // Cancel existing download for this song if any
        {
            let downloads = self.active_downloads.lock().unwrap();
            if let Some((_, task)) = downloads.iter().find(|(sid, _)| *sid == id) {
                debug!("download_song: cancelling existing download for {}", video_id);
                task.cancel_token.cancel();
            }
        }

        let cancel_token = Arc::new(tokio_util::sync::CancellationToken::new());
        debug!("download_song: starting download for {}", video_id);

        let effect = AsyncTask::new_stream(
            DownloadSong(song.video_id.clone(), id, cancel_token.clone(), self.audio_quality),
            HandleSongDownloadProgressUpdate,
            None,
        );

        let mut downloads = self.active_downloads.lock().unwrap();
        downloads.retain(|(song_id, _)| *song_id != id);
        downloads.push((
            id,
            DownloadTask {
                cancel_token,
            },
        ));

        song.download_status = DownloadStatus::Queued;
        effect
    }

    fn prefetch_thumbnails_for_indices(&self, song_ids: &[ListSongID]) -> ComponentEffect<Self> {
        let get_largest_thumbnail_url = |thumbs: &Vec<Thumbnail>| {
            thumbs
                .iter()
                .max_by_key(|t| t.height * t.width)
                .map(|t| t.url.clone())
        };

        let thumb_tasks: Vec<_> = song_ids
            .iter()
            .filter_map(|song_id| {
                let song = self.get_song_from_id(*song_id)?;
                if matches!(song.album_art, AlbumArtState::Downloaded(_)) {
                    return None;
                }
                if !song.thumbnails.as_ref().is_empty() && song.thumbnails.as_ref().iter().any(|t| !t.url.is_empty()) {
                    return None;
                }
                let thumb_url = get_largest_thumbnail_url(song.thumbnails.as_ref())?;
                let thumbnail_id = SongThumbnailID::from(song as &ListSong).into_owned();
                Some((thumbnail_id, thumb_url))
            })
            .collect();

        if thumb_tasks.is_empty() {
            return AsyncTask::new_no_op();
        }

        thumb_tasks
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
            .collect()
    }

    pub fn drop_unscoped_from_id(&mut self, id: ListSongID) {
        let Some(song_index) = self.get_index_from_id(id) else {
            return;
        };

        let forward_limit = song_index.saturating_add(SONGS_AHEAD_TO_BUFFER);
        let backwards_limit = song_index.saturating_sub(SONGS_BEHIND_TO_SAVE);

        let mut downloads = self.active_downloads.lock().unwrap();
        downloads.retain(|(song_id, task)| {
            if let Some(idx) = self.get_index_from_id(*song_id) {
                if idx < backwards_limit || idx >= forward_limit {
                    task.cancel_token.cancel();
                    return false;
                }
            }
            true
        });

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
    pub fn go_to_first(&mut self) {
        self.cur_selected = 0;
    }

    pub fn go_to_last(&mut self) {
        self.cur_selected = self.list.get_list_iter().len().saturating_sub(1);
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
            PlayState::Playing(id) | PlayState::Paused(id) => id,
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
        if self.list.get_list_iter().len() == 0 {
            return AsyncTask::new_no_op();
        }

        let actual_index = self.visual_to_actual_index(self.cur_selected);
        let Some(id) = self.get_id_from_index(actual_index) else {
            return AsyncTask::new_no_op();
        };
        self.play_song_id(id)
    }

    pub fn delete_selected(&mut self) -> ComponentEffect<Self> {
        // FIX: Don't delete if search is active but has no results
        if !self.search_text.is_empty() && self.search_indices.is_empty() {
            return AsyncTask::new_no_op();
        }

        let mut return_task = AsyncTask::new_no_op();

        if self.list.get_list_iter().len() == 0 {
            return return_task;
        }

        let visual_index_before = self.cur_selected;
        let actual_index = self.visual_to_actual_index(visual_index_before);

        if let Some(cur_playing_id) = self.get_cur_playing_id() {
            if Some(actual_index) == self.get_cur_playing_index() {
                self.play_status = PlayState::NotPlaying;
                return_task = self.stop_song_id(cur_playing_id);
            }
        }

        // Delete from main list first
        self.list.remove_song_index(actual_index);

        // Rebuild search to ensure accuracy
        if !self.search_text.is_empty() {
            self.update_search_indices();
            self.cur_selected = if self.search_indices.is_empty() {
                0
            } else {
                visual_index_before.min(self.search_indices.len() - 1)
            };
        } else {
            let new_max = self.list.get_list_iter().len().saturating_sub(1);
            self.cur_selected = self.cur_selected.min(new_max);
        }

        // Adjust shuffle indices
        if self.shuffle_enabled {
            if let Some(pos) = self.shuffle_indices.iter().position(|&i| i == actual_index) {
                self.shuffle_indices.remove(pos);
            }
            for idx in &mut self.shuffle_indices {
                if *idx > actual_index {
                    *idx = idx.saturating_sub(1);
                }
            }
        }

        return_task = return_task.push(self.regenerate_downloads_for_current());

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

            if let (Some(_current_id), Some(playing_idx)) =
                (self.get_cur_playing_id(), self.get_cur_playing_index())
            {
                if let Some(shuffled_pos) =
                    self.shuffle_indices.iter().position(|&i| i == playing_idx)
                {
                    self.cur_selected = shuffled_pos;
                }
            } else {
                self.cur_selected = 0.min(self.get_max_visual_index());
            }
        } else {
            if let Some(playing_idx) = self.get_cur_playing_index() {
                self.cur_selected =
                    playing_idx.min(self.list.get_list_iter().len().saturating_sub(1));
            }
            self.shuffle_indices.clear();
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
                    .unwrap_or_default();

                let album = song
                    .get_fields([ListSongDisplayableField::Album])
                    .into_iter()
                    .next()
                    .unwrap_or_default();

                let artist = song
                    .get_fields([ListSongDisplayableField::Artists])
                    .into_iter()
                    .next()
                    .unwrap_or_default();

                if title.to_lowercase().contains(&search_lower)
                    || album.to_lowercase().contains(&search_lower)
                    || artist.to_lowercase().contains(&search_lower) 
                {
                    Some(actual_idx)
                } else {
                    None
                }
            })
            .collect();
    }

    // FIX: When search is active, ignore shuffle and use search_indices directly
    fn visual_to_actual_index(&self, visual_index: usize) -> usize {
        let list_len = self.list.get_list_iter().count();
        if list_len == 0 {
            return 0;
        }

        // Search mode: use original list order, ignore shuffle
        if !self.search_text.is_empty() {
            if self.search_indices.is_empty() {
                return 0;
            }
            let clamped = visual_index.min(self.search_indices.len() - 1);
            return self.search_indices[clamped];
        }

        // No search: apply shuffle if enabled
        let base_index = visual_index.min(list_len - 1);
        if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices[base_index.min(self.shuffle_indices.len() - 1)]
        } else {
            base_index.min(list_len - 1)
        }
    }

    // FIX: When search is active, ignore shuffle
    fn actual_to_visual_index(&self, actual_index: usize) -> Option<usize> {
        // Search mode: find position in search results directly
        if !self.search_text.is_empty() {
            return self.search_indices.iter().position(|&i| i == actual_index);
        }

        // No search: apply shuffle mapping
        let shuffled_pos = if self.shuffle_enabled && !self.shuffle_indices.is_empty() {
            self.shuffle_indices
                .iter()
                .position(|&i| i == actual_index)?
        } else {
            actual_index
        };

        Some(shuffled_pos)
    }

    fn get_next_song_id(&self, _current_id: ListSongID) -> Option<ListSongID> {
        let current_visual = self
            .get_cur_playing_index()
            .and_then(|idx| self.actual_to_visual_index(idx))?;

        if current_visual >= self.get_max_visual_index() {
            return None;
        }

        let next_visual = current_visual.saturating_add(1);
        let next_actual = self.visual_to_actual_index(next_visual);
        self.get_id_from_index(next_actual)
    }

    fn get_prev_song_id(&self, _current_id: ListSongID) -> Option<ListSongID> {
        let current_visual = self
            .get_cur_playing_index()
            .and_then(|idx| self.actual_to_visual_index(idx))?;

        if current_visual == 0 {
            return None;
        }

        let prev_visual = current_visual.saturating_sub(1);
        let prev_actual = self.visual_to_actual_index(prev_visual);
        self.get_id_from_index(prev_actual)
    }

    // FIX: When search is active, ignore shuffle
    fn get_max_visual_index(&self) -> usize {
        let count = if !self.search_text.is_empty() {
            // Search mode: count of matching results
            self.search_indices.len()
        } else if self.shuffle_enabled {
            // Shuffle mode: count of shuffled items
            self.shuffle_indices.len()
        } else {
            // Normal mode: full list count
            self.list.get_list_iter().count()
        };

        count.saturating_sub(1)
    }

    fn cancel_all_downloads(&self) {
        let mut downloads = self.active_downloads.lock().unwrap();
        downloads.clear();
    }

    fn cancel_out_of_scope_downloads(&self, scope_ids: &[ListSongID]) {
        let downloads = self.active_downloads.lock().unwrap();

        for (song_id, task) in downloads.iter() {
            if !scope_ids.contains(song_id) {
                info!("cancel_out_of_scope_downloads: cancelling out-of-scope download for {:?}", song_id);
                task.cancel_token.cancel();
            }
        }
    }

    fn regenerate_downloads_for_current(&mut self) -> ComponentEffect<Self> {
        if let Some(current_id) = self.get_cur_playing_id() {
            self.drop_unscoped_from_id(current_id);
            self.download_upcoming_from_id(current_id)
        } else {
            AsyncTask::new_no_op()
        }
    }

    pub fn handle_song_download_progress_update(
        &mut self,
        update: DownloadProgressUpdateType,
        id: ListSongID,
    ) -> ComponentEffect<Self> {
        let song = self.get_song_from_id(id);
        let video_id = song.map(|s| s.video_id.get_raw().to_string()).unwrap_or_else(|| "unknown".to_string());
        
        if let Some(song) = song {
            match &song.download_status {
                DownloadStatus::None | DownloadStatus::Downloaded(_) | DownloadStatus::Failed => {
                    debug!("handle_song_download_progress: song {} already in final state, ignoring", video_id);
                    return AsyncTask::new_no_op();
                }
                _ => (),
            }
        } else {
            debug!("handle_song_download_progress: song id {:?} not found", id);
            return AsyncTask::new_no_op();
        }

        let mut effect = AsyncTask::new_no_op();

        match update {
            DownloadProgressUpdateType::Started => {
                info!("download_started: song_id={}", video_id);
                // Use index lookup for reliable status update
                if let Some(idx) = self.get_index_from_id(id) {
                    if let Some(song) = self.list.get_list_iter_mut().nth(idx) {
                        song.download_status = DownloadStatus::Queued;
                    }
                } else {
                    warn!("download_started: song {} not found by id {:?}", video_id, id);
                }
            }
            DownloadProgressUpdateType::Completed(song_buf) => {
                info!("download_done: song_id={}, size={}", video_id, song_buf.0.len());
                if let Some(idx) = self.get_index_from_id(id) {
                    if let Some(s) = self.list.get_list_iter_mut().nth(idx) {
                        s.download_status = DownloadStatus::Downloaded(Arc::new(song_buf));
                        info!("download_status_updated: song_id={} -> Downloaded", video_id);
                    }
                } else {
                    error!("download_done: song {} not found by id {:?}", video_id, id);
                }
                self.active_downloads
                    .lock()
                    .unwrap()
                    .retain(|(song_id, _)| *song_id != id);
                effect = self.handle_song_downloaded(id);
                
                // Start next download in queue if available
                if let Some(next_id) = self.download_queue.pop_front() {
                    info!("queue_starting_next: song_id={:?}", next_id);
                    effect = effect.push(self.download_song(next_id));
                }
            }
            DownloadProgressUpdateType::Error => {
                error!("download_error: song_id={}", video_id);
                if let Some(idx) = self.get_index_from_id(id) {
                    if let Some(song) = self.list.get_list_iter_mut().nth(idx) {
                        song.download_status = DownloadStatus::Failed;
                    }
                }
                self.active_downloads
                    .lock()
                    .unwrap()
                    .retain(|(song_id, _)| *song_id != id);
                
                // Start next download in queue if available (even on error)
                if let Some(next_id) = self.download_queue.pop_front() {
                    debug!("Starting next download in queue after error: {:?}", next_id);
                    effect = effect.push(self.download_song(next_id));
                }
            }
            DownloadProgressUpdateType::Retrying { times_retried } => {
                debug!("handle_song_download_progress: RETRYING (try {}) for {}", times_retried, video_id);
                if let Some(song) = self.list.get_list_iter_mut().find(|x| x.id == id) {
                    song.download_status = DownloadStatus::Retrying { times_retried };
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
