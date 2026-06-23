use super::songsearch::BrowserSongsAction;
use super::shared_components::{BrowserSearchAction, FilterAction, FilterManager, SearchBlock, SortAction, SortManager};
use crate::app::{AppCallback, NavTarget};
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, KeyRouter, Scrollable, TextHandler, YoutuiEffect,
};
use crate::app::structures::{AlbumOrUploadAlbumID, ListSongAlbum};

use crate::app::server::{
    GetAllLibrarySongs, GetAllLibraryPlaylists, GetAllLibraryArtists, GetAllLibraryAlbums,
    GetPlaylistTracks,
};
use crate::app::structures::{
    BrowserSongsList, ListSong, ListSongArtist, ListSongDisplayableField, ListStatus, MaybeRc,
    DownloadStatus, AlbumArtState, fuzzy_match, Percentage,
};
use crate::app::view::{
    AdvancedTableView, BasicConstraint, TableFilterCommand, TableSortCommand, TableView,
};
use crate::app::ui::browser::shared_components::get_adjusted_list_column;
use crate::app::ui::action::AppAction;
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::widgets::ScrollingTableState;
use async_callback_manager::{AsyncTask, FrontendEffect};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use std::borrow::Cow;
use std::collections::HashSet;
use ytmapi_rs::common::{PlaylistID, YoutubeID, LikeStatus};
use ytmapi_rs::parse::PlaylistSong;
use ytmapi_rs::parse::{LibraryPlaylist, LibraryArtist, SearchResultAlbum, TableListSong};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LibraryCategory {
    #[default]
    LikedSongs,
    Playlists,
    Artists,
    Albums,
}

impl LibraryCategory {
    pub const ALL: [LibraryCategory; 4] = [
        LibraryCategory::LikedSongs,
        LibraryCategory::Playlists,
        LibraryCategory::Artists,
        LibraryCategory::Albums,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            LibraryCategory::LikedSongs => "Liked Songs",
            LibraryCategory::Playlists => "Playlists",
            LibraryCategory::Artists => "Artists",
            LibraryCategory::Albums => "Albums",
        }
    }

    pub fn next(self) -> Self {
        match self {
            LibraryCategory::LikedSongs => LibraryCategory::Playlists,
            LibraryCategory::Playlists => LibraryCategory::Artists,
            LibraryCategory::Artists => LibraryCategory::Albums,
            LibraryCategory::Albums => LibraryCategory::LikedSongs,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            LibraryCategory::LikedSongs => LibraryCategory::Albums,
            LibraryCategory::Playlists => LibraryCategory::LikedSongs,
            LibraryCategory::Artists => LibraryCategory::Playlists,
            LibraryCategory::Albums => LibraryCategory::Artists,
        }
    }
}

#[derive(Default, PartialEq)]
pub enum InputRouting {
    #[default]
    Category,
    Content,
    Search,
}

impl_youtui_component!(LibraryBrowser);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserLibraryAction {
    SwitchToNextCategory,
    SwitchToPrevCategory,
    FocusContent,
    FocusCategory,
    ActivateSelected,
    DismissTracks,
    ReloadCategory,
}

impl Action for BrowserLibraryAction {
    fn context(&self) -> Cow<'_, str> {
        "Library Browser".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            BrowserLibraryAction::SwitchToNextCategory => "Next category",
            BrowserLibraryAction::SwitchToPrevCategory => "Previous category",
            BrowserLibraryAction::FocusContent => "Focus content panel",
            BrowserLibraryAction::FocusCategory => "Focus category list",
            BrowserLibraryAction::ActivateSelected => "Activate selected",
            BrowserLibraryAction::DismissTracks => "Go back from tracks",
            BrowserLibraryAction::ReloadCategory => "Reload category",
        }
        .into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LibraryEffect {
    SongsLoaded(Vec<ListSong>),
    PlaylistsLoaded(Vec<LibraryPlaylist>),
    PlaylistTracksLoaded(Vec<ListSong>),
    ArtistsLoaded(Vec<LibraryArtist>),
    AlbumsLoaded(Vec<SearchResultAlbum>),
    LoadError(String),
}

