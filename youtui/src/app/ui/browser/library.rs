use super::songsearch::BrowserSongsAction;
use super::shared_components::{BrowserSearchAction, FilterAction, FilterManager, SearchBlock, SortAction, SortManager};
use crate::app::{AppCallback, NavTarget};
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, KeyRouter, Scrollable, TextHandler, YoutuiEffect,
};
use crate::app::structures::{AlbumOrUploadAlbumID, ListSongAlbum};

use crate::app::server::{
    GetAllLibrarySongs, GetAllLibraryPlaylists, GetAllLibraryArtists, GetAllLibraryAlbums,
    GetPlaylistTracks, EnrichFromMetadataCache,
};
use crate::app::structures::{
    BrowserSongsList, ListSong, ListSongArtist, ListSongDisplayableField, ListStatus, MaybeRc,
    DownloadStatus, AlbumArtState, fuzzy_match, Percentage,
};
use crate::app::view::{
    AdvancedTableView, BasicConstraint, HasTitle, TableFilterCommand, TableSortCommand, TableView,
};
use crate::app::ui::browser::shared_components::get_adjusted_list_column;
use crate::app::ui::action::AppAction;
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::widgets::ScrollingTableState;
use async_callback_manager::{AsyncTask, FrontendEffect};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use ytmapi_rs::common::{PlaylistID, YoutubeID, LikeStatus, ArtistChannelID};
use ytmapi_rs::parse::PlaylistSong;
use ytmapi_rs::parse::{LibraryPlaylist, LibraryArtist, SearchResultAlbum, TableListSong};
use ytmapi_rs::query::library::GetLibrarySortOrder;

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
    CycleSortOrder,
}

