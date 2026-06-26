use super::action::{AppAction, TextEntryAction};
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, KeyRouter, Scrollable, TextHandler, YoutuiEffect,
};
use crate::app::queue_persistence;
use crate::app::server::song_downloader::DownloadProgressUpdateType;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use crate::app::server::{
    AutoplayDecodedSong, DecodeSong, DownloadSong, GetSongThumbnail, IncreaseVolume, Pause,
    PausePlay, PlayDecodedSong, QueueDecodedSong, Resume, Seek, SeekTo, Stop, StopAll,
    TaskMetadata, ValidateMetadata, AlbumTrack,
};
use crate::app::structures::{
    fuzzy_match, AlbumArtState, AlbumOrUploadAlbumID, AudioQuality, BrowserSongsList, DownloadStatus,
    ListSong, ListSongDisplayableField, ListSongID, Percentage, PlayState, SongListComponent,
    Thumbnail,
};
use std::collections::VecDeque;
use crate::app::ui::playlist::effect_handlers::{
    HandleAllStopped, HandleAutoplayUpdateOk, HandleGetSongThumbnailError,
    HandleGetSongThumbnailOk, HandlePausePlayResponse, HandlePausedResponse, HandlePlayUpdateError,
    HandlePlayUpdateOk, HandleQueueUpdateOk, HandleResumeResponse, HandleSetSongPlayProgress,
    HandleSongDownloadProgressUpdate, HandleStopped, HandleVolumeUpdate,
};
use crate::app::ui::playlist::effect_handlers_playlist::{
    HandleMetadataValidated, HandleMetadataValidationError,
    HandleRateSongOk, HandleRateSongErr,
    HandleFetchAlbumArtOk, HandleFetchAlbumArtErr,
};
use crate::app::ui::draw_media_controls::upgrade_thumbnail_url;
use crate::app::ui::{AppCallback, WindowContext};
use crate::app::view::draw::{draw_loadable, draw_panel_mut, draw_table};
use crate::app::view::{BasicConstraint, DrawableMut, HasTitle, Loadable, SortDirection, TableView};
use audio_player::{
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

pub mod lyrics_popup;
pub mod song_info_popup;
pub mod album_art_popup;
pub mod config_editor_popup;
pub mod playlist_save_popup;
pub mod playlist_update_popup;
pub mod playlist_editor_popup;
pub mod playlist_rename_popup;
pub mod playlist_edit_popup;
pub mod notes_popup;
pub mod playlist_details_popup;
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
    category_filter: Option<&'static str>,
    romaji_mode: bool,
    pub scrobble_state: Option<crate::app::scrobbler::ScrobbleState>,
    /// Guard: prevents duplicate scrobble submissions from overlapping progress updates
    scrobble_pending: bool,
    search_cur: usize,
    romaji_originals: HashMap<ListSongID, String>,
    pub album_tracks: Option<Vec<AlbumTrack>>,
    pub album_current_track: usize,
    pub scrobbling_config: crate::config::ScrobblingConfig,
    pub yt_dlp_cookie_path: Option<String>,
    pub repeat_mode: crate::app::structures::RepeatMode,
    pub radio_mode: bool,
    /// Transient error message shown in playlist header (clears on next action)
    pub last_error: Option<String>,
    /// Transient status notification (clears on next action)
    pub last_status: Option<String>,
    /// Pending chunks for multi-playlist split (video_ids, title, description, next_index)
    pub pending_playlist_chunks: Option<(Vec<Vec<ytmapi_rs::common::VideoID<'static>>>, String, Option<String>, Option<ytmapi_rs::query::playlist::PrivacyStatus>)>,
    /// Stack of deleted songs for undo (song, original_index)
    pub undo_stack: Vec<Vec<(crate::app::structures::ListSong, usize)>>,
    pub visual_mode: bool,
    pub visual_start: usize,
    yank_buffer: Vec<ListSong>,
    /// Guard: true when a FetchAlbumArt task is in-flight for current song
    pub album_art_fetching: bool,
    /// Album name being fetched for album art matching
    pub album_art_fetching_name: Option<String>,
    /// Pending count for next vim-style operation (e.g., 5dd → delete 5)
    pub pending_count: usize,
    /// Sort state for queue
    pub sort_mode: bool,
    pub sort_column: usize,
    pub sort_direction: SortDirection,
    /// Set true by playlist mutation handlers to signal library needs refresh
    pub library_playlist_mutated: bool,
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
    LoadFromYTM,
    ViewLyrics,
    TogglePlaylistCategoryFilter,
    NextSearchResult,
    PrevSearchResult,
    CopySongUrl,
    CopyAlbumUrl,
    OpenUrl,
    ToggleRomaji,
    ToggleRepeat,
    ToggleRadio,
    ViewSongInfo,
    SaveToExistingPlaylist,
    UndoDelete,
    DeleteToTop,
    DeleteToBottom,
    ToggleVisualMode,
    GoToArtist,
    GoToAlbum,
    GetRelatedTracks,
    ToggleLike,
    ViewAlbumCover,
    ContextActions,
    SortQueue,
    SortQueueAsc,
    SortQueueDesc,
    SortQueueClear,
    ForceSplitAlbum,
    PasteYanked,
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
            PlaylistAction::LoadFromYTM => "Load YouTube Music Playlist",
            PlaylistAction::ViewLyrics => "View Lyrics",
            PlaylistAction::TogglePlaylistCategoryFilter => "Toggle Category Filter",
            PlaylistAction::NextSearchResult => "Next Match",
            PlaylistAction::PrevSearchResult => "Prev Match",
            PlaylistAction::CopySongUrl => "Copy Song URL",
            PlaylistAction::CopyAlbumUrl => "Copy Album URL",
            PlaylistAction::OpenUrl => "Open URL",
            PlaylistAction::ToggleRomaji => "Toggle Romaji",
            PlaylistAction::ToggleRepeat => "Toggle Repeat Mode",
            PlaylistAction::ToggleRadio => "Toggle Radio Mode",
            PlaylistAction::ViewSongInfo => "View Song Info",
            PlaylistAction::SaveToExistingPlaylist => "Add Queue to Playlist",
            PlaylistAction::UndoDelete => "Undo Delete",
            PlaylistAction::DeleteToTop => "Delete to Top",
            PlaylistAction::DeleteToBottom => "Delete to Bottom",
            PlaylistAction::ToggleVisualMode => "Toggle Visual Mode",
            PlaylistAction::GoToArtist => "Go to Artist",
            PlaylistAction::GoToAlbum => "Go to Album",
            PlaylistAction::GetRelatedTracks => "Get Related Tracks",
            PlaylistAction::ToggleLike => "Like / Unlike",
            PlaylistAction::ViewAlbumCover => "View Album Cover",
            PlaylistAction::ContextActions => "Context Actions",
            PlaylistAction::SortQueue => "Sort Queue",
            PlaylistAction::SortQueueAsc => "Sort Ascending",
            PlaylistAction::SortQueueDesc => "Sort Descending",
            PlaylistAction::SortQueueClear => "Clear Sort",
            PlaylistAction::ForceSplitAlbum => "Force Split Album",
            PlaylistAction::PasteYanked => "Paste Yanked",
        }
        .into()
    }
}