impl FrontendEffect<LibraryBrowser, crate::app::server::ArcServer, crate::app::TaskMetadata>
    for LibraryEffect
{
    fn apply(
        self,
        target: &mut LibraryBrowser,
    ) -> impl Into<ComponentEffect<LibraryBrowser>> {
        match self {
            LibraryEffect::SongsLoaded(songs) => {
                info!(count = %songs.len(), "Liked songs loaded");
                target.loading = false;
                target.error = None;
                target.songs_fetched = true;
                target.song_list.clear();
                target.song_list.push_song_list(songs);
                target.song_list.state = ListStatus::Loaded;
                target.cur_selected = 0;
                target.widget_state = Default::default();
                target.input_routing = InputRouting::Content;
            }
            LibraryEffect::PlaylistsLoaded(playlists) => {
                info!(count = %playlists.len(), "Library playlists loaded");
                target.loading = false;
                target.error = None;
                target.playlists_fetched = true;
                target.playlist_data = playlists;
                target.playlist_selected = 0;
                target.input_routing = InputRouting::Content;
                target.show_playlist_tracks = false;
            }
            LibraryEffect::PlaylistTracksLoaded(songs) => {
                info!(count = %songs.len(), "Playlist tracks loaded");
                target.loading = false;
                target.error = None;
                target.playlist_tracks = songs;
                target.playlist_tracks_selected = 0;
                target.show_playlist_tracks = true;
                target.input_routing = InputRouting::Content;
            }
            LibraryEffect::ArtistsLoaded(artists) => {
                info!(count = %artists.len(), "Library artists loaded");
                target.loading = false;
                target.error = None;
                target.artists_fetched = true;
                target.artist_data = artists;
                target.artist_selected = 0;
                target.input_routing = InputRouting::Content;
            }
            LibraryEffect::AlbumsLoaded(albums) => {
                info!(count = %albums.len(), "Library albums loaded");
                target.loading = false;
                target.error = None;
                target.albums_fetched = true;
                target.album_data = albums;
                target.album_selected = 0;
                target.input_routing = InputRouting::Content;
            }
            LibraryEffect::LoadError(msg) => {
                warn!(error = %msg, "Library category load failed");
                target.loading = false;
                target.error = Some(msg);
            }
        }
        AsyncTask::new_no_op()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibrarySongsOk;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibrarySongsErr;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryPlaylistsOk;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryPlaylistsErr;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryArtistsOk;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryArtistsErr;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryAlbumsOk;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryAlbumsErr;

impl_youtui_task_handler!(HandleLibrarySongsOk, Vec<TableListSong>, LibraryBrowser, |_, raw: Vec<TableListSong>| {
    let songs: Vec<ListSong> = raw.into_iter().map(|ts| {
        use crate::app::structures::ListSongID;
        ListSong {
            video_id: ts.video_id,
            track_no: None,
            plays: String::new(),
            title: ts.title,
            explicit: Some(ts.explicit),
            download_status: DownloadStatus::None,
            id: ListSongID(0),
            duration_string: ts.duration,
            actual_duration: None,
            start_offset: None,
            year: None,
            genres: Vec::new(),
            styles: Vec::new(),
            album_art: AlbumArtState::None,
            artists: MaybeRc::Owned(ts.artists.into_iter().map(|a| ListSongArtist {
                name: a.name,
                id: None,
            }).collect()),
            thumbnails: MaybeRc::Owned(ts.thumbnails),
            album: None,
            like_status: ts.like_status,
        }
    }).collect();
    LibraryEffect::SongsLoaded(songs)
});
impl_youtui_task_handler!(HandleLibrarySongsErr, anyhow::Error, LibraryBrowser, |_, err: anyhow::Error| {
    LibraryEffect::LoadError(err.to_string())
});
impl_youtui_task_handler!(HandleLibraryPlaylistsOk, Vec<LibraryPlaylist>, LibraryBrowser, |_, pl: Vec<LibraryPlaylist>| {
    LibraryEffect::PlaylistsLoaded(pl)
});
impl_youtui_task_handler!(HandleLibraryPlaylistsErr, anyhow::Error, LibraryBrowser, |_, err: anyhow::Error| {
    LibraryEffect::LoadError(err.to_string())
});
#[derive(Clone, Copy, Debug, PartialEq)]
struct HandleLibraryPlaylistTracksOk;
#[derive(Clone, Copy, Debug, PartialEq)]
struct HandleLibraryPlaylistTracksErr;
impl_youtui_task_handler!(HandleLibraryPlaylistTracksOk, Vec<PlaylistSong>, LibraryBrowser, |_, songs: Vec<PlaylistSong>| {
    use std::rc::Rc;
    use crate::app::structures::ListSongID;
    let list_songs: Vec<ListSong> = songs.into_iter().map(|s| {
        let artists = MaybeRc::Owned(s.artists.into_iter().map(|a| ListSongArtist { name: a.name, id: None }).collect());
        let album = Some(MaybeRc::Owned(ListSongAlbum {
            name: s.album.name.clone(),
            id: AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
        }));
        let year = s.year.or_else(|| {
            s.album.name.split('(').last().and_then(|s| s.get(..4))
                .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
                .map(|y| y.to_string())
        });
        ListSong {
            video_id: s.video_id,
            track_no: None,
            plays: String::new(),
            title: s.title,
            explicit: Some(s.explicit),
            download_status: DownloadStatus::None,
            id: ListSongID(0),
            duration_string: s.duration,
            actual_duration: None,
            start_offset: None,
            year: year.map(Rc::new),
            album_art: AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            artists,
            thumbnails: MaybeRc::Owned(s.thumbnails),
            album,
            like_status: s.like_status,
        }
    }).collect();
    LibraryEffect::PlaylistTracksLoaded(list_songs)
});
impl_youtui_task_handler!(HandleLibraryPlaylistTracksErr, anyhow::Error, LibraryBrowser, |_, err: anyhow::Error| {
    tracing::error!("Error loading playlist tracks: {}", err);
    LibraryEffect::LoadError(err.to_string())
});
impl_youtui_task_handler!(HandleLibraryArtistsOk, Vec<LibraryArtist>, LibraryBrowser, |_, a: Vec<LibraryArtist>| {
    LibraryEffect::ArtistsLoaded(a)
});
impl_youtui_task_handler!(HandleLibraryArtistsErr, anyhow::Error, LibraryBrowser, |_, err: anyhow::Error| {
    LibraryEffect::LoadError(err.to_string())
});
impl_youtui_task_handler!(HandleLibraryAlbumsOk, Vec<SearchResultAlbum>, LibraryBrowser, |_, a: Vec<SearchResultAlbum>| {
    LibraryEffect::AlbumsLoaded(a)
});
impl_youtui_task_handler!(HandleLibraryAlbumsErr, anyhow::Error, LibraryBrowser, |_, err: anyhow::Error| {
    LibraryEffect::LoadError(err.to_string())
});

pub struct LibraryBrowser {
    pub input_routing: InputRouting,
    pub category: LibraryCategory,
    // Liked Songs state
    pub song_list: BrowserSongsList,
    pub cur_selected: usize,
    pub widget_state: ScrollingTableState,
    pub sort: SortManager,
    pub filter: FilterManager,
    // Playlists state
    pub playlist_data: Vec<LibraryPlaylist>,
    pub playlist_selected: usize,
    pub playlist_tracks: Vec<ListSong>,
    pub show_playlist_tracks: bool,
    pub playlist_tracks_selected: usize,
    pub liked_playlists: HashSet<PlaylistID<'static>>,
    pub tracks_widget_state: ScrollingTableState,
    pub tracks_sort: SortManager,
    pub tracks_filter: FilterManager,
    pub tracks_visual_mode: bool,
    pub tracks_visual_start: usize,
    // Artists state
    pub artist_data: Vec<LibraryArtist>,
    pub artist_selected: usize,
    // Albums state
    pub album_data: Vec<SearchResultAlbum>,
    pub album_selected: usize,
    // Loading
    pub loading: bool,
    pub error: Option<String>,
    // Whether each category has been fetched
    pub songs_fetched: bool,
    pub playlists_fetched: bool,
    pub artists_fetched: bool,
    pub albums_fetched: bool,
    // Local search
    pub search_active: bool,
    pub search: SearchBlock,
    pub cur_playing_video_id: Option<ytmapi_rs::common::VideoID<'static>>,
    pub local_filter_text: String,
}

impl LibraryBrowser {
    pub fn new() -> Self {
        Self {
            input_routing: InputRouting::Category,
            category: LibraryCategory::LikedSongs,
            song_list: Default::default(),
            cur_selected: 0,
            widget_state: Default::default(),
            sort: SortManager::new(),
            filter: Default::default(),
            playlist_data: Default::default(),
            playlist_selected: 0,
            playlist_tracks: Vec::new(),
            show_playlist_tracks: false,
            playlist_tracks_selected: 0,
            liked_playlists: HashSet::new(),
            tracks_widget_state: Default::default(),
            tracks_sort: SortManager::new(),
            tracks_filter: Default::default(),
            tracks_visual_mode: false,
            tracks_visual_start: 0,
            artist_data: Default::default(),
            artist_selected: 0,
            album_data: Default::default(),
            album_selected: 0,
            loading: false,
            error: None,
            songs_fetched: false,
            playlists_fetched: false,
            artists_fetched: false,
            albums_fetched: false,
            search_active: false,
            search: SearchBlock::default(),
            cur_playing_video_id: None,
            local_filter_text: String::new(),
        }
    }

    pub fn fetch_current_category(&mut self) -> AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata> {
        if self.loading {
            return AsyncTask::new_no_op();
        }
        match self.category {
            LibraryCategory::LikedSongs => {
                if self.songs_fetched {
                    return AsyncTask::new_no_op();
                }
                self.loading = true;
                AsyncTask::new_future_try(
                    GetAllLibrarySongs,
                    HandleLibrarySongsOk,
                    HandleLibrarySongsErr,
                    None,
                )
                .map_frontend(|this: &mut Self| this)
            }
            LibraryCategory::Playlists => {
                if self.playlists_fetched {
                    return AsyncTask::new_no_op();
                }
                self.loading = true;
                AsyncTask::new_future_try(
                    GetAllLibraryPlaylists,
                    HandleLibraryPlaylistsOk,
                    HandleLibraryPlaylistsErr,
                    None,
                )
                .map_frontend(|this: &mut Self| this)
            }
            LibraryCategory::Artists => {
                if self.artists_fetched {
                    return AsyncTask::new_no_op();
                }
                self.loading = true;
                AsyncTask::new_future_try(
                    GetAllLibraryArtists,
                    HandleLibraryArtistsOk,
                    HandleLibraryArtistsErr,
                    None,
                )
                .map_frontend(|this: &mut Self| this)
            }
            LibraryCategory::Albums => {
                if self.albums_fetched {
                    return AsyncTask::new_no_op();
                }
                self.loading = true;
                AsyncTask::new_future_try(
                    GetAllLibraryAlbums,
                    HandleLibraryAlbumsOk,
                    HandleLibraryAlbumsErr,
                    None,
                )
                .map_frontend(|this: &mut Self| this)
            }
        }
    }

    pub fn switch_to_next_category(&mut self) -> AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata> {
        debug!("Library category: {} → {}", self.category.label(), self.category.next().label());
        self.category = self.category.next();
        self.input_routing = InputRouting::Category;
        self.fetch_current_category()
    }

    pub fn switch_to_prev_category(&mut self) -> AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata> {
        debug!("Library category: {} → {}", self.category.label(), self.category.prev().label());
        self.category = self.category.prev();
        self.input_routing = InputRouting::Category;
        self.fetch_current_category()
    }

    pub fn focus_content(&mut self) -> AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata> {
        debug!("Library focus: content panel");
        self.input_routing = InputRouting::Content;
        self.fetch_current_category()
    }

    pub fn focus_category(&mut self) {
        debug!("Library focus: category panel");
        self.input_routing = InputRouting::Category;
    }

    pub fn reload_category(&mut self) -> AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata> {
        info!(category = %self.category.label(), "Reloading library category");
        match self.category {
            LibraryCategory::LikedSongs => self.songs_fetched = false,
            LibraryCategory::Playlists => self.playlists_fetched = false,
            LibraryCategory::Artists => self.artists_fetched = false,
            LibraryCategory::Albums => self.albums_fetched = false,
        }
        self.song_list.clear();
        self.loading = false;
        self.fetch_current_category()
    }

    pub fn play_selected_song(&self) -> (AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>, Option<AppCallback>) {
        if self.category != LibraryCategory::LikedSongs {
            return (AsyncTask::new_no_op(), None);
        }
        let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
        let Some(song) = songs.get(self.cur_selected) else {
            return (AsyncTask::new_no_op(), None);
        };
        debug!(title = %song.title, "Library: play selected song");
        (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylistAndPlay(vec![song.clone()])))
    }

    pub fn play_all_songs(&self) -> (AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>, Option<AppCallback>) {
        if self.category != LibraryCategory::LikedSongs {
            return (AsyncTask::new_no_op(), None);
        }
        let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
        debug!(count = %songs.len(), "Library: play all songs");
        (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylistAndPlay(songs)))
    }

    #[allow(dead_code)]
    pub fn load_selected_playlist(&self) -> (AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>, Option<AppCallback>) {
        if self.category != LibraryCategory::Playlists {
            return (AsyncTask::new_no_op(), None);
        }
        let Some(pl) = self.playlist_data.get(self.playlist_selected) else {
            return (AsyncTask::new_no_op(), None);
        };
        debug!(playlist = %pl.title, "Library: loading playlist");
        (AsyncTask::new_no_op(), Some(AppCallback::LoadPlaylistFromPopup(pl.playlist_id.clone())))
    }

    pub fn fetch_playlist_tracks(&mut self) -> AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata> {
        let Some(pl) = self.playlist_data.get(self.playlist_selected).cloned() else {
            return AsyncTask::new_no_op();
        };
        debug!(playlist = %pl.title, "Library: fetching playlist tracks");
        self.loading = true;
        AsyncTask::new_future_try(
            GetPlaylistTracks(pl.playlist_id),
            HandleLibraryPlaylistTracksOk,
            HandleLibraryPlaylistTracksErr,
            None,
        ).map_frontend(|this: &mut Self| this)
    }

    pub fn view_selected_lyrics(&self) -> (AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>, Option<AppCallback>) {
        if self.category != LibraryCategory::LikedSongs {
            return (AsyncTask::new_no_op(), None);
        }
        let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
        let Some(song) = songs.get(self.cur_selected) else {
            return (AsyncTask::new_no_op(), None);
        };
        let artist = song.artists.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(", ");
        let title = song.title.clone();
        debug!(title = %title, artist = %artist, "Library: view lyrics");
        (AsyncTask::new_no_op(), Some(AppCallback::ViewLyrics { artist, title }))
    }

    pub fn handle_toggle_search(&mut self) {
        self.search_active = !self.search_active;
        if self.search_active {
            self.search = SearchBlock::default();
            self.input_routing = InputRouting::Search;
        } else {
            self.search.clear_text();
            self.input_routing = InputRouting::Content;
        }
    }

    pub fn handle_text_entry_action(&mut self, action: crate::app::ui::action::TextEntryAction) -> async_callback_manager::AsyncTask<Self, crate::app::server::ArcServer, crate::app::server::TaskMetadata> {
        if self.search_active {
            if action == crate::app::ui::action::TextEntryAction::Submit {
                let text = self.search.search_contents.get_text().to_string();
                self.search.clear_text();
                self.search_active = false;
                self.input_routing = InputRouting::Content;
                // Apply filter using the search text
                let lower = text.to_lowercase();
                if !lower.is_empty() && self.category == LibraryCategory::Playlists {
                    self.playlist_data.retain(|p| p.title.to_lowercase().contains(&lower));
                }
            }
        }
        AsyncTask::new_no_op()
    }

    pub fn text_editor_mode(&self) -> Option<String> {
        if self.search_active {
            Some(self.search.search_contents.mode_char().to_string())
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn search_text(&self) -> &str {
        self.search.search_contents.get_text()
    }
    #[allow(dead_code)]
    pub fn is_search_active(&self) -> bool {
        self.search_active
    }
}

// -- Tracks table view --
impl LibraryBrowser {
    pub fn tracks_subcolumns_of_vec() -> [ListSongDisplayableField; 6] {
        [
            ListSongDisplayableField::TrackNo,
            ListSongDisplayableField::Artists,
            ListSongDisplayableField::Album,
            ListSongDisplayableField::Song,
            ListSongDisplayableField::Duration,
            ListSongDisplayableField::Year,
        ]
    }

    pub fn get_tracks_filtered_list_iter(&self) -> impl Iterator<Item = &ListSong> {
        let filter_text = self.local_filter_text.clone();
        self.playlist_tracks.iter().filter(move |ls| {
            if filter_text.is_empty() {
                return true;
            }
            let title = ls.get_fields([ListSongDisplayableField::Song]).into_iter().next().unwrap_or_default();
            let album = ls.get_fields([ListSongDisplayableField::Album]).into_iter().next().unwrap_or_default();
            let artist = ls.get_fields([ListSongDisplayableField::Artists]).into_iter().next().unwrap_or_default();
            fuzzy_match(&filter_text, &title).is_some()
                || fuzzy_match(&filter_text, &album).is_some()
                || fuzzy_match(&filter_text, &artist).is_some()
        })
    }
}

// -- Tracks table traits --
impl TableView for LibraryBrowser {
    fn get_selected_item(&self) -> usize {
        self.playlist_tracks_selected
    }
    fn get_state(&self) -> &ScrollingTableState {
        &self.tracks_widget_state
    }
    fn get_mut_state(&mut self) -> &mut ScrollingTableState {
        &mut self.tracks_widget_state
    }
    fn get_layout(&self) -> &[BasicConstraint] {
        &[
            BasicConstraint::Length(6),
            BasicConstraint::Percentage(Percentage(25)),
            BasicConstraint::Percentage(Percentage(30)),
            BasicConstraint::Percentage(Percentage(30)),
            BasicConstraint::Length(8),
            BasicConstraint::Length(5),
        ]
    }
    fn get_highlighted_row(&self) -> Option<usize> {
        if self.tracks_visual_mode {
            Some(self.tracks_visual_start)
        } else {
            None
        }
    }
    fn get_visual_range(&self) -> Option<(usize, usize)> {
        if self.tracks_visual_mode {
            let start = self.tracks_visual_start.min(self.playlist_tracks_selected);
            let end = self.tracks_visual_start.max(self.playlist_tracks_selected);
            Some((start, end))
        } else {
            None
        }
    }
    fn get_items(&self) -> impl ExactSizeIterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        self.playlist_tracks
            .iter()
            .map(|ls| ls.get_fields(Self::tracks_subcolumns_of_vec()).into_iter())
    }
    fn get_headings(&self) -> impl Iterator<Item = &'static str> {
        ["#", "Artist", "Album", "Song", "Duration", "Year"].into_iter()
    }
}