impl Action for BrowserLibraryAction {
    fn context(&self) -> Cow<'_, str> {
        "Library Browser".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            BrowserLibraryAction::SwitchToNextCategory => "Next category",
            BrowserLibraryAction::SwitchToPrevCategory => "Previous category",
            BrowserLibraryAction::FocusContent => "Focus tracks panel",
            BrowserLibraryAction::FocusCategory => "Focus categories",
            BrowserLibraryAction::ActivateSelected => "Open selected",
            BrowserLibraryAction::DismissTracks => "Back from tracks",
            BrowserLibraryAction::ReloadCategory => "Refresh category",
            BrowserLibraryAction::CycleSortOrder => "Sort: A-Z / Z-A / Recent",
        }
        .into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LibraryEffect {
    SongsLoaded(Vec<ListSong>),
    SongsEnriched(Vec<(usize, Option<String>, Vec<String>, Vec<String>)>),
    PlaylistsLoaded(Vec<LibraryPlaylist>),

    ArtistsLoaded(Vec<LibraryArtist>),
    AlbumsLoaded(Vec<SearchResultAlbum>),
    RemoveItemsSuccess,
    RemoveItemsError(String),
    ReorderItemsSuccess,
    ReorderItemsError(String),
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
                // Build enrichment data BEFORE consuming songs
                let enrich_data: Vec<(usize, String, String, Option<String>)> = songs.iter().enumerate()
                    .filter_map(|(i, s)| {
                        let artist: String = s.artists.iter()
                            .map(|a| a.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        let album = s.album.as_ref().map(|a| a.as_ref().name.clone());
                        if artist.is_empty() { None } else { Some((i, artist, s.title.clone(), album)) }
                    })
                    .collect();
                let has_enrich = !enrich_data.is_empty();

                target.loading = false;
                target.error = None;
                target.songs_fetched = true;
                target.song_list.clear();
                target.song_list.push_song_list(songs);
                target.song_list.state = ListStatus::Loaded;
                target.cur_selected = 0;
                target.widget_state = Default::default();
                target.input_routing = InputRouting::Content;

                if has_enrich {
                    return AsyncTask::new_future_try(
                        EnrichFromMetadataCache(enrich_data),
                        HandleEnrichFromCacheOk,
                        HandleEnrichFromCacheErr,
                        None,
                    );
                }
            }
            LibraryEffect::SongsEnriched(results) => {
                let count = results.len();
                for (idx, year, genres, styles) in results {
                    let year_rc = year.map(Rc::new);
                    target.song_list.update_song_at(idx, year_rc, genres, styles);
                }
                info!(count = %count, "Library songs enriched from cache");
            }
            LibraryEffect::PlaylistsLoaded(playlists) => {
                info!(count = %playlists.len(), "Library playlists loaded");
                target.loading = false;
                target.error = None;
                target.playlists_fetched = true;
                target.playlist_data = playlists;
                target.playlist_selected = 0;
                target.input_routing = InputRouting::Content;
                // show_playlist_tracks preserved across refreshes
                // Only DismissTracks action closes it
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
            LibraryEffect::RemoveItemsSuccess => {
                info!("Library playlist items removed successfully");
                target.loading = false;
                target.tracks_visual_mode = false;
                target.playlists_fetched = false;
            }
            LibraryEffect::RemoveItemsError(msg) => {
                error!("Failed to remove playlist items: {}", msg);
                target.loading = false;
                target.error = Some(msg);
            }
            LibraryEffect::ReorderItemsSuccess => {
                info!("Library playlist items reordered successfully");
                target.loading = false;
            }
            LibraryEffect::ReorderItemsError(msg) => {
                error!("Failed to reorder playlist items: {}", msg);
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryRemoveItemsOk;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryRemoveItemsErr;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryReorderItemsOk;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleLibraryReorderItemsErr;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleEnrichFromCacheOk;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleEnrichFromCacheErr;

impl_youtui_task_handler!(HandleLibrarySongsOk, Vec<TableListSong>, LibraryBrowser, |_, raw: Vec<TableListSong>| {
    let songs: Vec<ListSong> = raw.into_iter().map(|ts| {
        use crate::app::structures::ListSongID;
        use crate::app::structures::ListSongArtist;
        use crate::app::structures::ArtistOrUploadArtistID;
        use ytmapi_rs::common::AlbumID;
        // YTM library song has no year field - extract from album name parenthetical
        let year = ts.album.name.split('(').last()
            .and_then(|s| s.get(..4))
            .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
            .map(|y| std::rc::Rc::new(y.to_string()));
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
            year,
            genres: Vec::new(),
            styles: Vec::new(),
            album_art: AlbumArtState::None,
            artists: MaybeRc::Owned(ts.artists.into_iter().map(|a| ListSongArtist {
                name: a.name,
                id: a.id.map(ArtistOrUploadArtistID::Artist),
            }).collect()),
            thumbnails: MaybeRc::Owned(ts.thumbnails),
            album: Some(MaybeRc::Owned(ListSongAlbum {
                name: ts.album.name,
                id: AlbumOrUploadAlbumID::Album(AlbumID::from_raw(ts.album.id.get_raw().to_string())),
            })),
            like_status: ts.like_status,
            is_album_upload: false,
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
    let mut set_id_map = HashMap::new();
    let list_songs: Vec<ListSong> = songs.into_iter().map(|s| {
        let vid = s.video_id.get_raw().to_string();
        // Always use the raw video_id as the setVideoId - the API returns a
        // separate setVideoId but removal also works with just the video_id
        set_id_map.insert(vid.clone(), vid);
        let artists = MaybeRc::Owned(s.artists.into_iter().map(|a| ListSongArtist { name: a.name, id: None }).collect());
        let album = Some(MaybeRc::Owned(ListSongAlbum {
            name: s.album.name.clone(),
            id: AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
        }));
        let year = s.album.name.split('(').last().and_then(|s| s.get(..4))
            .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
            .map(|y| y.to_string());
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
            is_album_upload: false,
        }
    }).collect();
    // The effect handler will populate track_set_ids from the songs
    // We chain it via the effect
    struct PopulateSetIds(Vec<ListSong>, HashMap<String, String>);
    impl FrontendEffect<LibraryBrowser, crate::app::server::ArcServer, crate::app::TaskMetadata> for PopulateSetIds {
        fn apply(self, target: &mut LibraryBrowser) -> impl Into<ComponentEffect<LibraryBrowser>> {
            target.playlist_tracks = self.0;
            target.playlist_tracks_selected = 0;
            target.show_playlist_tracks = true;
            target.input_routing = InputRouting::Content;
            target.track_set_ids = self.1;
            AsyncTask::new_no_op()
        }
    }
    PopulateSetIds(list_songs, set_id_map)
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
impl_youtui_task_handler!(HandleLibraryRemoveItemsOk, (), LibraryBrowser, |_, _: ()| {
    LibraryEffect::RemoveItemsSuccess
});
impl_youtui_task_handler!(HandleLibraryRemoveItemsErr, anyhow::Error, LibraryBrowser, |_, err: anyhow::Error| {
    LibraryEffect::RemoveItemsError(err.to_string())
});
impl_youtui_task_handler!(HandleLibraryReorderItemsOk, (), LibraryBrowser, |_, _: ()| {
    LibraryEffect::ReorderItemsSuccess
});
impl_youtui_task_handler!(HandleLibraryReorderItemsErr, anyhow::Error, LibraryBrowser, |_, err: anyhow::Error| {
    LibraryEffect::ReorderItemsError(err.to_string())
});
impl_youtui_task_handler!(HandleEnrichFromCacheOk, Vec<(usize, Option<String>, Vec<String>, Vec<String>)>, LibraryBrowser, |_, results: Vec<(usize, Option<String>, Vec<String>, Vec<String>)>| {
    LibraryEffect::SongsEnriched(results)
});
impl_youtui_task_handler!(HandleEnrichFromCacheErr, anyhow::Error, LibraryBrowser, |_, _: anyhow::Error| {
    info!("Cache enrichment failed (non-critical): no metadata will be shown in library");
    LibraryEffect::LoadError(String::new()) // silent, no UI error
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
    pub track_set_ids: HashMap<String, String>,
    pub tracks_widget_state: ScrollingTableState,
    pub tracks_sort: SortManager,
    pub tracks_filter: FilterManager,
    pub tracks_visual_mode: bool,
    pub tracks_visual_start: usize,
    // Playlists table state (category list, not tracks subview)
    pub playlists_widget_state: ScrollingTableState,
    pub playlists_sort: SortManager,
    pub playlists_filter: FilterManager,
    // Artists state
    pub artist_data: Vec<LibraryArtist>,
    pub artist_selected: usize,
    pub artists_widget_state: ScrollingTableState,
    pub artists_sort: SortManager,
    pub artists_filter: FilterManager,
    // Albums state
    pub album_data: Vec<SearchResultAlbum>,
    pub album_selected: usize,
    pub albums_widget_state: ScrollingTableState,
    pub albums_sort: SortManager,
    pub albums_filter: FilterManager,
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
    pub sort_order: GetLibrarySortOrder,
    pub subscribed_artists: HashSet<ArtistChannelID<'static>>,
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
            track_set_ids: HashMap::new(),
            tracks_widget_state: Default::default(),
            tracks_sort: SortManager::new(),
            tracks_filter: Default::default(),
            tracks_visual_mode: false,
            tracks_visual_start: 0,
            playlists_widget_state: Default::default(),
            playlists_sort: SortManager::new(),
            playlists_filter: Default::default(),
            artist_data: Default::default(),
            artist_selected: 0,
            artists_widget_state: Default::default(),
            artists_sort: SortManager::new(),
            artists_filter: Default::default(),
            album_data: Default::default(),
            album_selected: 0,
            albums_widget_state: Default::default(),
            albums_sort: SortManager::new(),
            albums_filter: Default::default(),
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
            sort_order: GetLibrarySortOrder::Default,
            subscribed_artists: HashSet::new(),
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
                let task = AsyncTask::new_future_try(
                    GetAllLibrarySongs { sort_order: self.sort_order.clone() },
                    HandleLibrarySongsOk,
                    HandleLibrarySongsErr,
                    None,
                )
                .map_frontend(|this: &mut Self| this);
                task
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
                    GetAllLibraryArtists { sort_order: self.sort_order.clone() },
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
                    GetAllLibraryAlbums { sort_order: self.sort_order.clone() },
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
                if !lower.is_empty() {
                    self.local_filter_text = text.clone();
                } else {
                    self.local_filter_text.clear();
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

}

// -- Table view helpers for all categories --
impl LibraryBrowser {
    pub fn liked_songs_subcolumns_of_vec() -> [ListSongDisplayableField; 7] {
        [
            ListSongDisplayableField::TrackNo,
            ListSongDisplayableField::Artists,
            ListSongDisplayableField::Album,
            ListSongDisplayableField::Song,
            ListSongDisplayableField::Duration,
            ListSongDisplayableField::Year,
            ListSongDisplayableField::LikeStatus,
        ]
    }

    pub fn tracks_subcolumns_of_vec() -> [ListSongDisplayableField; 7] {
        Self::liked_songs_subcolumns_of_vec()
    }

    /// Returns the currently active sort/filter manager for the active category.
    fn active_sort(&self) -> &SortManager {
        if self.show_playlist_tracks {
            &self.tracks_sort
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &self.sort,
                LibraryCategory::Playlists => &self.playlists_sort,
                LibraryCategory::Artists => &self.artists_sort,
                LibraryCategory::Albums => &self.albums_sort,
            }
        }
    }
    fn active_sort_mut(&mut self) -> &mut SortManager {
        if self.show_playlist_tracks {
            &mut self.tracks_sort
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &mut self.sort,
                LibraryCategory::Playlists => &mut self.playlists_sort,
                LibraryCategory::Artists => &mut self.artists_sort,
                LibraryCategory::Albums => &mut self.albums_sort,
            }
        }
    }
    fn active_filter(&self) -> &FilterManager {
        if self.show_playlist_tracks {
            &self.tracks_filter
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &self.filter,
                LibraryCategory::Playlists => &self.playlists_filter,
                LibraryCategory::Artists => &self.artists_filter,
                LibraryCategory::Albums => &self.albums_filter,
            }
        }
    }
    fn active_filter_mut(&mut self) -> &mut FilterManager {
        if self.show_playlist_tracks {
            &mut self.tracks_filter
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &mut self.filter,
                LibraryCategory::Playlists => &mut self.playlists_filter,
                LibraryCategory::Artists => &mut self.artists_filter,
                LibraryCategory::Albums => &mut self.albums_filter,
            }
        }
    }

    pub fn get_tracks_filtered_list_iter(&self) -> impl Iterator<Item = &ListSong> {
        let filter_text = &self.local_filter_text;
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

    fn get_liked_songs_filtered_iter(&self) -> impl Iterator<Item = &ListSong> {
        let ft = &self.local_filter_text;
        self.song_list.get_list_iter().filter(move |ls| {
            if ft.is_empty() { return true; }
            let title = ls.get_fields([ListSongDisplayableField::Song]).into_iter().next().unwrap_or_default();
            let album = ls.get_fields([ListSongDisplayableField::Album]).into_iter().next().unwrap_or_default();
            let artist = ls.get_fields([ListSongDisplayableField::Artists]).into_iter().next().unwrap_or_default();
            fuzzy_match(&ft, &title).is_some()
                || fuzzy_match(&ft, &album).is_some()
                || fuzzy_match(&ft, &artist).is_some()
        })
    }

    fn get_playlists_filtered_iter(&self) -> impl Iterator<Item = (usize, &LibraryPlaylist)> {
        let ft = self.local_filter_text.to_lowercase();
        self.playlist_data.iter().enumerate().filter(move |(_, pl)| {
            ft.is_empty() || pl.title.to_lowercase().contains(&ft)
        })
    }

    fn get_artists_filtered_iter(&self) -> impl Iterator<Item = (usize, &LibraryArtist)> {
        let ft = self.local_filter_text.to_lowercase();
        self.artist_data.iter().enumerate().filter(move |(_, a)| {
            ft.is_empty() || a.artist.to_lowercase().contains(&ft)
        })
    }

    fn get_albums_filtered_iter(&self) -> impl Iterator<Item = (usize, &SearchResultAlbum)> {
        let ft = self.local_filter_text.to_lowercase();
        self.album_data.iter().enumerate().filter(move |(_, a)| {
            ft.is_empty()
                || a.artist.to_lowercase().contains(&ft)
                || a.title.to_lowercase().contains(&ft)
        })
    }

    // -- Column builders (allocates per frame; acceptable for library dataset sizes) --

    fn build_liked_songs_columns(&self) -> Vec<Vec<Cow<'_, str>>> {
        let fields = Self::liked_songs_subcolumns_of_vec();
        self.song_list.get_list_iter()
            .enumerate()
            .map(|(i, ls)| {
                let mut row = ls.get_fields(fields).to_vec();
                // Replace TrackNo display with row index (liked songs have no track numbers)
                if let Some(col) = row.get_mut(0) {
                    *col = Cow::Owned((i + 1).to_string());
                }
                row
            })
            .collect()
    }

    fn build_playlist_columns(&self) -> Vec<Vec<Cow<'_, str>>> {
        self.playlist_data.iter().enumerate().map(|(i, pl)| {
            vec![
                Cow::<'_, str>::Owned((i + 1).to_string()),
                Cow::Borrowed(pl.title.as_str()),
                Cow::Borrowed(pl.tracks.as_str()),
                Cow::Borrowed(pl.author.as_str()),
            ]
        }).collect()
    }

    fn build_artist_columns(&self) -> Vec<Vec<Cow<'_, str>>> {
        self.artist_data.iter().enumerate().map(|(i, a)| {
            vec![
                Cow::<'_, str>::Owned((i + 1).to_string()),
                Cow::Borrowed(a.artist.as_str()),
                Cow::Borrowed(a.byline.as_str()),
            ]
        }).collect()
    }

    fn build_album_columns(&self) -> Vec<Vec<Cow<'_, str>>> {
        self.album_data.iter().enumerate().map(|(i, a)| {
            vec![
                Cow::<'_, str>::Owned((i + 1).to_string()),
                Cow::Borrowed(a.artist.as_str()),
                Cow::Borrowed(a.title.as_str()),
                Cow::Borrowed(a.year.as_str()),
                Cow::<'_, str>::Owned(format!("{:?}", a.album_type)),
            ]
        }).collect()
    }
}

// -- Unified TableView (dispatches on show_playlist_tracks + category) --
impl TableView for LibraryBrowser {
    fn get_selected_item(&self) -> usize {
        let (raw_idx, has_filter) = if self.show_playlist_tracks {
            let filtered = !self.local_filter_text.is_empty()
                || !self.tracks_filter.filter_commands.is_empty()
                || !self.tracks_sort.sort_commands.is_empty();
            (self.playlist_tracks_selected, filtered)
        } else {
            let filtered = !self.local_filter_text.is_empty()
                || !self.active_filter().filter_commands.is_empty()
                || !self.active_sort().sort_commands.is_empty();
            let raw = match self.category {
                LibraryCategory::LikedSongs => self.cur_selected,
                LibraryCategory::Playlists => self.playlist_selected,
                LibraryCategory::Artists => self.artist_selected,
                LibraryCategory::Albums => self.album_selected,
            };
            (raw, filtered)
        };
        if !has_filter { return raw_idx; }

        if self.show_playlist_tracks {
            let selected_vid = self.playlist_tracks.get(raw_idx).map(|s| s.video_id.clone());
            if let Some(ref vid) = selected_vid {
                self.get_tracks_filtered_list_iter()
                    .position(|s| s.video_id == *vid)
                    .unwrap_or(0)
            } else { 0 }
        } else {
            match self.category {
                LibraryCategory::LikedSongs => {
                    let selected_vid = self.song_list.get_list_iter().nth(raw_idx).map(|s| s.video_id.clone());
                    if let Some(ref vid) = selected_vid {
                        self.get_liked_songs_filtered_iter()
                            .position(|s| s.video_id == *vid)
                            .unwrap_or(0)
                    } else { 0 }
                }
                LibraryCategory::Playlists => {
                    self.get_playlists_filtered_iter()
                        .position(|(i, _)| i == raw_idx)
                        .unwrap_or(0)
                }
                LibraryCategory::Artists => {
                    self.get_artists_filtered_iter()
                        .position(|(i, _)| i == raw_idx)
                        .unwrap_or(0)
                }
                LibraryCategory::Albums => {
                    self.get_albums_filtered_iter()
                        .position(|(i, _)| i == raw_idx)
                        .unwrap_or(0)
                }
            }
        }
    }
    fn get_state(&self) -> &ScrollingTableState {
        if self.show_playlist_tracks {
            &self.tracks_widget_state
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &self.widget_state,
                LibraryCategory::Playlists => &self.playlists_widget_state,
                LibraryCategory::Artists => &self.artists_widget_state,
                LibraryCategory::Albums => &self.albums_widget_state,
            }
        }
    }
    fn get_mut_state(&mut self) -> &mut ScrollingTableState {
        if self.show_playlist_tracks {
            &mut self.tracks_widget_state
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &mut self.widget_state,
                LibraryCategory::Playlists => &mut self.playlists_widget_state,
                LibraryCategory::Artists => &mut self.artists_widget_state,
                LibraryCategory::Albums => &mut self.albums_widget_state,
            }
        }
    }
    fn get_layout(&self) -> &[BasicConstraint] {
        static TRACKS_LAYOUT: [BasicConstraint; 7] = [
            BasicConstraint::Length(6),
            BasicConstraint::Percentage(Percentage(25)),
            BasicConstraint::Percentage(Percentage(30)),
            BasicConstraint::Percentage(Percentage(30)),
            BasicConstraint::Length(8),
            BasicConstraint::Length(5),
            BasicConstraint::Length(4),
        ];
        static PLAYLISTS_LAYOUT: [BasicConstraint; 4] = [
            BasicConstraint::Length(4),
            BasicConstraint::Percentage(Percentage(40)),
            BasicConstraint::Percentage(Percentage(20)),
            BasicConstraint::Percentage(Percentage(30)),
        ];
        static ARTISTS_LAYOUT: [BasicConstraint; 3] = [
            BasicConstraint::Length(5),
            BasicConstraint::Percentage(Percentage(50)),
            BasicConstraint::Percentage(Percentage(40)),
        ];
        static ALBUMS_LAYOUT: [BasicConstraint; 5] = [
            BasicConstraint::Length(4),
            BasicConstraint::Percentage(Percentage(30)),
            BasicConstraint::Percentage(Percentage(30)),
            BasicConstraint::Length(6),
            BasicConstraint::Length(10),
        ];

        if self.show_playlist_tracks {
            &TRACKS_LAYOUT
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &TRACKS_LAYOUT,
                LibraryCategory::Playlists => &PLAYLISTS_LAYOUT,
                LibraryCategory::Artists => &ARTISTS_LAYOUT,
                LibraryCategory::Albums => &ALBUMS_LAYOUT,
            }
        }
    }
    fn get_highlighted_row(&self) -> Option<usize> {
        if self.tracks_visual_mode {
            Some(self.tracks_visual_start)
        } else if self.show_playlist_tracks {
            self.cur_playing_video_id.as_ref().and_then(|vid| {
                self.get_tracks_filtered_list_iter()
                    .position(|s| s.video_id == *vid)
            })
        } else if self.category == LibraryCategory::LikedSongs {
            self.cur_playing_video_id.as_ref().and_then(|vid| {
                self.get_liked_songs_filtered_iter()
                    .position(|s| s.video_id == *vid)
            })
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
        let cols: Vec<Vec<Cow<'_, str>>> = if self.show_playlist_tracks {
            self.playlist_tracks
                .iter()
                .map(|ls| ls.get_fields(Self::tracks_subcolumns_of_vec()).to_vec())
                .collect()
        } else {
            match self.category {
                LibraryCategory::LikedSongs => self.build_liked_songs_columns(),
                LibraryCategory::Playlists => self.build_playlist_columns(),
                LibraryCategory::Artists => self.build_artist_columns(),
                LibraryCategory::Albums => self.build_album_columns(),
            }
        };
        cols.into_iter().map(|v| v.into_iter())
    }
    fn get_headings(&self) -> impl Iterator<Item = &'static str> {
        let headings: &[&'static str] = if self.show_playlist_tracks {
            &["#", "Artist", "Album", "Song", "Duration", "Year", "Liked"]
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &["#", "Artist", "Album", "Song", "Duration", "Year", "Liked"],
                LibraryCategory::Playlists => &["#", "Title", "Tracks", "Author"],
                LibraryCategory::Artists => &["#", "Artist", "Byline"],
                LibraryCategory::Albums => &["#", "Artist", "Album", "Year", "Type"],
            }
        };
        headings.iter().copied()
    }
}

