use super::songsearch::BrowserSongsAction;
use super::shared_components::{FilterAction, FilterManager, SortAction, SortManager};
use crate::app::AppCallback;
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, KeyRouter, Scrollable, YoutuiEffect,
};
use crate::app::server::{
    GetAllLibrarySongs, GetAllLibraryPlaylists, GetAllLibraryArtists, GetAllLibraryAlbums,
};
use crate::app::structures::{
    BrowserSongsList, ListSong, ListSongArtist, ListStatus, MaybeRc,
    DownloadStatus, AlbumArtState,
};
use crate::app::ui::action::AppAction;
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::widgets::ScrollingTableState;
use async_callback_manager::{AsyncTask, FrontendEffect};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use std::borrow::Cow;
use ytmapi_rs::common::YoutubeID;
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
            BrowserLibraryAction::ReloadCategory => "Reload category",
        }
        .into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LibraryEffect {
    SongsLoaded(Vec<ListSong>),
    PlaylistsLoaded(Vec<LibraryPlaylist>),
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
        self.loading = true;
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
}

// -- Scrollable --
impl Scrollable for LibraryBrowser {
    fn increment_list(&mut self, amount: isize) {
        match self.input_routing {
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
                    let max = self.playlist_data.len().saturating_sub(1);
                    self.playlist_selected = self
                        .playlist_selected
                        .saturating_add_signed(amount)
                        .min(max);
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
                _ => warn!("Unsupported song action for liked songs: {:?}", action),
            },
            LibraryCategory::Playlists => match action {
                BrowserSongsAction::PlaySong => {
                    return self.load_selected_playlist();
                }
                BrowserSongsAction::PlaySongs => {
                    // Append selected playlist to existing queue
                    if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                        debug!(playlist = %pl.title, "Library: appending playlist to queue");
                        return (AsyncTask::new_no_op(), Some(AppCallback::AppendPlaylistFromPopup(pl.playlist_id.clone())));
                    }
                }
                BrowserSongsAction::CopySongUrl => {
                    if let Some(pl) = self.playlist_data.get(self.playlist_selected) {
                        let raw_url = format!("https://music.youtube.com/playlist?list={}", pl.playlist_id.get_raw().strip_prefix("VL").unwrap_or(pl.playlist_id.get_raw()));
                        let _ = std::process::Command::new("wl-copy").arg(&raw_url).spawn();
                        info!("Copied URL: {raw_url}");
                    }
                    return (AsyncTask::new_no_op(), None);
                }
                _ => warn!("Unsupported song action for playlists: {:?}", action),
            },
            _ => warn!(
                "Received songs action {:?} but library category is {:?}",
                action, self.category
            ),
        }
        (AsyncTask::new_no_op(), None)
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
                        return self.load_selected_playlist();
                    }
                    _ => {}
                }
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