impl AdvancedTableView for LibraryBrowser {
    fn get_filtered_count(&self) -> usize {
        self.get_tracks_filtered_list_iter().count()
    }
    fn get_sortable_columns(&self) -> &[usize] {
        &[0, 1, 2, 3]
    }
    fn push_sort_command(&mut self, sort_command: TableSortCommand) -> anyhow::Result<()> {
        if !self.get_sortable_columns().contains(&sort_command.column) {
            anyhow::bail!("Unable to sort column {}", sort_command.column);
        }
        let field = get_adjusted_list_column(sort_command.column, Self::tracks_subcolumns_of_vec())?;
        self.playlist_tracks.sort_by(|a, b| match sort_command.direction {
            crate::app::view::SortDirection::Asc => a
                .get_field(field)
                .partial_cmp(&b.get_field(field))
                .unwrap_or(std::cmp::Ordering::Equal),
            crate::app::view::SortDirection::Desc => b
                .get_field(field)
                .partial_cmp(&a.get_field(field))
                .unwrap_or(std::cmp::Ordering::Equal),
        });
        self.tracks_sort.sort_commands.retain(|cmd| cmd.column != sort_command.column);
        self.tracks_sort.sort_commands.push(sort_command);
        Ok(())
    }
    fn clear_sort_commands(&mut self) {
        self.tracks_sort.sort_commands.clear();
    }
    fn get_sort_commands(&self) -> &[TableSortCommand] {
        &self.tracks_sort.sort_commands
    }
    fn get_filtered_items(&self) -> impl Iterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        self.get_tracks_filtered_list_iter()
            .map(|ls| ls.get_fields(Self::tracks_subcolumns_of_vec()).into_iter())
    }
    fn get_filterable_columns(&self) -> &[usize] {
        &[1, 2, 3]
    }
    fn get_filter_commands(&self) -> &[TableFilterCommand] {
        &self.tracks_filter.filter_commands
    }
    fn clear_filter_commands(&mut self) {
        self.tracks_filter.filter_commands.clear();
    }
    fn get_sort_popup_cur(&self) -> usize {
        self.tracks_sort.cur
    }
    fn sort_popup_shown(&self) -> bool {
        self.tracks_sort.shown
    }
    fn filter_popup_shown(&self) -> bool {
        self.tracks_filter.shown
    }
    fn get_sort_state(&self) -> &ratatui::widgets::ListState {
        &self.tracks_sort.state
    }
    fn get_mut_sort_state(&mut self) -> &mut ratatui::widgets::ListState {
        &mut self.tracks_sort.state
    }
    fn get_mut_filter_state(&mut self) -> &mut vi_text_editor::ViTextEditor {
        &mut self.tracks_filter.filter_text
    }
}