// -- Unified AdvancedTableView (dispatches on show_playlist_tracks + category) --
impl AdvancedTableView for LibraryBrowser {
    fn get_filtered_count(&self) -> usize {
        if self.show_playlist_tracks {
            self.get_tracks_filtered_list_iter().count()
        } else {
            match self.category {
                LibraryCategory::LikedSongs => self.get_liked_songs_filtered_iter().count(),
                LibraryCategory::Playlists => self.get_playlists_filtered_iter().count(),
                LibraryCategory::Artists => self.get_artists_filtered_iter().count(),
                LibraryCategory::Albums => self.get_albums_filtered_iter().count(),
            }
        }
    }
    fn get_sortable_columns(&self) -> &[usize] {
        if self.show_playlist_tracks {
            &[0, 1, 2, 3, 6]
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &[0, 1, 2, 3, 6],
                LibraryCategory::Playlists => &[1, 3],
                LibraryCategory::Artists => &[1],
                LibraryCategory::Albums => &[1, 2, 3],
            }
        }
    }
    fn push_sort_command(&mut self, sort_command: TableSortCommand) -> anyhow::Result<()> {
        if !self.get_sortable_columns().contains(&sort_command.column) {
            anyhow::bail!("Unable to sort column {}", sort_command.column);
        }
        let cmp = |asc: bool, a: &str, b: &str| -> std::cmp::Ordering {
            if asc { a.to_lowercase().cmp(&b.to_lowercase()) }
            else { b.to_lowercase().cmp(&a.to_lowercase()) }
        };

        if self.show_playlist_tracks {
            let field = get_adjusted_list_column(sort_command.column, Self::tracks_subcolumns_of_vec())?;
            let asc = sort_command.direction == crate::app::view::SortDirection::Asc;
            self.playlist_tracks.sort_by(|a, b| a.get_field(field).partial_cmp(&b.get_field(field)).unwrap_or(std::cmp::Ordering::Equal));
            _ = asc;
            self.tracks_sort.sort_commands.retain(|cmd| cmd.column != sort_command.column);
            self.tracks_sort.sort_commands.push(sort_command);
        } else {
            match self.category {
                LibraryCategory::LikedSongs => {
                    let field = get_adjusted_list_column(sort_command.column, Self::liked_songs_subcolumns_of_vec())?;
                    let asc = sort_command.direction == crate::app::view::SortDirection::Asc;
                    self.song_list.sort_list_by(|a, b| {
                        if asc { a.get_field(field).partial_cmp(&b.get_field(field)) }
                        else { b.get_field(field).partial_cmp(&a.get_field(field)) }
                        .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    self.sort.sort_commands.retain(|cmd| cmd.column != sort_command.column);
                    self.sort.sort_commands.push(sort_command);
                }
                LibraryCategory::Playlists => {
                    let asc = sort_command.direction == crate::app::view::SortDirection::Asc;
                    match sort_command.column {
                        1 => self.playlist_data.sort_by(|a, b| cmp(asc, &a.title, &b.title)),
                        3 => self.playlist_data.sort_by(|a, b| cmp(asc, &a.author, &b.author)),
                        _ => {},
                    }
                    self.playlists_sort.sort_commands.retain(|cmd| cmd.column != sort_command.column);
                    self.playlists_sort.sort_commands.push(sort_command);
                }
                LibraryCategory::Artists => {
                    let asc = sort_command.direction == crate::app::view::SortDirection::Asc;
                    self.artist_data.sort_by(|a, b| cmp(asc, &a.artist, &b.artist));
                    self.artists_sort.sort_commands.retain(|cmd| cmd.column != sort_command.column);
                    self.artists_sort.sort_commands.push(sort_command);
                }
                LibraryCategory::Albums => {
                    let asc = sort_command.direction == crate::app::view::SortDirection::Asc;
                    match sort_command.column {
                        1 => self.album_data.sort_by(|a, b| cmp(asc, &a.artist, &b.artist)),
                        2 => self.album_data.sort_by(|a, b| cmp(asc, &a.title, &b.title)),
                        3 => self.album_data.sort_by(|a, b| {
                            let ya = a.year.parse::<u16>().unwrap_or(0);
                            let yb = b.year.parse::<u16>().unwrap_or(0);
                            if asc { ya.cmp(&yb) } else { yb.cmp(&ya) }
                        }),
                        _ => {},
                    }
                    self.albums_sort.sort_commands.retain(|cmd| cmd.column != sort_command.column);
                    self.albums_sort.sort_commands.push(sort_command);
                }
            }
        }
        Ok(())
    }
    fn clear_sort_commands(&mut self) {
        self.active_sort_mut().sort_commands.clear();
    }
    fn get_sort_commands(&self) -> &[TableSortCommand] {
        &self.active_sort().sort_commands
    }
    fn get_filtered_items(&self) -> impl Iterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        let fields = Self::tracks_subcolumns_of_vec();
        let iter: Box<dyn Iterator<Item = Box<dyn Iterator<Item = Cow<'_, str>> + '_>> + '_> = if self.show_playlist_tracks {
            Box::new(self.get_tracks_filtered_list_iter().map(move |ls| {
                let v: Box<dyn Iterator<Item = Cow<'_, str>> + '_> =
                    Box::new(ls.get_fields(fields).into_iter());
                v
            }))
        } else {
            match self.category {
                LibraryCategory::LikedSongs => {
                    let fields = Self::liked_songs_subcolumns_of_vec();
                    Box::new(self.get_liked_songs_filtered_iter().map(move |ls| {
                        let v: Box<dyn Iterator<Item = Cow<'_, str>> + '_> =
                            Box::new(ls.get_fields(fields).into_iter());
                        v
                    }))
                }
                LibraryCategory::Playlists => Box::new(self.get_playlists_filtered_iter().map(|(i, pl)| {
                    let v: Box<dyn Iterator<Item = Cow<'_, str>> + '_> = Box::new(
                        [
                            Cow::Owned((i + 1).to_string()),
                            Cow::Borrowed(pl.title.as_str()),
                            Cow::Borrowed(pl.tracks.as_str()),
                            Cow::Borrowed(pl.author.as_str()),
                        ]
                        .into_iter(),
                    );
                    v
                })),
                LibraryCategory::Artists => Box::new(self.get_artists_filtered_iter().map(|(i, a)| {
                    let v: Box<dyn Iterator<Item = Cow<'_, str>> + '_> = Box::new(
                        [
                            Cow::Owned((i + 1).to_string()),
                            Cow::Borrowed(a.artist.as_str()),
                            Cow::Borrowed(a.byline.as_str()),
                        ]
                        .into_iter(),
                    );
                    v
                })),
                LibraryCategory::Albums => Box::new(self.get_albums_filtered_iter().map(|(i, a)| {
                    let v: Box<dyn Iterator<Item = Cow<'_, str>> + '_> = Box::new(
                        [
                            Cow::Owned((i + 1).to_string()),
                            Cow::Borrowed(a.artist.as_str()),
                            Cow::Borrowed(a.title.as_str()),
                            Cow::Borrowed(a.year.as_str()),
                            Cow::Owned(format!("{:?}", a.album_type)),
                        ]
                        .into_iter(),
                    );
                    v
                })),
            }
        };
        iter
    }
    fn get_filterable_columns(&self) -> &[usize] {
        if self.show_playlist_tracks {
            &[1, 2, 3]
        } else {
            match self.category {
                LibraryCategory::LikedSongs => &[1, 2, 3],
                LibraryCategory::Playlists => &[1, 3],
                LibraryCategory::Artists => &[1],
                LibraryCategory::Albums => &[1, 2],
            }
        }
    }
    fn get_filter_commands(&self) -> &[TableFilterCommand] {
        &self.active_filter().filter_commands
    }
    fn clear_filter_commands(&mut self) {
        self.active_filter_mut().filter_commands.clear();
    }
    fn get_sort_popup_cur(&self) -> usize {
        self.active_sort().cur
    }
    fn sort_popup_shown(&self) -> bool {
        self.active_sort().shown
    }
    fn filter_popup_shown(&self) -> bool {
        self.active_filter().shown
    }
    fn get_sort_state(&self) -> &ratatui::widgets::ListState {
        &self.active_sort().state
    }
    fn get_mut_sort_state(&mut self) -> &mut ratatui::widgets::ListState {
        &mut self.active_sort_mut().state
    }
    fn get_mut_filter_state(&mut self) -> &mut vi_text_editor::ViTextEditor {
        &mut self.active_filter_mut().filter_text
    }
}