impl ActionHandler<PlaylistAction> for Playlist {
    fn apply_action(&mut self, action: PlaylistAction) -> impl Into<YoutuiEffect<Playlist>> {
        self.last_error.take(); // Clear transient error on any action
        self.last_status.take(); // Clear transient status on any action
        match action {
            PlaylistAction::ViewBrowser => (AsyncTask::new_no_op(), Some(self.view_browser())),
            PlaylistAction::PlaySelected => (self.play_selected(), None),
            PlaylistAction::DeleteSelected => (self.delete_selected(), None),
            PlaylistAction::DeleteAll => (self.delete_all(), None),
            PlaylistAction::DeleteToTop => (self.delete_to_top(), None),
            PlaylistAction::DeleteToBottom => (self.delete_to_bottom(), None),
            PlaylistAction::UndoDelete => (self.undo_delete(), None),
            PlaylistAction::ToggleVisualMode => (self.toggle_visual_mode(), None),
            PlaylistAction::ToggleShuffle => (self.toggle_shuffle(), None),
            PlaylistAction::ToggleSearch => (self.toggle_search(), None),
            PlaylistAction::ClearSearch => {
                if self.visual_mode { self.visual_mode = false; }
                (self.clear_search(), None)
            }
            PlaylistAction::SaveQueue => {
                match queue_persistence::auto_save(self) {
                    Ok(_) => info!("Queue saved successfully"),
                    Err(e) => warn!("Failed to save queue: {}", e),
                }
                (AsyncTask::new_no_op(), None)
            }
            PlaylistAction::LoadQueue => {
                match queue_persistence::auto_load(self) {
                    Ok(_) => info!("Queue loaded successfully"),
                    Err(e) => warn!("Failed to load queue: {}", e),
                }
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
            PlaylistAction::SaveToExistingPlaylist => {
                let video_ids: Vec<_> = self.list.get_list_iter()
                    .map(|s| s.video_id.clone())
                    .collect();
                if video_ids.is_empty() {
                    return (AsyncTask::new_no_op(), None);
                }
                (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistUpdatePopup(video_ids)))
            },
            PlaylistAction::LoadFromYTM => {
                let video_ids: Vec<VideoID<'static>> = Vec::new();
                (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistUpdatePopup(video_ids)))
            },
            PlaylistAction::TogglePlaylistCategoryFilter => {
                self.category_filter = match self.category_filter {
                    None => Some("Album:"),
                    Some("Album:") => Some("EP:"),
                    Some("EP:") => Some("Single:"),
                    _ => None,
                };
                self.update_search_indices();
                self.cur_selected = self.cur_selected.min(self.search_indices.len().saturating_sub(1));
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::NextSearchResult => {
                if !self.search_indices.is_empty() {
                    self.search_cur = (self.search_cur + 1) % self.search_indices.len();
                    self.cur_selected = self.search_cur.min(self.search_indices.len().saturating_sub(1));
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::PrevSearchResult => {
                if !self.search_indices.is_empty() {
                    self.search_cur = self.search_cur.saturating_sub(1);
                    self.cur_selected = self.search_cur;
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::CopySongUrl => {
                if self.visual_mode {
                    let (start, end) = if self.visual_start <= self.cur_selected {
                        (self.visual_start, self.cur_selected)
                    } else {
                        (self.cur_selected, self.visual_start)
                    };
                    let lines: Vec<String> = self.list.get_list_iter()
                        .skip(start).take(end - start + 1)
                        .map(|s| {
                            let artists = s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                            format!("{} - {}", artists, s.title)
                        })
                        .collect();
                    // Save songs to yank buffer for paste
                    self.yank_buffer = self.list.get_list_iter()
                        .skip(start).take(end - start + 1)
                        .cloned()
                        .collect();
                    crate::app::structures::copy_to_clipboard(&lines.join("\n"));
                    info!("Yanked {} lines from visual selection to clipboard and buffer", self.yank_buffer.len());
                    self.visual_mode = false;
                    return (AsyncTask::new_no_op(), None);
                }
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                if let Some(song) = self.get_song_from_idx(actual_index) {
                    let raw_url = format!("https://music.youtube.com/watch?v={}", song.video_id.get_raw());
                    crate::app::structures::copy_to_clipboard(&raw_url);
                    info!("Copied URL: {}", raw_url);
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::CopyAlbumUrl => {
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                if let Some(song) = self.get_song_from_idx(actual_index) {
                    if let Some(album) = &song.album {
                        let raw = match &album.id {
                            AlbumOrUploadAlbumID::Album(id) => id.get_raw(),
                            AlbumOrUploadAlbumID::UploadAlbum(id) => id.get_raw(),
                        };
                        let raw_url = format!("https://music.youtube.com/browse/{}", raw);
                        crate::app::structures::copy_to_clipboard(&raw_url);
                        info!("Copied album URL: {}", raw_url);
                    }
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::OpenUrl => {
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::GoToArtist => {
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                if let Some(song) = self.get_song_from_idx(actual_index) {
                    if let Some(cb) = crate::app::ui::browser::shared_components::navigate_to_artist(song) {
                        return (AsyncTask::new_no_op(), Some(cb));
                    }
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::GoToAlbum => {
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                if let Some(song) = self.get_song_from_idx(actual_index) {
                    if let Some(cb) = crate::app::ui::browser::shared_components::navigate_to_album(song) {
                        return (AsyncTask::new_no_op(), Some(cb));
                    }
                    warn!("Song has no album data, cannot navigate to album");
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::GetRelatedTracks => {
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                if let Some(song) = self.get_song_from_idx(actual_index) {
                    return (AsyncTask::new_no_op(), Some(AppCallback::GetRelatedTracks(song.video_id.clone())));
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::ToggleLike => {
                // Guard: splitted album tracks cannot be liked
                if self.album_tracks.is_some() {
                    if let Some(song) = self.list.get_list_iter().nth(self.cur_selected) {
                        if song.track_no.is_some() {
                            self.last_error = Some("Cannot like splitted album tracks yet".to_string());
                            return (AsyncTask::new_no_op(), None);
                        }
                    }
                }
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                if let Some(song) = self.list.get_list_iter_mut().nth(actual_index) {
                    use ytmapi_rs::common::LikeStatus;
                    let new_status = match song.like_status {
                        LikeStatus::Liked => LikeStatus::Indifferent,
                        _ => LikeStatus::Liked,
                    };
                    song.like_status = new_status.clone();
                    let video_id = song.video_id.clone();
                    let effect = AsyncTask::new_future_try(
                        crate::app::server::RateSong(video_id, new_status),
                        HandleRateSongOk,
                        HandleRateSongErr,
                        None,
                    ).map_frontend(|this: &mut Self| this);
                    return (effect, None);
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::ForceSplitAlbum => {
                (self.handle_force_split(self.cur_selected), None)
            },
            PlaylistAction::PasteYanked => {
                if !self.yank_buffer.is_empty() {
                    let insert_pos = self.cur_selected.saturating_add(1);
                    let max = self.list.get_list_iter().count();
                    let pos = insert_pos.min(max);
                    self.list.insert_song_list_at(self.yank_buffer.clone(), pos);
                    if self.shuffle_enabled { self.generate_shuffle_indices(); }
                    info!("Pasted {} yanked songs at position {}", self.yank_buffer.len(), pos);
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::ToggleRomaji => {
                if self.romaji_mode {
                    // Restore originals
                    for song in self.list.get_list_iter_mut() {
                        if let Some(orig) = self.romaji_originals.remove(&song.id) {
                            song.title = orig;
                        }
                    }
                } else {
                    // Save originals and convert all song titles
                    for song in self.list.get_list_iter_mut() {
                        let converted = crate::app::ui::playlist::lyrics_popup::japanese_to_romaji(&song.title);
                        if converted != song.title {
                            self.romaji_originals.insert(song.id, song.title.clone());
                            song.title = converted;
                        }
                    }
                }
                self.romaji_mode = !self.romaji_mode;
                info!("Romaji mode: {}", self.romaji_mode);
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::ToggleRepeat => {
                use crate::app::structures::RepeatMode;
                self.repeat_mode = match self.repeat_mode {
                    RepeatMode::Off => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::Off,
                };
                info!("Repeat mode: {:?}", self.repeat_mode);
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::ToggleRadio => {
                self.radio_mode = !self.radio_mode;
                info!("Radio mode: {}", self.radio_mode);
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::ViewLyrics => {
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                let song = self.get_song_from_idx(actual_index);
                if let Some(song) = song {
                    let artist = song.artists.iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    (AsyncTask::new_no_op(), Some(AppCallback::ViewLyrics {
                        artist,
                        title: song.title.clone(),
                    }))
                } else {
                    (AsyncTask::new_no_op(), None)
                }
            },
            PlaylistAction::ViewSongInfo => {
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                let song = self.get_song_from_idx(actual_index);
                if let Some(song) = song {
                    (AsyncTask::new_no_op(), Some(AppCallback::ViewSongInfo {
                        song: song.clone(),
                    }))
                } else {
                    (AsyncTask::new_no_op(), None)
                }
            },
            PlaylistAction::ContextActions => {
                let actual_index = self.visual_to_actual_index(self.cur_selected);
                if let Some(song) = self.get_song_from_idx(actual_index) {
                    let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                    tracing::info!("Context actions for: {} - {}", artist, song.title);
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::ViewAlbumCover => {
                (AsyncTask::new_no_op(), Some(AppCallback::ViewAlbumCover))
            },
            PlaylistAction::SortQueue => {
                if self.sort_mode {
                    let field = sort_column_to_field(self.sort_column);
                    self.list.sort(field, self.sort_direction);
                    self.sort_mode = false;
                } else {
                    self.sort_mode = true;
                }
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::SortQueueAsc => {
                let field = sort_column_to_field(0);
                self.list.sort(field, SortDirection::Asc);
                self.sort_mode = false;
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::SortQueueDesc => {
                let field = sort_column_to_field(0);
                self.list.sort(field, SortDirection::Desc);
                self.sort_mode = false;
                (AsyncTask::new_no_op(), None)
            },
            PlaylistAction::SortQueueClear => {
                self.sort_mode = false;
                (AsyncTask::new_no_op(), None)
            },
        }
    }
}

fn sort_column_to_field(col: usize) -> ListSongDisplayableField {
    match col {
        0 => ListSongDisplayableField::Song,
        1 => ListSongDisplayableField::Artists,
        2 => ListSongDisplayableField::Album,
        3 => ListSongDisplayableField::Duration,
        _ => ListSongDisplayableField::Song,
    }
}

fn sort_column_label(col: usize) -> &'static str {
    match col {
        0 => "Title",
        1 => "Artist",
        2 => "Album",
        3 => "Duration",
        _ => "Clear",
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
                KeyCode::Esc => {
                    self.search_enabled = false;
                    Some(AsyncTask::new_no_op())
                }
                KeyCode::Enter => {
                    // Let Enter reach keymap dispatch for TextEntryAction::Submit
                    return None;
                }
                _ => None,
            },
            _ => None,
        }
    }
}

impl DrawableMut for Playlist {
    fn draw_mut_chunk(&mut self, f: &mut Frame, chunk: Rect, selected: bool, cur_tick: u64) {
        if self.sort_mode {
            use ratatui::widgets::{Clear, Block, Borders, List, ListItem, Paragraph};
            use ratatui::style::{Color, Style};
            use ratatui::text::Span;
            let popup = crate::drawutils::centered_rect(6, 28, chunk);
            f.render_widget(Clear, popup);
            let items: Vec<ListItem> = (0..=3).map(|col| {
                let label = if col == self.sort_column {
                    format!("▸ {} {:?}", sort_column_label(col), self.sort_direction)
                } else {
                    format!("  {}", sort_column_label(col))
                };
                ListItem::new(label).style(Style::default().fg(Color::White))
            }).collect();
            let inner = ratatui::layout::Rect {
                x: popup.x,
                y: popup.y,
                width: popup.width,
                height: popup.height - 1,
            };
            f.render_widget(
                List::new(items)
                    .block(Block::default()
                        .title(" Sort Queue ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan))),
                inner,
            );
            let help_y = popup.y + popup.height - 1;
            let help_chunk = ratatui::layout::Rect { x: popup.x, y: help_y, width: popup.width, height: 1 };
            f.render_widget(
                Paragraph::new(Span::styled(
                    "j/k col  Enter apply  Esc cancel",
                    Style::default().fg(Color::DarkGray),
                )),
                help_chunk,
            );
            return;
        }
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
        if self.sort_mode {
            let cols = 4;
            self.sort_column = ((self.sort_column as isize + amount).rem_euclid(cols)) as usize;
            return;
        }
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
            BasicConstraint::Length(6),
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

        let mut items = Vec::with_capacity(list_len);
        for visual_i in 0..list_len {
            let actual_i = self.visual_to_actual_index(visual_i);

            let Some(ls) = self.list.get_song_from_idx(actual_i) else {
                error!("BUG: Visual index mapping failed for index {actual_i}");
                continue;
            };

            let first_field: Cow<'_, str> = if Some(visual_i) == cur_playing_visual {
                match self.play_status {
                    PlayState::NotPlaying => Cow::Borrowed(">>>"),
                    PlayState::Playing(_) | PlayState::Paused(_) | PlayState::Buffering(_) => {
                        Cow::Owned((visual_i + 1).to_string())
                    }
                    PlayState::Stopped => Cow::Borrowed(">>>"),
                    PlayState::Error(_) => Cow::Borrowed(">>>"),
                }
            } else {
                Cow::Owned((visual_i + 1).to_string())
            };

            items.push(iter::once(first_field).chain(ls.get_fields([
                ListSongDisplayableField::DownloadStatus,
                ListSongDisplayableField::TrackNo,
                ListSongDisplayableField::Artists,
                ListSongDisplayableField::Album,
                ListSongDisplayableField::Song,
                ListSongDisplayableField::Duration,
                ListSongDisplayableField::Year,
            ])));
        }
        items.into_iter()
    }

    fn get_headings(&self) -> impl Iterator<Item = &'static str> {
        [
            "p#", "", "t#", "Artist", "Album", "Song", "Duration", "Year",
        ]
        .into_iter()
    }

    fn get_visual_range(&self) -> Option<(usize, usize)> {
        if self.visual_mode {
            let start = self.visual_start.min(self.cur_selected);
            let end = self.visual_start.max(self.cur_selected);
            Some((start, end))
        } else {
            None
        }
    }

    fn get_highlighted_row(&self) -> Option<usize> {
        if self.visual_mode {
            Some(self.visual_start)
        } else {
            self.get_cur_playing_index()
                .and_then(|idx| self.actual_to_visual_index(idx))
        }
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
            let total = self.search_indices.len();
            let cur = self.search_cur + 1;
            format!(" [SEARCH: {} ({}/{})]", self.search_text, cur.min(total), total)
        } else if self.search_enabled {
            " [SEARCH]".to_string()
        } else {
            "".to_string()
        };

        let romaji_indicator = if self.romaji_mode { " [Romaji]" } else { "" };
        let cat_indicator = match self.category_filter {
            Some("Album:") => " [Albums]",
            Some("EP:") => " [EPs]",
            Some("Single:") => " [Singles]",
            _ => "",
        };
        let err_indicator = self.last_error.as_ref().map(|e| format!(" [ERR: {}]", e)).unwrap_or_default();
        let status_indicator = self.last_status.as_ref().map(|s| format!(" [! {}]", s)).unwrap_or_default();
        format!(
            "Local playlist - {} songs{}{}{}{}{}{}{}",
            self.list.get_list_iter().len(),
            quality_indicator,
            shuffle_indicator,
            search_indicator,
            cat_indicator,
            romaji_indicator,
            err_indicator,
            status_indicator,
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
            category_filter: None,
            romaji_mode: false,
            search_cur: 0,
            scrobble_state: None,
            scrobble_pending: false,
            scrobbling_config: crate::config::ScrobblingConfig::default(),
            romaji_originals: HashMap::new(),
            album_tracks: None,
            album_current_track: 0,
            yt_dlp_cookie_path: None,
            repeat_mode: crate::app::structures::RepeatMode::Off,
            radio_mode: false,
            last_error: None,
            last_status: None,
            pending_playlist_chunks: None,
            undo_stack: Vec::new(),
            visual_mode: false,
            visual_start: 0,
            yank_buffer: Vec::new(),
            album_art_fetching: false,
            album_art_fetching_name: None,
            pending_count: 0,
            search_text: String::new(),
            search_indices: Vec::new(),
            pre_search_selected: 0,
            sort_mode: false,
            sort_column: 0,
            sort_direction: SortDirection::Asc,
            library_playlist_mutated: false,
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

    /// Create track entries from an album tracklist.
    /// Each track gets: track_no, start_offset (accumulated) for ffmpeg seeking,
    /// actual_duration for gapless/scrobble, year from validation or yt-dlp fallback.
    /// If original is already downloaded (src_arc.is_some()): shares Arc, removes original, plays track 1.
    /// Returns Option<ComponentEffect> for auto-play when download is ready.
    pub fn insert_album_tracks(
        &mut self,
        song_id: ListSongID,
        tracks: &[AlbumTrack],
        artist: &Option<String>,
        album: &Option<String>,
        year: &Option<String>,
        original_album: &Option<String>,
    ) -> Option<ComponentEffect<Self>> {
        let Some(src_idx) = self.get_index_from_id(song_id) else { return None; };

        // Extract all data from src_song before any mutable borrows
        let (video_raw, album_artist, album_year, src_arc, parent_duration, src_genres, src_styles, src_thumbnails, src_album_art, src_like_status) = {
            let Some(src_song) = self.get_song_from_idx(src_idx) else { return None; };
            let video_raw = src_song.video_id.get_raw().to_string();
            let album_artist = crate::app::structures::normalize_artist_name(&artist.clone().unwrap_or_else(|| {
                src_song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")
            }));
            // Use validated year, fallback to yt-dlp year on original entry
            let album_year = year.clone().or_else(|| {
                src_song.year.as_ref().map(|y| y.to_string())
            }).map(std::rc::Rc::new);
            let src_arc = match &src_song.download_status {
                DownloadStatus::Downloaded(arc) => Some(arc.clone()),
                _ => None,
            };
            let parent_duration = src_song.actual_duration;
            let src_genres = src_song.genres.clone();
            let src_styles = src_song.styles.clone();
            let src_thumbnails = src_song.thumbnails.clone();
            let src_album_art = src_song.album_art.clone();
            let src_like_status = src_song.like_status.clone();
            (video_raw, album_artist, album_year, src_arc, parent_duration, src_genres, src_styles, src_thumbnails, src_album_art, src_like_status)
        };
        // Use metadata-provided album name, fall back to original YouTube title
        let album_name = album.clone().or_else(|| original_album.clone());

        // Guard: skip if tracks already exist for this video_id (cascade prevention)
        let existing_tracks = self.list.get_list_iter()
            .filter(|s| s.video_id.get_raw() == video_raw)
            .filter(|s| s.track_no.is_some())
            .count();
        if existing_tracks >= tracks.len() {
            info!("insert_album_tracks: {} tracks already exist, skipping insert", existing_tracks);
            if let Some(arc) = src_arc {
                for i in 0..tracks.len() {
                    let track_idx = src_idx + i + 1;
                    if let Some(song) = self.list.get_list_iter_mut().nth(track_idx) {
                        if song.video_id.get_raw() == video_raw && matches!(song.download_status, DownloadStatus::None) {
                            song.download_status = DownloadStatus::Downloaded(arc.clone());
                        }
                    }
                }
            }
            return None;
        }

        let mut accum = 0.0;
        for (i, track) in tracks.iter().enumerate() {
            let dur_secs = track.duration_secs as u64;
            let dur_str = format!("{}:{:02}", dur_secs / 60, dur_secs % 60);
            let list_artists = vec![crate::app::structures::ListSongArtist {
                name: album_artist.clone(),
                id: None,
            }];
            let list_album = album_name.as_ref().map(|n| {
                crate::app::structures::MaybeRc::Owned(crate::app::structures::ListSongAlbum {
                    name: n.clone(),
                    id: crate::app::structures::AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
                })
            });
            use ytmapi_rs::common::YoutubeID;
            let video_id: ytmapi_rs::common::VideoID<'static> = ytmapi_rs::common::VideoID::from_raw(video_raw.clone());
            let is_last = i == tracks.len() - 1;
            let list_song = crate::app::structures::ListSong {
                video_id,
                track_no: Some(i + 1),
                plays: String::new(),
                title: track.title.clone(),
                explicit: None,
                download_status: DownloadStatus::None,
                id: self.list.create_next_id(),
                duration_string: dur_str,
                actual_duration: if is_last {
                    // Fill remaining time up to parent duration for last track
                    parent_duration.map(|total| {
                        let remaining = total.as_secs_f64() - accum;
                        if remaining > 0.0 {
                            std::time::Duration::from_secs_f64(remaining)
                        } else {
                            std::time::Duration::from_secs_f64(track.duration_secs)
                        }
                    })
                } else {
                    Some(std::time::Duration::from_secs_f64(track.duration_secs))
                },
                start_offset: Some(std::time::Duration::from_secs_f64(accum)),
                year: album_year.clone(),
                album_art: src_album_art.clone(),
                genres: src_genres.clone(),
                styles: src_styles.clone(),
                artists: crate::app::structures::MaybeRc::Owned(list_artists),
                thumbnails: src_thumbnails.clone(),
                album: list_album,
                like_status: src_like_status.clone(),
            };
            self.list.insert_after(src_idx + i, list_song);
            accum += track.duration_secs;
        }
        info!("insert_album_tracks: inserted {} tracks after index {}", tracks.len(), src_idx);

        // If original is already downloaded, share Arc with new track entries
        if let Some(arc) = src_arc {
            for i in 0..tracks.len() {
                let track_idx = src_idx + i + 1;
                if let Some(song) = self.list.get_list_iter_mut().nth(track_idx) {
                    if song.video_id.get_raw() == video_raw && matches!(song.download_status, DownloadStatus::None) {
                        song.download_status = DownloadStatus::Downloaded(arc.clone());
                    }
                }
            }
            info!("insert_album_tracks: shared downloaded audio, removing original");
            // Remove original entry (Arc stays alive via track clones)
            self.list.remove_at(src_idx);
            // Auto-play track 1 since everything is ready
            if let Some(track1_id) = self.list.get_list_iter()
                .find(|s| s.track_no == Some(1) && s.video_id.get_raw() == video_raw)
                .map(|s| s.id)
            {
                let play_effect = self.play_song_id(track1_id);
                return Some(play_effect);
            }
        }
        None
    }

    /// Handle ForceSplitAlbum: find/reconstruct parent, clear split tracks, re-validate metadata.
    fn handle_force_split(&mut self, selected_idx: usize) -> ComponentEffect<Self> {
        let Some(song) = self.get_song_from_idx(selected_idx) else {
            self.last_error = Some("No song selected for force-split".into());
            return AsyncTask::new_no_op();
        };
        let video_raw = song.video_id.get_raw().to_string();
        let parent = self.list.get_list_iter().enumerate()
            .find(|(_, s)| s.video_id.get_raw() == video_raw && s.track_no.is_none())
            .map(|(i, s)| (i, s.id));
        let parent_id: ListSongID;
        let parent_idx: usize;
        if let Some((idx, pid)) = parent {
            parent_id = pid;
            parent_idx = idx;
            let to_remove: Vec<usize> = self.list.get_list_iter().enumerate()
                .filter(|(_, t)| t.video_id.get_raw() == video_raw && t.track_no.is_some())
                .map(|(i, _)| i)
                .collect();
            for i in to_remove.into_iter().rev() {
                self.list.remove_at(i);
            }
        } else {
            let t1_idx = self.list.get_list_iter().enumerate()
                .find(|(_, t)| t.video_id.get_raw() == video_raw && t.track_no == Some(1))
                .map(|(i, t)| (i, t.id));
            let Some((t1_idx, t1_id)) = t1_idx else {
                self.last_error = Some("No parent or track 1 found for force-split".into());
                return AsyncTask::new_no_op();
            };
            let to_remove: Vec<usize> = self.list.get_list_iter().enumerate()
                .filter(|(i, t)| t.video_id.get_raw() == video_raw && t.track_no.is_some() && *i != t1_idx)
                .map(|(i, _)| i)
                .collect();
            for i in to_remove.into_iter().rev() {
                self.list.remove_at(i);
            }
            if let Some(t) = self.list.get_list_iter_mut().nth(t1_idx) {
                t.track_no = None;
                t.start_offset = None;
            }
            parent_id = t1_id;
            parent_idx = t1_idx;
        }
        self.album_tracks = None;
        self.album_current_track = 0;
        self.cur_selected = parent_idx;
        let Some(p_song) = self.get_song_from_id(parent_id) else {
            self.last_error = Some("Parent song not found after re-indexing".into());
            return AsyncTask::new_no_op();
        };
        let artist = p_song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
        let clean_title = p_song.title.clone();
        if artist.is_empty() || clean_title.is_empty() {
            self.last_error = Some("Could not determine artist/title for re-validation".into());
            return AsyncTask::new_no_op();
        }
        let p_album = p_song.album.as_ref().map(|a| a.name.clone());
        info!("Force-split: re-validating song {:?} (artist={:?}, title={:?}, album={:?})", parent_id, artist, clean_title, p_album);
        let validation_task = AsyncTask::new_future_try(
            ValidateMetadata(artist, clean_title, parent_id, self.scrobbling_config.api_key.clone(), Some(self.scrobbling_config.discogs_token.clone()).filter(|s| !s.is_empty()), p_album),
            HandleMetadataValidated(parent_id),
            HandleMetadataValidationError,
            None,
        );
        let effect = self.download_upcoming_from_id(parent_id);
        effect.push(validation_task)
    }

    pub fn update_song_info(&mut self, id: ListSongID, song: ListSong) {
        if let Some(idx) = self.get_index_from_id(id) {
            if let Some(existing) = self.list.get_list_iter_mut().nth(idx) {
                existing.title = song.title;
                existing.artists = song.artists;
                existing.album = song.album;
                existing.year = song.year;
                existing.track_no = song.track_no;
                info!("Updated song info for {:?}", id);
            }
        }
    }

    pub fn set_scrobbling_config(&mut self, config: crate::config::ScrobblingConfig) {
        self.scrobbling_config = config;
    }

    /// Strip "{artist} - " prefix from title (case-insensitive) for clean metadata lookup
    fn strip_artist_prefix(artist: &str, title: &str) -> String {
        let lower = title.to_lowercase();
        let art_lower = artist.to_lowercase();
        if lower.starts_with(&format!("{} - ", art_lower)) {
            title[artist.len() + 3..].trim().to_string()
        } else if artist.len() >= 2 && lower.starts_with(&art_lower) && !lower[art_lower.len()..].starts_with(&art_lower) {
            title[artist.len()..].trim().to_string()
        } else {
            title.to_string()
        }
    }

    /// Strip YouTube noise patterns (official audio, lyrics, etc.) from title end
    fn strip_youtube_noise(title: &str) -> String {
        let noise_tags = [
            "official audio", "official video", "lyric video", "lyrics",
            "legendado", "c legendado", "c legenda", "com legenda",
            "com legendado", "legendado pt", "legendado pt-br",
            "subtitle", "subtitles",
        ];
        let mut s = title.to_string();
        loop {
            let lower = s.to_lowercase().trim().to_string();
            let mut found = false;
            for tag in &noise_tags {
                if let Some(pos) = lower.rfind(tag) {
                    let before = &s[..pos].trim();
                    let cut = if let Some(paren_start) = before.rfind('(') {
                        let between = &before[paren_start..];
                        if between.to_lowercase().contains(tag) {
                            &before[..paren_start.max(1).saturating_sub(1)]
                        } else {
                            &s[..pos]
                        }
                    } else {
                        &s[..pos]
                    };
                    if cut.len() < s.len() {
                        s = cut.trim().to_string();
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                s = s.trim_end_matches(|c| c == '(').trim().to_string();
                break;
            }
        }
        s.trim().to_string()
    }

    /// Strip parenthesized/bracketed groups containing album metadata tags (album, ep, demo, etc.)
    fn strip_album_metadata_tags(title: &str) -> String {
        let mut s = title.to_string();
        let tags = [
            "studio album", "live album", "full-length album", "full-length",
            "full album", "full ep", "full lp", "full demo", "full single",
            "official album", "compilation", "bootleg", "anthology", "collection",
            "self-titled", "self titled", "s/t",
            "single", "demo", "ep", "lp", "album", "singles",
        ];
        loop {
            let chars: Vec<char> = s.chars().collect();
            let mut modified = false;
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '(' || chars[i] == '[' {
                    let open = i;
                    let mut depth = 1;
                    i += 1;
                    while i < chars.len() && depth > 0 {
                        if chars[i] == '(' || chars[i] == '[' { depth += 1; }
                        else if chars[i] == ')' || chars[i] == ']' { depth -= 1; }
                        i += 1;
                    }
                    if depth == 0 {
                        let group: String = chars[open..i].iter().collect();
                        let group_lower = group.to_lowercase();
                        let group_tokens: Vec<&str> = group_lower
                            .split(|c: char| !c.is_alphanumeric())
                            .filter(|t| !t.is_empty())
                            .collect();
                        let has_tag = tags.iter().any(|tag| {
                            let tag_tokens: Vec<&str> = tag.split_whitespace().collect();
                            if tag_tokens.is_empty() { return false; }
                            group_tokens.windows(tag_tokens.len())
                                .any(|w| w == tag_tokens.as_slice())
                        });
                        if has_tag {
                            let before: String = chars[..open].iter().collect();
                            let after: String = chars[i..].iter().collect();
                            s = format!("{}{}", before.trim(), after.trim()).trim().to_string();
                            modified = true;
                            break;
                        }
                    }
                } else {
                    i += 1;
                }
            }
            if !modified { break; }
        }
        s.trim_end_matches(|c: char| c == '(' || c == '[' || c == '-' || c == ',' || c == '.')
            .trim().to_string()
    }

    /// Strip extracted year from last parenthesized group in title
    fn strip_year_from_title(title: &str) -> String {
        let lower = title.to_lowercase();
        if let Some(paren) = lower.rfind("(") {
            let inner = lower[paren..].trim_matches(|c| c == '(' || c == ')' || c == ' ');
            if inner.split(|c: char| !c.is_ascii_digit())
                .find(|p| p.len() == 4)
                .and_then(|p| {
                    let y = p.parse::<u16>().ok()?;
                    if (1900..2100).contains(&y) { Some(y) } else { None }
                })
                .is_some()
            {
                title[..paren].trim().to_string()
            } else {
                title.to_string()
            }
        } else {
            title.to_string()
        }
    }

    /// Clean song title for metadata lookup: strip artist prefix, noise tags, album metadata tags, year
    fn clean_title_for_metadata(artist: &str, title: &str) -> String {
        let s = Self::strip_artist_prefix(artist, title);
        let s = Self::strip_youtube_noise(&s);
        let s = Self::strip_album_metadata_tags(&s);
        Self::strip_year_from_title(&s)
    }

    pub fn add_yt_video(&mut self, video_id: ytmapi_rs::common::VideoID<'static>, url: &str) -> ComponentEffect<Self> {
        use ytmapi_rs::common::YoutubeID;
        let raw_id = video_id.get_raw().to_string();
        tracing::info!("add_yt_video: {} {}", raw_id, url);

        // Dedup: skip if video_id already exists in playlist
        if self.list.get_list_iter().any(|s| s.video_id.get_raw() == raw_id) {
            info!("add_yt_video: {} already in playlist, skipping", raw_id);
            return AsyncTask::new_no_op();
        }

        // Fetch metadata via yt-dlp
        let mut duration = String::from("0");
        let mut meta_cmd = std::process::Command::new("yt-dlp");
        meta_cmd.args(["--dump-json", "--no-warnings", "--flat-playlist"]);
        if self.yt_dlp_cookie_path.is_some() {
            meta_cmd.args(["--cookies-from-browser", "chromium"]);
        }
        meta_cmd.arg(&format!("https://youtu.be/{}", raw_id));
        let (title, artist, year) = match meta_cmd.output()
        {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    let t = v.get("title").and_then(|s| s.as_str()).unwrap_or(&raw_id).to_string();
                    let uploader = v.get("uploader").and_then(|s| s.as_str()).unwrap_or("Unknown").to_string();
                    // Try to extract real artist from title ("Artist - Song"), fallback to uploader
                    let a = if t.contains(" - ") {
                        t.splitn(2, " - ").next().map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty() && s.len() < 80)
                            .unwrap_or_else(|| uploader.clone())
                    } else {
                        uploader.clone()
                    };
                    if let Some(d) = v.get("duration").and_then(|s| s.as_f64()) {
                        let secs = d as u64;
                        duration = format!("{}:{:02}", secs / 60, secs % 60);
                    }
                    let year = v.get("release_year")
                        .and_then(|s| s.as_i64())
                        .or_else(|| {
                            v.get("upload_date")
                                .and_then(|s| s.as_str())
                                .and_then(|d| d.get(..4))
                                .and_then(|y| y.parse::<i64>().ok())
                        })
                        .map(|y| y.to_string());
                    (t, a, year)
                } else { (raw_id.clone(), "YouTube".to_string(), None) }
            }
            _ => (raw_id.clone(), "YouTube".to_string(), None),
        };

        // Extract year from title parenthetical as fallback when yt-dlp has no year
        // e.g., "Anti-Everything E.P. (2003)" → "2003", "Scat Blast FULL ALBUM (2021...)" → "2021"
        let title_year = if year.is_none() {
            title.split('(').nth(1).and_then(|after_paren| {
                after_paren.split(')').next().and_then(|inner| {
                    inner.split(|c: char| !c.is_ascii_digit())
                        .find(|p| p.len() == 4)
                        .and_then(|p| p.parse::<u16>().ok())
                        .filter(|y| (1900..2100).contains(y))
                        .map(|y| y.to_string())
                })
            })
        } else {
            None
        };
        let year = year.or(title_year);

        let clean_title = Self::clean_title_for_metadata(&artist, &title);
        let song = ytmapi_rs::parse::SearchResultSong::from_yt_dlp(
            clean_title.clone(), artist.clone(), video_id, None, format!("{}", duration),
        );
        let old_count = self.list.get_list_iter().count();
        let id = self.list.append_raw_search_result_songs(vec![song]);
        if self.list.get_list_iter().count() > old_count {
            self.cur_selected = self.list.get_list_iter().count().saturating_sub(1);
            // Set initial album name from YouTube video title (before metadata overwrites)
            if let Some(idx) = self.get_index_from_id(id) {
                if let Some(s) = self.list.get_list_iter_mut().nth(idx) {
                    s.album = Some(crate::app::structures::MaybeRc::Owned(
                        crate::app::structures::ListSongAlbum {
                            name: clean_title.clone(),
                            id: AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
                        },
                    ));
                }
            }
            if let Some(year) = year {
                if let Some(idx) = self.get_index_from_id(id) {
                    if let Some(s) = self.list.get_list_iter_mut().nth(idx) {
                        s.year = Some(std::rc::Rc::new(year));
                    }
                }
            }
            let album_name = if let Some(idx) = self.get_index_from_id(id) {
                self.get_song_from_idx(idx).and_then(|s| s.album.as_ref().map(|a| a.name.clone()))
            } else { None };
            // Spawn metadata validation (Last.fm -> MusicBrainz) in parallel with download
            let validation_task = AsyncTask::new_future_try(
                ValidateMetadata(artist, clean_title, id, self.scrobbling_config.api_key.clone(), Some(self.scrobbling_config.discogs_token.clone()).filter(|s| !s.is_empty()), album_name),
                HandleMetadataValidated(id),
                HandleMetadataValidationError,
                None,
            );
            if let Some(song_id) = self.get_id_from_index(self.cur_selected) {
                let dl_effect = self.download_upcoming_from_id(song_id);
                return dl_effect.push(validation_task);
            }
        }
        AsyncTask::new_no_op()
    }

    pub fn play_song_id(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        // Guard: don't re-decode if already Playing the same id
        if matches!(self.play_status, PlayState::Playing(cur) | PlayState::Paused(cur) if cur == id) {
            return AsyncTask::new_no_op();
        }
        self.drop_unscoped_from_id(id);

        let mut effect = self.download_upcoming_from_id(id);

        self.cur_played_dur = None;

        if let Some(song_index) = self.get_index_from_id(id) {
            let (pointer, offset, actual_dur) = {
                let song = self.get_song_from_idx(song_index).expect("Checked previously");
                match &song.download_status {
                    DownloadStatus::Downloaded(p) => (p.clone(), song.start_offset, song.actual_duration),
                    _ => {
                        let maybe_effect = self.get_cur_playing_id().map(|cur_id| self.stop_song_id(cur_id));
                        self.play_status = PlayState::Buffering(id);
                        self.queue_status = QueueState::NotQueued;
                        if let Some(stop_effect) = maybe_effect {
                            effect = effect.push(stop_effect);
                        }
                        return effect;
                    }
                }
            };
            let task = DecodeSong(pointer, offset, actual_dur).map_stream(PlayDecodedSong(id));
            let constraint = Some(Constraint::new_block_matching_metadata(
                TaskMetadata::PlayingSong,
            ));
            let mut effect = effect.push(AsyncTask::new_stream_try(
                task,
                HandlePlayUpdateOk,
                HandlePlayUpdateError(id),
                constraint,
            ));
            self.play_status = PlayState::Playing(id);
            self.queue_status = QueueState::NotQueued;
            if self.scrobbling_config.enabled {
                self.scrobble_pending = false;
                if let Some(old) = self.scrobble_state.take() {
                    if old.should_scrobble() {
                        let cfg = self.scrobbling_config.clone();
                        tokio::spawn(async move {
                            crate::app::scrobbler::submit_scrobble(&cfg, &old).await;
                        });
                    }
                }
                let is_track_entry = self.get_song_from_idx(song_index).and_then(|s| s.track_no).is_some();
                // Trigger FetchAlbumArt if not already downloaded (with throttle)
                if let Some(song) = self.get_song_from_idx(song_index) {
                    let should_fetch = matches!(song.album_art, crate::app::structures::AlbumArtState::None | AlbumArtState::Init)
                        && !self.album_art_fetching
                        && self.cur_played_dur.map_or(false, |d| d.as_secs() > 5 || d.is_zero());
                    if should_fetch {
                        if let Some(ref album) = song.album {
                            let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                            let album_name = album.name.clone();
                            let api_key = self.scrobbling_config.api_key.clone();
                            if !api_key.is_empty() {
                                self.album_art_fetching = true;
                                self.album_art_fetching_name = Some(album_name.clone());
                                effect = effect.push(AsyncTask::new_future_try(
                                    crate::app::server::FetchAlbumArt(artist, album_name, api_key),
                                    HandleFetchAlbumArtOk,
                                    HandleFetchAlbumArtErr,
                                    None,
                                ).map_frontend(|this: &mut Self| this));
                            }
                        }
                    }
                }
                if self.album_tracks.is_none() || is_track_entry {
                    if let Some(song) = self.get_song_from_idx(song_index) {
                        let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                        let album = song.album.as_ref().map(|a| a.name.clone());
                        let dur = song.actual_duration.unwrap_or(std::time::Duration::from_secs(240));
                        self.scrobble_state = Some(crate::app::scrobbler::ScrobbleState::new(artist, song.title.clone(), album, dur));
                    }
                } else {
                    // Album mode: boundary checker handles per-track scrobbling
                    self.album_current_track = 0;
                    info!("Album mode: started playback, will scrobble {} tracks", self.album_tracks.as_ref().map_or(0, |t| t.len()));
                }
            }
            return effect;
        }
        effect
    }

    pub fn autoplay_song_id(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        self.drop_unscoped_from_id(id);

        let mut effect = self.download_upcoming_from_id(id);

        self.cur_played_dur = None;

        if let Some(song_index) = self.get_index_from_id(id) {
            let (pointer, offset, actual_dur) = {
                let song = self.get_song_from_idx(song_index).expect("Checked previously");
                match &song.download_status {
                    DownloadStatus::Downloaded(p) => (p.clone(), song.start_offset, song.actual_duration),
                    _ => {
                        let maybe_effect = self.get_cur_playing_id().map(|cur_id| self.stop_song_id(cur_id));
                        self.play_status = PlayState::Buffering(id);
                        self.queue_status = QueueState::NotQueued;
                        if let Some(stop_effect) = maybe_effect {
                            effect = effect.push(stop_effect);
                        }
                        return effect;
                    }
                }
            };
            let task = DecodeSong(pointer, offset, actual_dur).map_stream(AutoplayDecodedSong(id));
            let effect = effect.push(AsyncTask::new_stream_try(
                task,
                HandleAutoplayUpdateOk,
                HandlePlayUpdateError(id),
                None,
            ));
            self.play_status = PlayState::Playing(id);
            self.queue_status = QueueState::NotQueued;
            return effect;
        }
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

    /// Called when a song finishes downloading.
    /// If this is the original album entry (track_no.is_none()) and album_tracks exist:
    /// shares the Arc<InMemSong> with all track entries, waits for ALL tracks to have Downloaded
    /// status, then auto-plays track 1 and removes the original entry from the list.
    /// If tracks aren't ready yet (validation hasn't completed): returns without playing.
    pub fn handle_song_downloaded(&mut self, id: ListSongID) -> ComponentEffect<Self> {
        let start = std::time::Instant::now();
        info!("handle_song_downloaded ENTER: id={:?}, state={:?}, queue={:?}",
            id, self.play_status, self.queue_status);

        // If album tracks are present and this is the ORIGINAL entry (not a track), share Arc with track entries
        if let Some(ref tracks) = self.album_tracks.clone() {
            let mut effect = AsyncTask::new_no_op();
            let is_original = self.get_song_from_id(id).map(|s| s.track_no.is_none()).unwrap_or(false);
            if tracks.len() >= 2 && is_original {
                let original_arc = self.get_song_from_id(id)
                    .and_then(|s| match &s.download_status {
                        DownloadStatus::Downloaded(arc) => Some(arc.clone()),
                        _ => None,
                    });
                if let Some(arc) = original_arc {
                    let video_raw = self.get_song_from_id(id)
                        .map(|s| s.video_id.get_raw().to_string())
                        .unwrap_or_default();
                    let indices: Vec<usize> = self.list.get_list_iter().enumerate()
                        .filter(|(_, s)| s.track_no.is_some() && s.video_id.get_raw() == video_raw)
                        .filter(|(_, s)| matches!(s.download_status, DownloadStatus::None))
                        .map(|(i, _)| i)
                        .collect();
                    let count = indices.len();
                    for idx in &indices {
                        if let Some(song) = self.list.get_list_iter_mut().nth(*idx) {
                            song.download_status = DownloadStatus::Downloaded(arc.clone());
                        }
                    }
                    info!("handle_song_downloaded: shared album audio with {} track entries", count);
                }
            }

            // For albums: don't play original, instead auto-play track 1 when all ready
            if is_original && tracks.len() >= 2 {
                // Check if all track entries exist and have downloaded audio
                let video_raw = self.get_song_from_id(id)
                    .map(|s| s.video_id.get_raw().to_string())
                    .unwrap_or_default();
                let all_ready = self.list.get_list_iter()
                    .filter(|s| s.track_no.is_some() && s.video_id.get_raw() == video_raw)
                    .all(|s| matches!(s.download_status, DownloadStatus::Downloaded(_)));
                if all_ready {
                    if let Some(track1_id) = self.list.get_list_iter()
                        .find(|s| s.track_no == Some(1) && s.video_id.get_raw() == video_raw)
                        .map(|s| s.id)
                    {
                        info!("Album tracks ready, playing track 1");
                        let play_effect = self.play_song_id(track1_id);
                        effect = effect.push(play_effect);
                    }
                    // Remove original entry ONLY if tracks were actually created
                    let actual_tracks = self.list.get_list_iter()
                        .filter(|s| s.track_no.is_some() && s.video_id.get_raw() == video_raw)
                        .count();
                    if actual_tracks > 0 {
                        if let Some(idx) = self.get_index_from_id(id) {
                            self.list.remove_at(idx);
                            info!("handle_song_downloaded: removed original entry ({} tracks)", actual_tracks);
                        }
                    } else {
                        info!("handle_song_downloaded: no tracks created, keeping original entry");
                    }
                } else {
                    info!("Album download complete but waiting for validation (tracks not ready)");
                }
                return effect;
            }

            let should_play = match self.play_status {
                PlayState::Buffering(target_id) => { target_id == id }
                PlayState::NotPlaying | PlayState::Stopped => true,
                PlayState::Playing(cur) => cur == id,
                PlayState::Paused(cur) => cur == id,
                PlayState::Error(_) => false,
            };
            if should_play {
                let play_effect = self.play_song_id(id);
                effect = effect.push(play_effect);
            }
            return effect;
        }

        let should_play = match self.play_status {
            PlayState::Buffering(target_id) => { target_id == id }
            PlayState::NotPlaying | PlayState::Stopped => true,
            PlayState::Playing(cur) => cur == id,
            PlayState::Paused(cur) => cur == id,
            PlayState::Error(_) => false,
        };
        if should_play {
            info!("play_attempt: song_id={:?}, state={:?}, ms_since_download={}",
                id, self.play_status, start.elapsed().as_millis());
            let effect = self.play_song_id(id);
            info!("play_started: song_id={:?}, ms_to_start={}", id, start.elapsed().as_millis());
            return effect;
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

    /// Extract thumbnails from a song list and spawn download tasks.
    /// Sets `song.album_art = AlbumArtState::None` for songs without thumbnails.
    fn collect_thumbnail_tasks(song_list: &mut [ListSong]) -> ComponentEffect<Self> {
        let albums = song_list.iter_mut().filter_map(|song| {
            let thumb_url = song.thumbnails.as_ref().iter()
                .max_by_key(|t| t.height * t.width)
                .map(|t| t.url.clone());
            let Some(thumb_url) = thumb_url else {
                song.album_art = AlbumArtState::None;
                return None;
            };
            let thumb_url = upgrade_thumbnail_url(&thumb_url);
            let thumbnail_id = SongThumbnailID::from(song as &ListSong).into_owned();
            Some((thumbnail_id, thumb_url))
        }).collect::<HashMap<SongThumbnailID, String>>();
        albums.into_iter().map(|(thumbnail_id, thumbnail_url)| {
            AsyncTask::new_future_try(
                GetSongThumbnail { thumbnail_url, thumbnail_id: thumbnail_id.clone() },
                HandleGetSongThumbnailOk,
                HandleGetSongThumbnailError(thumbnail_id),
                None,
            )
        }).collect()
    }

    pub fn push_song_list(
        &mut self,
        mut song_list: Vec<ListSong>,
    ) -> (ListSongID, ComponentEffect<Self>) {
        let effect = Self::collect_thumbnail_tasks(&mut song_list);

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

        // Spawn metadata validation for first added song (enables album splitting)
        if let Some(song) = self.get_song_from_id(first_id) {
            let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
            if !artist.is_empty() {
                let clean_title = Self::clean_title_for_metadata(&artist, &song.title);
                let album = song.album.as_ref().map(|a| a.name.clone());
                let validation_task = AsyncTask::new_future_try(
                    ValidateMetadata(artist, clean_title, first_id, self.scrobbling_config.api_key.clone(),
                        Some(self.scrobbling_config.discogs_token.clone()).filter(|s| !s.is_empty()), album),
                    HandleMetadataValidated(first_id),
                    HandleMetadataValidationError,
                    None,
                );
                return (first_id, effect.push(validation_task));
            }
        }
        (first_id, effect)
    }

    pub fn insert_next_song_list(
        &mut self,
        mut song_list: Vec<ListSong>,
    ) -> (ListSongID, ComponentEffect<Self>) {
        let effect = Self::collect_thumbnail_tasks(&mut song_list);

        let insert_pos = self.get_cur_playing_index().map(|i| i + 1).unwrap_or(0);
        let first_id = self.list.insert_song_list_at(song_list, insert_pos);
        if self.shuffle_enabled {
            self.generate_shuffle_indices();
            if let Some(playing_idx) = self.get_cur_playing_index() {
                if let Some(shuffled_pos) = self.shuffle_indices.iter().position(|&i| i == playing_idx) {
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

                if self.repeat_mode == crate::app::structures::RepeatMode::One {
                    info!("Repeat One: replaying current track");
                    self.play_song_id(*id)
                } else if let Some(next_song_id) = self.get_next_song_id(*id) {
                    self.autoplay_song_id(next_song_id)
                } else if self.radio_mode {
                    info!("Radio mode active, auto-play stopped. Fetch recommendations on next run.");
                    self.queue_status = QueueState::NotQueued;
                    self.stop_song_id(*id)
                } else {
                    match self.repeat_mode {
                        crate::app::structures::RepeatMode::All => {
                            if let Some(first_id) = self.get_id_from_index(0) {
                                self.play_song_id(first_id)
                            } else {
                                self.queue_status = QueueState::NotQueued;
                                self.stop_song_id(*id)
                            }
                        }
                        crate::app::structures::RepeatMode::One => {
                            self.play_song_id(*id)
                        }
                        crate::app::structures::RepeatMode::Off => {
                            info!("No next song - resetting play status");
                            self.queue_status = QueueState::NotQueued;
                            self.stop_song_id(*id)
                        }
                    }
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
                let thumb_url = get_largest_thumbnail_url(song.thumbnails.as_ref())?;
                let thumb_url = upgrade_thumbnail_url(&thumb_url);
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

    pub fn get_selected_album_art(&self) -> Option<std::rc::Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>> {
        let actual = self.visual_to_actual_index(self.cur_selected);
        self.get_song_from_idx(actual).and_then(|s| match &s.album_art {
            crate::app::structures::AlbumArtState::Downloaded(t) => Some(t.clone()),
            _ => None,
        })
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
        if self.scrobbling_config.enabled {
            self.scrobble_pending = false;
            if let Some(old) = self.scrobble_state.take() {
                if old.should_scrobble() {
                    let cfg = self.scrobbling_config.clone();
                    tokio::spawn(async move {
                        crate::app::scrobbler::submit_scrobble(&cfg, &old).await;
                    });
                }
            }
        }
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
        if self.sort_mode {
            let field = sort_column_to_field(self.sort_column);
            self.list.sort(field, self.sort_direction);
            self.sort_mode = false;
            return AsyncTask::new_no_op();
        }
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
        if self.visual_mode {
            // Delete visual range (visual_start to cur_selected)
            let (start, end) = if self.visual_start <= self.cur_selected {
                (self.visual_start, self.cur_selected)
            } else {
                (self.cur_selected, self.visual_start)
            };
            let all_songs: Vec<_> = self.list.get_list_iter().cloned().collect();
            let total = all_songs.len();
            let mut to_delete: Vec<(ListSong, usize)> = Vec::new();
            for idx in start..=end {
                let actual = self.visual_to_actual_index(idx);
                if actual < total {
                    to_delete.push((all_songs[actual].clone(), actual));
                }
            }
            self.undo_stack.push(to_delete.clone());
            // Remove from end to start so indices stay valid
            for (_, actual) in to_delete.iter().rev() {
                self.list.remove_song_index(*actual);
            }
            self.visual_mode = false;
            self.cur_selected = start.min(self.list.get_list_iter().count().saturating_sub(1));
            let mut return_task = AsyncTask::new_no_op();
            return_task = return_task.push(self.regenerate_downloads_for_current());
            return return_task;
        }
        // FIX: Don't delete if search is active but has no results
        if !self.search_text.is_empty() && self.search_indices.is_empty() {
            return AsyncTask::new_no_op();
        }

        let mut return_task = AsyncTask::new_no_op();

        if self.list.get_list_iter().len() == 0 {
            return return_task;
        }

        // Count-based delete: delete N items from current position
        let count = self.pending_count.max(1);
        self.pending_count = 0;

        // Delete up to `count` items one by one
        for _ in 0..count {
            if self.cur_selected >= self.list.get_list_iter().len() {
                break;
            }
            let visual_index_before = self.cur_selected;
            let actual_index = self.visual_to_actual_index(visual_index_before);

            if let Some(cur_playing_id) = self.get_cur_playing_id() {
                if Some(actual_index) == self.get_cur_playing_index() {
                    self.play_status = PlayState::NotPlaying;
                    return_task = self.stop_song_id(cur_playing_id);
                }
            }

            // Save to undo stack
            if let Some(song) = self.list.get_list_iter().nth(actual_index).cloned() {
                self.undo_stack.push(vec![(song, actual_index)]);
            }

            self.list.remove_song_index(actual_index);

            // Rebuild search if active
            if !self.search_text.is_empty() {
                self.update_search_indices();
                if self.search_indices.is_empty() {
                    self.cur_selected = 0;
                    break;
                }
                self.cur_selected = self.cur_selected.min(self.search_indices.len() - 1);
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
        }

        return_task = return_task.push(self.regenerate_downloads_for_current());

        return_task
    }

    pub fn delete_all(&mut self) -> ComponentEffect<Self> {
        self.reset()
    }

    pub fn delete_to_top(&mut self) -> ComponentEffect<Self> {
        let idx = self.cur_selected;
        if idx == 0 { return AsyncTask::new_no_op(); }
        let all_songs: Vec<_> = self.list.get_list_iter().cloned().collect();
        let to_delete: Vec<_> = (0..idx).filter_map(|i| {
            let actual = self.visual_to_actual_index(i);
            if actual < all_songs.len() {
                Some((all_songs[actual].clone(), actual))
            } else { None }
        }).collect();
        self.undo_stack.push(to_delete.clone());
        for (_, actual) in to_delete.iter().rev() {
            self.list.remove_song_index(*actual);
        }
        self.cur_selected = 0;
        self.regenerate_downloads_for_current()
    }

    pub fn delete_to_bottom(&mut self) -> ComponentEffect<Self> {
        let idx = self.cur_selected;
        let total = self.list.get_list_iter().count();
        if idx >= total.saturating_sub(1) { return AsyncTask::new_no_op(); }
        let all_songs: Vec<_> = self.list.get_list_iter().cloned().collect();
        let to_delete: Vec<_> = (idx..total).filter_map(|i| {
            let actual = self.visual_to_actual_index(i);
            if actual < all_songs.len() {
                Some((all_songs[actual].clone(), actual))
            } else { None }
        }).collect();
        self.undo_stack.push(to_delete.clone());
        for (_, actual) in to_delete.iter().rev() {
            self.list.remove_song_index(*actual);
        }
        self.regenerate_downloads_for_current()
    }

    pub fn toggle_visual_mode(&mut self) -> ComponentEffect<Self> {
        self.visual_mode = !self.visual_mode;
        if self.visual_mode {
            self.visual_start = self.cur_selected;
        }
        AsyncTask::new_no_op()
    }

    pub fn undo_delete(&mut self) -> ComponentEffect<Self> {
        if let Some(mut batch) = self.undo_stack.pop() {
            batch.sort_by_key(|(_, idx)| *idx);
            for (song, original_idx) in &batch {
                self.list.insert_at(*original_idx, song.clone());
            }
            if let Some((_, first_idx)) = batch.first() {
                let max = self.list.get_list_iter().len().saturating_sub(1);
                self.cur_selected = (*first_idx).min(max);
            }
        }
        self.regenerate_downloads_for_current()
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
        if self.sort_mode {
            self.sort_mode = false;
            return AsyncTask::new_no_op();
        }
        self.search_text.clear();
        self.update_search_indices();
        self.cur_selected = self.cur_selected.min(self.get_max_visual_index());
        AsyncTask::new_no_op()
    }

    pub fn handle_text_entry_action(&mut self, action: TextEntryAction) {
        if action == TextEntryAction::Submit && self.search_enabled {
            self.search_enabled = false;
        }
    }

    fn update_search_indices(&mut self) {
        self.search_cur = 0;

        self.search_indices = self
            .list
            .get_list_iter()
            .enumerate()
            .filter(|(_, song)| {
                if let Some(cat) = self.category_filter {
                    let album_name = song.album.as_ref().map(|a| a.name.as_str()).unwrap_or("");
                    album_name.starts_with(cat)
                } else {
                    true
                }
            })
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

                if fuzzy_match(&self.search_text, &title).is_some()
                    || fuzzy_match(&self.search_text, &album).is_some()
                    || fuzzy_match(&self.search_text, &artist).is_some()
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
        for (_, task) in downloads.iter() {
            task.cancel_token.cancel();
        }
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
            // Mark 0-byte downloads as Failed (prevents sharing empty Arc with album tracks)
            DownloadProgressUpdateType::Completed(song_buf) => {
                let size = song_buf.0.len();
                info!("download_done: song_id={}, size={}", video_id, size);
                if let Some(idx) = self.get_index_from_id(id) {
                    if let Some(s) = self.list.get_list_iter_mut().nth(idx) {
                        if size == 0 {
                            error!("download_done: song {} has 0 bytes, marking as Failed", video_id);
                            s.download_status = DownloadStatus::Failed;
                        } else {
                            s.download_status = DownloadStatus::Downloaded(Arc::new(song_buf));
                            info!("download_status_updated: song_id={} -> Downloaded", video_id);
                        }
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
                self.last_error = Some("Download failed - check yt-dlp".to_string());
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

        let (start_offset, is_album_track) = self.get_song_from_id(id).map(|s| {
            (s.start_offset, s.track_no.is_some())
        }).unwrap_or((None, false));

        // Convert absolute progress to track-relative for album tracks
        // When ffmpeg extraction was used, d is already track-relative
        let track_rel = match start_offset {
            Some(_) if is_album_track => d,
            Some(offset) => d.saturating_sub(offset),
            None => d,
        };
        // Cap at actual_duration so progress never exceeds track boundary
        let capped = match self.get_cur_playing_song().and_then(|s| s.actual_duration) {
            Some(max) => track_rel.min(max),
            None => track_rel,
        };
        self.cur_played_dur = Some(capped);

        // Persistent scrobble: check on every progress update regardless of context
        if self.scrobbling_config.enabled && !self.scrobble_pending {
            if let Some(ref state) = self.scrobble_state.clone() {
                if state.should_scrobble() {
                    self.scrobble_pending = true;
                    let cfg = self.scrobbling_config.clone();
                    let s = state.clone();
                    tokio::spawn(async move {
                        crate::app::scrobbler::submit_scrobble(&cfg, &s).await;
                    });
                    if let Some(s) = self.scrobble_state.as_mut() {
                        s.scrobbled = true;
                        self.scrobble_pending = false;
                    }
                }
            }
        }

        // Album boundary scrobbling: only when playing the ORIGINAL entry (full album)
        if !is_album_track {
            if let Some(ref tracks) = self.album_tracks.clone() {
                if tracks.len() >= 2 {
                    let total_album_dur: f64 = tracks.iter().map(|t| t.duration_secs).sum();
                    let video_dur = self.get_cur_playing_song()
                        .and_then(|s| s.actual_duration)
                        .unwrap_or(d)
                        .as_secs_f64();
                    let elapsed = d.as_secs_f64();
                    let mut accum = 0.0;
                    let mut new_index = 0;
                    for (i, track) in tracks.iter().enumerate() {
                        let track_end = if total_album_dur > 0.0 {
                            (accum + track.duration_secs) / total_album_dur * video_dur
                        } else {
                            accum + track.duration_secs
                        };
                        if elapsed < track_end {
                            new_index = i;
                            break;
                        }
                        accum += track.duration_secs;
                    }
                    if new_index >= tracks.len() { new_index = tracks.len() - 1; }
                    if new_index != self.album_current_track {
                        if new_index > self.album_current_track {
                            for scrobbled_idx in self.album_current_track..new_index {
                                if let Some(track) = tracks.get(scrobbled_idx) {
                                    let cfg = self.scrobbling_config.clone();
                                    let artist = self.get_cur_playing_song()
                                        .map(|s| s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "))
                                        .unwrap_or_default();
                                    let state = crate::app::scrobbler::ScrobbleState::new(
                                        artist,
                                        track.title.clone(),
                                        None,
                                        Duration::from_secs_f64(track.duration_secs),
                                    );
                                    let cfg2 = cfg.clone();
                                    tokio::spawn(async move {
                                        crate::app::scrobbler::submit_scrobble(&cfg2, &state).await;
                                    });
                                    info!("Album track scrobbled: #{} {} ({})", scrobbled_idx + 1, track.title, track.duration_secs);
                                }
                            }
                        }
                        self.album_current_track = new_index;
                    }
                }
            }
        }

        // Gapless auto-advance: uses track-relative progress vs actual_duration
        if let Some(duration_dif) = {
            let cur_dur = self
                .get_cur_playing_song()
                .and_then(|song| song.actual_duration);
            self.cur_played_dur
                .as_ref()
                .zip(cur_dur)
                .map(|(d1, d2)| d2.saturating_sub(*d1))
        } {
            // Repeat One: don't queue next track, replay current via autoplay_next_or_stop
            if self.repeat_mode != crate::app::structures::RepeatMode::One
                && duration_dif
                .saturating_sub(GAPLESS_PLAYBACK_THRESHOLD)
                .is_zero()
                && !matches!(self.queue_status, QueueState::Queued(_))
                && let Some(next_song) = self.get_next_song()
                && let DownloadStatus::Downloaded(song) = &next_song.download_status
            {
                let offset = next_song.start_offset;
                let actual_dur = next_song.actual_duration;
                let task = DecodeSong(song.clone(), offset, actual_dur).map_stream(QueueDecodedSong(id));
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
            if song.start_offset.is_none() || song.actual_duration.is_none() {
                song.actual_duration = duration;
            }
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
            if song.start_offset.is_none() || song.actual_duration.is_none() {
                song.actual_duration = duration;
            }
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