// -- Scrollable --
impl Scrollable for LibraryBrowser {
    fn increment_list(&mut self, amount: isize) {
        match self.input_routing {
            InputRouting::Search => {} // no scrolling in search mode
            InputRouting::Category => {
                let idx = self.category as isize;
                let new_idx = (idx + amount).rem_euclid(LibraryCategory::ALL.len() as isize) as usize;
                let new_cat = LibraryCategory::ALL[new_idx];
                self.category = new_cat;
            }
            InputRouting::Content => match self.category {
                LibraryCategory::LikedSongs => {
                    let max = self.song_list.get_list_iter().count().saturating_sub(1);
                    self.cur_selected = self
                        .cur_selected
                        .saturating_add_signed(amount)
                        .min(max);
                }
                LibraryCategory::Playlists => {
                    if self.show_playlist_tracks {
                        let max = self.playlist_tracks.len().saturating_sub(1);
                        self.playlist_tracks_selected = self
                            .playlist_tracks_selected
                            .saturating_add_signed(amount)
                            .min(max);
                    } else {
                        let max = self.playlist_data.len().saturating_sub(1);
                        self.playlist_selected = self
                            .playlist_selected
                            .saturating_add_signed(amount)
                            .min(max);
                    }
                }
                LibraryCategory::Artists => {
                    let max = self.artist_data.len().saturating_sub(1);
                    self.artist_selected = self
                        .artist_selected
                        .saturating_add_signed(amount)
                        .min(max);
                }
                LibraryCategory::Albums => {
                    let max = self.album_data.len().saturating_sub(1);
                    self.album_selected = self
                        .album_selected
                        .saturating_add_signed(amount)
                        .min(max);
                }
            },
        }
    }