impl HasTitle for LibraryBrowser {
    fn get_title(&self) -> Cow<'_, str> {
        if self.show_playlist_tracks {
            let total = self.playlist_tracks.len();
            let search_tag = if !self.local_filter_text.is_empty() {
                let count = self.get_tracks_filtered_list_iter().count();
                format!(" [SEARCH: {} ({}/{})]", self.local_filter_text, count, total)
            } else {
                String::new()
            };
            format!("Playlist Tracks - {} tracks{}", total, search_tag).into()
        } else {
            let sort_label = match self.sort_order {
                GetLibrarySortOrder::Default => "",
                GetLibrarySortOrder::NameAsc => " [A-Z]",
                GetLibrarySortOrder::NameDesc => " [Z-A]",
                GetLibrarySortOrder::RecentlySaved => " [Recent]",
            };
            let total = match self.category {
                LibraryCategory::LikedSongs => self.song_list.get_list_iter().count(),
                LibraryCategory::Playlists => self.playlist_data.len(),
                LibraryCategory::Artists => self.artist_data.len(),
                LibraryCategory::Albums => self.album_data.len(),
            };
            let filtered_count: usize = match self.category {
                LibraryCategory::LikedSongs => self.get_liked_songs_filtered_iter().count(),
                LibraryCategory::Playlists => self.get_playlists_filtered_iter().count(),
                LibraryCategory::Artists => self.get_artists_filtered_iter().count(),
                LibraryCategory::Albums => self.get_albums_filtered_iter().count(),
            };
            let search_tag = if !self.local_filter_text.is_empty() {
                format!(" [SEARCH: {} ({}/{})]", self.local_filter_text, filtered_count, total)
            } else {
                String::new()
            };
            format!("Library - {}{}{}", self.category.label(), sort_label, search_tag).into()
        }
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
                        let max = if !self.local_filter_text.is_empty() || !self.tracks_filter.filter_commands.is_empty() || !self.tracks_sort.sort_commands.is_empty() {
                            self.get_tracks_filtered_list_iter().count().saturating_sub(1)
                        } else {
                            self.playlist_tracks.len().saturating_sub(1)
                        };
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
    fn apply_action(&mut self, _action: FilterAction) -> impl Into<YoutuiEffect<Self>> {
        if self.show_playlist_tracks {
            self.tracks_filter.shown = !self.tracks_filter.shown;
        } else {
            match self.category {
                LibraryCategory::LikedSongs => self.filter.shown = !self.filter.shown,
                LibraryCategory::Playlists => self.playlists_filter.shown = !self.playlists_filter.shown,
                LibraryCategory::Artists => self.artists_filter.shown = !self.artists_filter.shown,
                LibraryCategory::Albums => self.albums_filter.shown = !self.albums_filter.shown,
            }
        }
        ComponentEffect::new_no_op()
    }
}