    fn is_scrollable(&self) -> bool {
        true
    }
}

impl ActionHandler<FilterAction> for LibraryBrowser {
    fn apply_action(&mut self, action: FilterAction) -> impl Into<YoutuiEffect<Self>> {
        match self.category {
            LibraryCategory::LikedSongs => match action {
                FilterAction::Close => self.filter.shown = false,
                FilterAction::ClearFilter => {
                    self.filter.shown = false;
                }
                FilterAction::Apply => {
                    self.filter.shown = false;
                }
            },
            LibraryCategory::Playlists => match action {
                FilterAction::Close | FilterAction::ClearFilter => {
                    self.filter.shown = false;
                }
                FilterAction::Apply => {
                    self.filter.shown = false;
                }
            },
            _ => warn!("Filter not supported for {:?} category", self.category),
        }
        ComponentEffect::new_no_op()
    }
}

impl ActionHandler<SortAction> for LibraryBrowser {
    fn apply_action(&mut self, action: SortAction) -> impl Into<YoutuiEffect<Self>> {
        match self.category {
            LibraryCategory::LikedSongs => match action {
                SortAction::Close => self.sort.shown = false,
                SortAction::ClearSort => {
                    self.sort.shown = false;
                }
                SortAction::SortSelectedAsc => {
                    self.sort.shown = false;
                }
                SortAction::SortSelectedDesc => {
                    self.sort.shown = false;
                }
            },
            LibraryCategory::Playlists => match action {
                SortAction::Close => self.sort.shown = false,
                SortAction::ClearSort => {
                    self.sort.shown = false;
                }
                SortAction::SortSelectedAsc => {
                    self.playlist_data.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
                    self.sort.shown = false;
                }
                SortAction::SortSelectedDesc => {
                    self.playlist_data.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase()));
                    self.sort.shown = false;
                }
            },
            _ => warn!("Sort not supported for {:?} category", self.category),
        }
        ComponentEffect::new_no_op()
    }
}

impl ActionHandler<BrowserSongsAction> for LibraryBrowser {
    fn apply_action(&mut self, action: BrowserSongsAction) -> impl Into<YoutuiEffect<Self>> {
        // If category panel is focused, redirect Enter to focus content (triggers fetch)
        if self.input_routing != InputRouting::Content && action == BrowserSongsAction::PlaySong {
            return (self.focus_content(), None);
        }
        match self.category {
            LibraryCategory::LikedSongs => match action {
                BrowserSongsAction::PlaySong => {
                    return self.play_selected_song();
                }
                BrowserSongsAction::PlaySongs => {
                    return self.play_all_songs();
                }
                BrowserSongsAction::ViewLyrics => {
                    return self.view_selected_lyrics();
                }
                BrowserSongsAction::CopySongUrl => {
                    let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
                    if let Some(song) = songs.get(self.cur_selected) {
                        let raw_url = format!("https://music.youtube.com/watch?v={}", song.video_id.get_raw());
                        let _ = std::process::Command::new("wl-copy").arg(&raw_url).spawn();
                        info!("Copied URL: {raw_url}");
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                BrowserSongsAction::AddSongToPlaylist => {
                    let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
                    if let Some(song) = songs.get(self.cur_selected) {
                        debug!(title = %song.title, "Library: add song to playlist");
                        return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylist(vec![song.clone()])));
                    }
                }
                BrowserSongsAction::AddSongsToPlaylist => {
                    let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
                    debug!(count = %songs.len(), "Library: add all songs to playlist");
                    return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylist(songs)));
                }
                BrowserSongsAction::Filter => {
                    self.filter.shown = !self.filter.shown;
                    return (AsyncTask::new_no_op(), None);
                }
                BrowserSongsAction::Sort => {
                    self.sort.shown = !self.sort.shown;
                    return (AsyncTask::new_no_op(), None);
                }
                BrowserSongsAction::GoToArtist => {
                    let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
                    if let Some(song) = songs.get(self.cur_selected) {
                        if let Some(cb) = super::shared_components::navigate_to_artist(song) {
                            return (AsyncTask::new_no_op(), Some(cb));
                        }
                    }
                }
                BrowserSongsAction::GoToAlbum => {
                    let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
                    if let Some(song) = songs.get(self.cur_selected) {
                        if let Some(cb) = super::shared_components::navigate_to_album(song) {
                            return (AsyncTask::new_no_op(), Some(cb));
                        }
                        warn!("Song has no album data, cannot navigate to album");
                    }
                }
                BrowserSongsAction::SaveToExistingPlaylist => {
                    let video_ids: Vec<_> = self.song_list.get_list_iter()
                        .map(|s| s.video_id.clone())
                        .collect();
                    if !video_ids.is_empty() {
                        return (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistUpdatePopup(video_ids)));
                    }
                }
                BrowserSongsAction::InsertNext => {
                    let songs: Vec<_> = self.song_list.get_list_iter().skip(self.cur_selected).cloned().collect();
                    if !songs.is_empty() {
                        return (AsyncTask::new_no_op(), Some(AppCallback::InsertNext(songs)));
                    }
                }
                BrowserSongsAction::GetRelatedTracks => {
                    let songs: Vec<_> = self.song_list.get_list_iter().cloned().collect();
                    if let Some(song) = songs.get(self.cur_selected) {
                        return (AsyncTask::new_no_op(), Some(AppCallback::GetRelatedTracks(song.video_id.clone())));
                    }
                }
                _ => warn!("Unsupported song action for liked songs: {:?}", action),
            },
            #[allow(unreachable_patterns)]
            LibraryCategory::Playlists => match action {
                BrowserSongsAction::PlaySong => {
                    if self.show_playlist_tracks {
                        // Playing a track from the tracks view
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylistAndPlay(vec![song.clone()])));
                        }
                    } else {
                        // Show tracks in-browser
                        return (self.fetch_playlist_tracks(), None);
                    }
                }
                BrowserSongsAction::PlaySongs => {
                    if self.show_playlist_tracks {
                        let songs: Vec<_> = self.playlist_tracks.clone();
                        return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylistAndPlay(songs)));
                    }
                    if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                        debug!(playlist = %pl.title, "Library: appending playlist to queue");
                        return (AsyncTask::new_no_op(), Some(AppCallback::AppendPlaylistFromPopup(pl.playlist_id.clone())));
                    }
                }
                BrowserSongsAction::CopySongUrl => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            let raw_url = format!("https://music.youtube.com/watch?v={}", song.video_id.get_raw());
                            let _ = std::process::Command::new("wl-copy").arg(&raw_url).spawn();
                            info!("Copied URL: {raw_url}");
                        }
                    } else if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                        let raw_url = format!("https://music.youtube.com/playlist?list={}", pl.playlist_id.get_raw().strip_prefix("VL").unwrap_or(pl.playlist_id.get_raw()));
                        let _ = std::process::Command::new("wl-copy").arg(&raw_url).spawn();
                        info!("Copied URL: {raw_url}");
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                BrowserSongsAction::ViewLyrics => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                            return (AsyncTask::new_no_op(), Some(AppCallback::ViewLyrics { artist, title: song.title.clone() }));
                        }
                    }
                }
                BrowserSongsAction::GoToArtist => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            if let Some(cb) = super::shared_components::navigate_to_artist(song) {
                                return (AsyncTask::new_no_op(), Some(cb));
                            }
                        }
                    }
                }
                BrowserSongsAction::GoToAlbum => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            if let Some(cb) = super::shared_components::navigate_to_album(song) {
                                return (AsyncTask::new_no_op(), Some(cb));
                            }
                        }
                    }
                }
                BrowserSongsAction::AddSongToPlaylist => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylist(vec![song.clone()])));
                        }
                    }
                }
                BrowserSongsAction::AddSongsToPlaylist => {
                    if self.show_playlist_tracks {
                        let songs: Vec<_> = self.playlist_tracks.clone();
                        return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylist(songs)));
                    }
                }
                BrowserSongsAction::SaveToExistingPlaylist => {
                    if self.show_playlist_tracks {
                        let video_ids: Vec<_> = self.playlist_tracks.iter()
                            .map(|s| s.video_id.clone())
                            .collect();
                        if !video_ids.is_empty() {
                            return (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistUpdatePopup(video_ids)));
                        }
                    }
                }
                BrowserSongsAction::InsertNext => {
                    if self.show_playlist_tracks {
                        let songs: Vec<_> = self.playlist_tracks.clone();
                        if !songs.is_empty() {
                            return (AsyncTask::new_no_op(), Some(AppCallback::InsertNext(songs)));
                        }
                    }
                }
                BrowserSongsAction::GetRelatedTracks => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::GetRelatedTracks(song.video_id.clone())));
                        }
                    }
                }
                BrowserSongsAction::Filter => {
                    if self.show_playlist_tracks {
                        self.filter.shown = !self.filter.shown;
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                BrowserSongsAction::Sort => {
                    if self.show_playlist_tracks {
                        self.sort.shown = !self.sort.shown;
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                BrowserSongsAction::DeletePlaylist => {
                    if !self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::ShowDeleteConfirm(pl.playlist_id.clone(), pl.title.clone())));
                        }
                    }
                }
                BrowserSongsAction::RenamePlaylist => {
                    if !self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::OpenRenamePopup(pl.playlist_id.clone(), pl.title.clone())));
                        }
                    }
                }
                BrowserSongsAction::EditPlaylistDetails => {
                    if !self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::OpenEditPopup(pl.playlist_id.clone(), pl.title.clone())));
                        }
                    }
                }
                BrowserSongsAction::RatePlaylist => {
                    if !self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let was_liked = !self.liked_playlists.insert(pl.playlist_id.clone());
                            let rating = if was_liked { LikeStatus::Indifferent } else { LikeStatus::Liked };
                            return (AsyncTask::new_no_op(), Some(AppCallback::RatePlaylistFromLibrary(
                                pl.playlist_id.clone(),
                                rating,
                            )));
                        }
                    }
                }
                BrowserSongsAction::GetPlaylistDetails => {
                    if !self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::OpenDetailsPopup(pl.playlist_id.clone(), pl.title.clone())));
                        }
                    }
                }
                BrowserSongsAction::RemoveTrackFromPlaylist => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                                return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(
                                    pl.playlist_id.clone(),
                                    vec![song.video_id.clone()],
                                )));
                            }
                        }
                    }
                }
                BrowserSongsAction::MoveTrackUp => {
                    if self.show_playlist_tracks && self.playlist_tracks_selected > 0 {
                        let cur = self.playlist_tracks_selected;
                        if let (Some(song), Some(above)) = (self.playlist_tracks.get(cur), self.playlist_tracks.get(cur - 1)) {
                            if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                                return (AsyncTask::new_no_op(), Some(AppCallback::ReorderPlaylistItemFromLibrary(
                                    pl.playlist_id.clone(),
                                    song.video_id.clone(),
                                    above.video_id.clone(),
                                )));
                            }
                        }
                    }
                }
                BrowserSongsAction::MoveTrackDown => {
                    if self.show_playlist_tracks && self.playlist_tracks_selected + 1 < self.playlist_tracks.len() {
                        let cur = self.playlist_tracks_selected;
                        if let (Some(song), Some(below)) = (self.playlist_tracks.get(cur), self.playlist_tracks.get(cur + 1)) {
                            if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                                return (AsyncTask::new_no_op(), Some(AppCallback::ReorderPlaylistItemFromLibrary(
                                    pl.playlist_id.clone(),
                                    song.video_id.clone(),
                                    below.video_id.clone(),
                                )));
                            }
                        }
                    }
                }
                BrowserSongsAction::MergePlaylist => {
                    if !self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistMergePopup(pl.playlist_id.clone())));
                        }
                    }
                }
                BrowserSongsAction::ToggleVisualMode => {
                    if self.show_playlist_tracks {
                        self.tracks_visual_mode = !self.tracks_visual_mode;
                        if self.tracks_visual_mode {
                            self.tracks_visual_start = self.playlist_tracks_selected;
                        }
                    }
                }
                BrowserSongsAction::DeleteSelected => {
                    if self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let ids: Vec<_> = if self.tracks_visual_mode {
                                let start = self.tracks_visual_start.min(self.playlist_tracks_selected);
                                let end = self.tracks_visual_start.max(self.playlist_tracks_selected);
                                self.playlist_tracks[start..=end].iter().map(|s| s.video_id.clone()).collect()
                            } else {
                                self.playlist_tracks.get(self.playlist_tracks_selected).map(|s| vec![s.video_id.clone()]).unwrap_or_default()
                            };
                            self.tracks_visual_mode = false;
                            return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(pl.playlist_id.clone(), ids)));
                        }
                    }
                }
                BrowserSongsAction::DeleteToTop => {
                    if self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let ids: Vec<_> = self.playlist_tracks[..self.playlist_tracks_selected].iter().map(|s| s.video_id.clone()).collect();
                            return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(pl.playlist_id.clone(), ids)));
                        }
                    }
                }
                BrowserSongsAction::DeleteToBottom => {
                    if self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let start = self.playlist_tracks_selected + 1;
                            if start < self.playlist_tracks.len() {
                                let ids: Vec<_> = self.playlist_tracks[start..].iter().map(|s| s.video_id.clone()).collect();
                                return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(pl.playlist_id.clone(), ids)));
                            }
                        }
                    }
                }
                _ => warn!("Unsupported song action for playlists: {:?}", action),
            },
            LibraryCategory::Artists => match action {
                BrowserSongsAction::PlaySong => {
                    if let Some(artist) = self.artist_data.get(self.artist_selected) {
                        debug!(name = %artist.artist, "Library: opening artist page");
                        return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::ArtistChannel(artist.channel_id.clone()))));
                    }
                }
                BrowserSongsAction::SubscribeToArtist => {
                    if let Some(artist) = self.artist_data.get(self.artist_selected) {
                        return (AsyncTask::new_no_op(), Some(AppCallback::SubscribeToArtistFromLibrary(artist.channel_id.clone())));
                    }
                }
                BrowserSongsAction::UnsubscribeFromArtist => {
                    if let Some(artist) = self.artist_data.get(self.artist_selected) {
                        return (AsyncTask::new_no_op(), Some(AppCallback::UnsubscribeFromArtistFromLibrary(vec![artist.channel_id.clone()])));
                    }
                }
                _ => warn!("Unsupported song action for artists: {:?}", action),
            },
            LibraryCategory::Albums => match action {
                BrowserSongsAction::PlaySong => {
                    if let Some(album) = self.album_data.get(self.album_selected) {
                        let query = format!("{} {}", album.artist, album.title);
                        return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::SongSearch(query))));
                    }
                }
                _ => warn!("Unsupported song action for albums: {:?}", action),
            },
        }
        (AsyncTask::new_no_op(), None)
    }
}