impl ActionHandler<SortAction> for LibraryBrowser {
    fn apply_action(&mut self, _action: SortAction) -> impl Into<YoutuiEffect<Self>> {
        if self.show_playlist_tracks {
            self.tracks_sort.shown = !self.tracks_sort.shown;
        } else {
            match self.category {
                LibraryCategory::LikedSongs => self.sort.shown = !self.sort.shown,
                LibraryCategory::Playlists => self.playlists_sort.shown = !self.playlists_sort.shown,
                LibraryCategory::Artists => self.artists_sort.shown = !self.artists_sort.shown,
                LibraryCategory::Albums => self.albums_sort.shown = !self.albums_sort.shown,
            }
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
                        crate::app::structures::copy_to_clipboard(&raw_url);
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
                BrowserSongsAction::QueueSong => {
                    if let Some(song) = self.song_list.get_list_iter().nth(self.cur_selected) {
                        return (AsyncTask::new_no_op(), Some(AppCallback::QueueSong(vec![song.clone()])));
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
                            crate::app::structures::copy_to_clipboard(&raw_url);
                            info!("Copied URL: {raw_url}");
                        }
                    } else if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                        let raw_url = format!("https://music.youtube.com/playlist?list={}", pl.playlist_id.get_raw().strip_prefix("VL").unwrap_or(pl.playlist_id.get_raw()));
                        crate::app::structures::copy_to_clipboard(&raw_url);
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
                BrowserSongsAction::QueueSong => {
                    if self.show_playlist_tracks {
                        if let Some(song) = self.playlist_tracks.get(self.playlist_tracks_selected) {
                            return (AsyncTask::new_no_op(), Some(AppCallback::QueueSong(vec![song.clone()])));
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
                BrowserSongsAction::OpenPlaylistEditor => {
                    if self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let tracks = self.playlist_tracks.clone();
                            return (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistEditor {
                                playlist_id: pl.playlist_id.clone(),
                                playlist_title: pl.title.clone(),
                                tracks,
                            }));
                        }
                    } else {
                        self.error = Some("Open a playlist first (Enter)".into());
                    }
                }
                BrowserSongsAction::RatePlaylist => {
                    if !self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            if self.liked_playlists.contains(&pl.playlist_id) {
                                self.liked_playlists.remove(&pl.playlist_id);
                                debug!("Library: toggle unlike playlist");
                                return (AsyncTask::new_no_op(), Some(AppCallback::RatePlaylistFromLibrary(
                                    pl.playlist_id.clone(),
                                    LikeStatus::Indifferent,
                                )));
                            } else {
                                self.liked_playlists.insert(pl.playlist_id.clone());
                                debug!("Library: toggle like playlist");
                                return (AsyncTask::new_no_op(), Some(AppCallback::RatePlaylistFromLibrary(
                                    pl.playlist_id.clone(),
                                    LikeStatus::Liked,
                                )));
                            }
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
                        // Use filtered list to find correct track by visual position
                        let filtered: Vec<&ListSong> = self.get_tracks_filtered_list_iter().collect();
                        if let Some(song) = filtered.get(self.playlist_tracks_selected) {
                            if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                                let raw = self.track_set_ids.get(song.video_id.get_raw())
                                    .cloned()
                                    .unwrap_or_else(|| song.video_id.get_raw().to_string());
                                let set_id = ytmapi_rs::common::SetVideoID::from_raw(raw);
                                // Remove from local list by video_id match
                                let vid = song.video_id.get_raw().to_string();
                                self.playlist_tracks.retain(|t| t.video_id.get_raw() != vid);
                                self.playlist_tracks_selected = self.playlist_tracks_selected
                                    .min(self.playlist_tracks.len().saturating_sub(1));
                                return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(
                                    pl.playlist_id.clone(),
                                    vec![set_id],
                                )));
                            }
                        }
                    }
                }
                BrowserSongsAction::MoveTrackUp => {
                    if self.show_playlist_tracks && self.playlist_tracks_selected > 0 {
                        let cur = self.playlist_tracks_selected;
                        let filtered: Vec<&ListSong> = self.get_tracks_filtered_list_iter().collect();
                        let song_vid = filtered.get(cur).map(|s| s.video_id.get_raw().to_string());
                        let above_vid = filtered.get(cur - 1).map(|s| s.video_id.get_raw().to_string());
                        if let (Some(ref sv), Some(ref av)) = (song_vid, above_vid) {
                            if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                                let cur_idx = self.playlist_tracks.iter().position(|t| t.video_id.get_raw() == sv);
                                let above_idx = self.playlist_tracks.iter().position(|t| t.video_id.get_raw() == av);
                                if let (Some(ci), Some(ai)) = (cur_idx, above_idx) {
                                    self.playlist_tracks.swap(ci, ai);
                                    self.playlist_tracks_selected = self.playlist_tracks_selected.saturating_sub(1);
                                }
                                return (AsyncTask::new_no_op(), Some(AppCallback::ReorderPlaylistItemFromLibrary(
                                    pl.playlist_id.clone(),
                                    ytmapi_rs::common::VideoID::from_raw(sv.clone()),
                                    ytmapi_rs::common::VideoID::from_raw(av.clone()),
                                )));
                            }
                        }
                    }
                }
                BrowserSongsAction::MoveTrackDown => {
                    if self.show_playlist_tracks {
                        let cur = self.playlist_tracks_selected;
                        let filtered: Vec<&ListSong> = self.get_tracks_filtered_list_iter().collect();
                        if cur + 1 < filtered.len() {
                            let song_vid = filtered.get(cur).map(|s| s.video_id.get_raw().to_string());
                            let below_vid = filtered.get(cur + 1).map(|s| s.video_id.get_raw().to_string());
                            if let (Some(ref sv), Some(ref bv)) = (song_vid, below_vid) {
                                if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                                    let cur_idx = self.playlist_tracks.iter().position(|t| t.video_id.get_raw() == sv);
                                    let below_idx = self.playlist_tracks.iter().position(|t| t.video_id.get_raw() == bv);
                                    if let (Some(ci), Some(bi)) = (cur_idx, below_idx) {
                                        self.playlist_tracks.swap(ci, bi);
                                        self.playlist_tracks_selected += 1;
                                    }
                                    return (AsyncTask::new_no_op(), Some(AppCallback::ReorderPlaylistItemFromLibrary(
                                        pl.playlist_id.clone(),
                                        ytmapi_rs::common::VideoID::from_raw(sv.clone()),
                                        ytmapi_rs::common::VideoID::from_raw(bv.clone()),
                                    )));
                                }
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
                        info!("Toggling visual mode: {} -> {}", self.tracks_visual_mode, !self.tracks_visual_mode);
                        self.tracks_visual_mode = !self.tracks_visual_mode;
                        if self.tracks_visual_mode {
                            self.tracks_visual_start = self.playlist_tracks_selected;
                        }
                    }
                }
                BrowserSongsAction::DeleteSelected => {
                    if self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let filtered: Vec<&ListSong> = self.get_tracks_filtered_list_iter().collect();
                            let (ids, to_remove): (Vec<_>, Vec<_>) = if self.tracks_visual_mode {
                                let start = self.tracks_visual_start.min(self.playlist_tracks_selected);
                                let end = self.tracks_visual_start.max(self.playlist_tracks_selected);
                                filtered[start..=end].iter().map(|s| {
                                    let raw = self.track_set_ids.get(s.video_id.get_raw())
                                        .cloned().unwrap_or_else(|| s.video_id.get_raw().to_string());
                                    (ytmapi_rs::common::SetVideoID::from_raw(raw), s.video_id.get_raw().to_string())
                                }).unzip()
                            } else {
                                filtered.get(self.playlist_tracks_selected).map(|s| {
                                    let raw = self.track_set_ids.get(s.video_id.get_raw())
                                        .cloned().unwrap_or_else(|| s.video_id.get_raw().to_string());
                                    (vec![ytmapi_rs::common::SetVideoID::from_raw(raw)], vec![s.video_id.get_raw().to_string()])
                                }).unwrap_or_default()
                            };
                            self.tracks_visual_mode = false;
                            // Remove from local list by video_id
                            for vid in &to_remove {
                                self.playlist_tracks.retain(|t| t.video_id.get_raw() != vid);
                            }
                            self.playlist_tracks_selected = self.playlist_tracks_selected
                                .min(self.playlist_tracks.len().saturating_sub(1));
                            if !ids.is_empty() {
                                return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(pl.playlist_id.clone(), ids)));
                            }
                        }
                    }
                }
                BrowserSongsAction::DeleteToTop => {
                    if self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let filtered: Vec<&ListSong> = self.get_tracks_filtered_list_iter().collect();
                            let (ids, to_remove): (Vec<_>, Vec<_>) = filtered[..self.playlist_tracks_selected].iter().map(|s| {
                                let raw = self.track_set_ids.get(s.video_id.get_raw())
                                    .cloned().unwrap_or_else(|| s.video_id.get_raw().to_string());
                                (ytmapi_rs::common::SetVideoID::from_raw(raw), s.video_id.get_raw().to_string())
                            }).unzip();
                            for vid in &to_remove {
                                self.playlist_tracks.retain(|t| t.video_id.get_raw() != vid);
                            }
                            self.playlist_tracks_selected = 0;
                            if !ids.is_empty() {
                                return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(pl.playlist_id.clone(), ids)));
                            }
                        }
                    }
                }
                BrowserSongsAction::DeleteToBottom => {
                    if self.show_playlist_tracks {
                        if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                            let filtered: Vec<&ListSong> = self.get_tracks_filtered_list_iter().collect();
                            let start = self.playlist_tracks_selected + 1;
                            if start < filtered.len() {
                                let (ids, to_remove): (Vec<_>, Vec<_>) = filtered[start..].iter().map(|s| {
                                    let raw = self.track_set_ids.get(s.video_id.get_raw())
                                        .cloned().unwrap_or_else(|| s.video_id.get_raw().to_string());
                                    (ytmapi_rs::common::SetVideoID::from_raw(raw), s.video_id.get_raw().to_string())
                                }).unzip();
                                for vid in &to_remove {
                                    self.playlist_tracks.retain(|t| t.video_id.get_raw() != vid);
                                }
                                if !ids.is_empty() {
                                    return (AsyncTask::new_no_op(), Some(AppCallback::RemovePlaylistItemsFromLibrary(pl.playlist_id.clone(), ids)));
                                }
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
                BrowserSongsAction::GoToArtist => {
                    if let Some(artist) = self.artist_data.get(self.artist_selected) {
                        debug!(name = %artist.artist, "Library: go to artist via menu");
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
                BrowserSongsAction::ToggleSubscribeArtist => {
                    if let Some(artist) = self.artist_data.get(self.artist_selected) {
                        let cid = artist.channel_id.clone();
                        if self.subscribed_artists.contains(&cid) {
                            debug!(name = %artist.artist, "Library: toggle unsubscribe artist");
                            self.subscribed_artists.remove(&cid);
                            return (AsyncTask::new_no_op(), Some(AppCallback::UnsubscribeFromArtistFromLibrary(vec![cid])));
                        } else {
                            debug!(name = %artist.artist, "Library: toggle subscribe artist");
                            self.subscribed_artists.insert(cid.clone());
                            return (AsyncTask::new_no_op(), Some(AppCallback::SubscribeToArtistFromLibrary(cid)));
                        }
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
                BrowserSongsAction::GoToArtist => {
                    if let Some(album) = self.album_data.get(self.album_selected) {
                        let artist = album.artist.clone();
                        return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::Artist(artist))));
                    }
                }
                BrowserSongsAction::GoToAlbum => {
                    if let Some(album) = self.album_data.get(self.album_selected) {
                        debug!(album = %album.title, artist = %album.artist, "Library albums: open album direct");
                        return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::AlbumOpen {
                            artist: album.artist.clone(),
                            album: album.title.clone(),
                            album_id: album.album_id.clone(),
                        })));
                    }
                }
                BrowserSongsAction::RatePlaylist => {
                    // Navigate to album search where RatePlaylist works (audio_playlist_id available)
                    if let Some(album) = self.album_data.get(self.album_selected) {
                        debug!(album = %album.title, "Library: rate album - navigating to album search");
                        return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::AlbumOpen {
                            artist: album.artist.clone(),
                            album: album.title.clone(),
                            album_id: album.album_id.clone(),
                        })));
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
                    LibraryCategory::Artists => {
                        if let Some(artist) = self.artist_data.get(self.artist_selected) {
                            debug!(name = %artist.artist, "Library: activate artist via Enter");
                            return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::ArtistChannel(artist.channel_id.clone()))));
                        }
                    }
                    LibraryCategory::Albums => {
                        if let Some(album) = self.album_data.get(self.album_selected) {
                            debug!(album = %album.title, artist = %album.artist, "Library: activate album via Enter (direct open)");
                            return (AsyncTask::new_no_op(), Some(AppCallback::Navigate(NavTarget::AlbumOpen {
                                artist: album.artist.clone(),
                                album: album.title.clone(),
                                album_id: album.album_id.clone(),
                            })));
                        }
                    }
                }
            }
            BrowserLibraryAction::DismissTracks => {
                self.show_playlist_tracks = false;
                self.tracks_visual_mode = false;
                self.tracks_visual_start = 0;
                return (AsyncTask::new_no_op(), None);
            }
            BrowserLibraryAction::ReloadCategory => {
                return (self.reload_category(), None);
            }
            BrowserLibraryAction::CycleSortOrder => {
                self.sort_order = match self.sort_order {
                    GetLibrarySortOrder::Default => GetLibrarySortOrder::NameAsc,
                    GetLibrarySortOrder::NameAsc => GetLibrarySortOrder::NameDesc,
                    GetLibrarySortOrder::NameDesc => GetLibrarySortOrder::RecentlySaved,
                    GetLibrarySortOrder::RecentlySaved => GetLibrarySortOrder::Default,
                };
                info!(sort = ?self.sort_order, "Library sort order changed");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_order_cycle() {
        let mut lib = LibraryBrowser::new();
        assert_eq!(lib.sort_order, GetLibrarySortOrder::Default);

        lib.apply_action(BrowserLibraryAction::CycleSortOrder);
        assert_eq!(lib.sort_order, GetLibrarySortOrder::NameAsc);

        lib.apply_action(BrowserLibraryAction::CycleSortOrder);
        assert_eq!(lib.sort_order, GetLibrarySortOrder::NameDesc);

        lib.apply_action(BrowserLibraryAction::CycleSortOrder);
        assert_eq!(lib.sort_order, GetLibrarySortOrder::RecentlySaved);

        lib.apply_action(BrowserLibraryAction::CycleSortOrder);
        assert_eq!(lib.sort_order, GetLibrarySortOrder::Default);
    }

    #[test]
    fn get_filtered_items_playlists_yields_lazy_iterator() {
        let mut lib = LibraryBrowser::new();
        lib.category = LibraryCategory::Playlists;
        lib.playlist_data = vec![
            LibraryPlaylist {
                playlist_id: PlaylistID::from_raw("PL1"),
                title: "My Favorites".into(),
                thumbnails: vec![],
                tracks: "42 songs".into(),
                author: "Author1".into(),
                author_id: None,
            },
            LibraryPlaylist {
                playlist_id: PlaylistID::from_raw("PL2"),
                title: "Chill Vibes".into(),
                thumbnails: vec![],
                tracks: "15 songs".into(),
                author: "Author2".into(),
                author_id: None,
            },
        ];

        let items: Vec<Vec<Cow<'_, str>>> = lib.get_filtered_items()
            .map(|row| row.collect())
            .collect();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0][1], "My Favorites");
        assert_eq!(items[0][2], "42 songs");
        assert_eq!(items[1][1], "Chill Vibes");
        assert_eq!(items[1][2], "15 songs");
    }

    #[test]
    fn get_filtered_items_artists_yields_lazy_iterator() {
        let mut lib = LibraryBrowser::new();
        lib.category = LibraryCategory::Artists;
        // Use serde_json to construct non-exhaustive struct from external crate
        let radiohead: LibraryArtist = serde_json::from_str(
            r#"{"channel_id":"CH1","artist":"Radiohead","byline":"12 songs"}"#
        ).unwrap();
        let nirvana: LibraryArtist = serde_json::from_str(
            r#"{"channel_id":"CH2","artist":"Nirvana","byline":"8 songs"}"#
        ).unwrap();
        lib.artist_data = vec![radiohead, nirvana];

        let items: Vec<Vec<Cow<'_, str>>> = lib.get_filtered_items()
            .map(|row| row.collect())
            .collect();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0][1], "Radiohead");
        assert_eq!(items[1][1], "Nirvana");
    }

    #[test]
    fn get_filtered_items_empty_default() {
        let lib = LibraryBrowser::new();
        assert_eq!(lib.playlist_data.len(), 0);
        // Default category is LikedSongs, which has no songs
        let items: Vec<Vec<Cow<'_, str>>> = lib.get_filtered_items()
            .map(|row| row.collect())
            .collect();
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn get_filtered_items_playlists_respects_local_filter() {
        let mut lib = LibraryBrowser::new();
        lib.category = LibraryCategory::Playlists;
        lib.playlist_data = vec![
            LibraryPlaylist {
                playlist_id: PlaylistID::from_raw("PL1"),
                title: "My Favorites".into(),
                thumbnails: vec![],
                tracks: "42 songs".into(),
                author: "Author1".into(),
                author_id: None,
            },
            LibraryPlaylist {
                playlist_id: PlaylistID::from_raw("PL2"),
                title: "Chill Vibes".into(),
                thumbnails: vec![],
                tracks: "15 songs".into(),
                author: "Author2".into(),
                author_id: None,
            },
        ];
        lib.local_filter_text = "chill".into();

        let items: Vec<Vec<Cow<'_, str>>> = lib.get_filtered_items()
            .map(|row| row.collect())
            .collect();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0][1], "Chill Vibes");
    }
}