use crossterm::event::Event;
impl TextHandler for LibraryBrowser {
    fn is_text_handling(&self) -> bool {
        self.input_routing == InputRouting::Search
    }
    fn get_text(&self) -> Option<&str> {
        Some(self.search.search_contents.get_text())
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        self.search.search_contents.set_text(&text.into());
    }
    fn clear_text(&mut self) -> bool {
        self.search.clear_text();
        true
    }
    fn handle_text_event_impl(&mut self, event: &Event) -> Option<ComponentEffect<Self>> {
        self.search
            .handle_text_event_impl(event)
            .map(|effect| effect.map_frontend(|this: &mut Self| &mut this.search))
    }
}

impl ActionHandler<BrowserSearchAction> for LibraryBrowser {
    fn apply_action(&mut self, action: BrowserSearchAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            BrowserSearchAction::Close => {
                self.handle_toggle_search();
            }
            _ => warn!("Search suggestion navigation not supported in library"),
        }
        YoutuiEffect::new_no_op()
    }
}

impl ActionHandler<BrowserLibraryAction> for LibraryBrowser {
    fn apply_action(&mut self, action: BrowserLibraryAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            BrowserLibraryAction::SwitchToNextCategory => {
                return (self.switch_to_next_category(), None);
            }
            BrowserLibraryAction::SwitchToPrevCategory => {
                return (self.switch_to_prev_category(), None);
            }
            BrowserLibraryAction::FocusContent => {
                return (self.focus_content(), None);
            }
            BrowserLibraryAction::FocusCategory => {
                self.focus_category();
            }
            BrowserLibraryAction::ActivateSelected => {
                if self.input_routing != InputRouting::Content {
                    return (self.focus_content(), None);
                }
                match self.category {
                    LibraryCategory::LikedSongs => {
                        return self.play_selected_song();
                    }
                    LibraryCategory::Playlists => {
                        if self.show_playlist_tracks {
                            if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                                return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylistAndPlay(vec![song.clone()])));
                            }
                        } else {
                            return (self.fetch_playlist_tracks(), None);
                        }
                    }
                    _ => {}
                }
            }
            BrowserLibraryAction::DismissTracks => {
                self.show_playlist_tracks = false;
                return (AsyncTask::new_no_op(), None);
            }
            BrowserLibraryAction::ReloadCategory => {
                return (self.reload_category(), None);
            }
        }
        (AsyncTask::new_no_op(), None)
    }
}

impl KeyRouter<AppAction> for LibraryBrowser {
    fn get_all_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        [&config.keybinds.browser_library, &config.keybinds.filter, &config.keybinds.sort, &config.keybinds.list]
            .into_iter()
    }

    fn get_active_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        self.get_all_keybinds(config)
    }
}
