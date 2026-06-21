use crate::app::AppCallback;
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, KeyRouter, Scrollable, TextHandler, YoutuiEffect,
};
use crate::app::structures::{BrowserSongsList, ListSong};
use crate::app::ui::action::{AppAction, TextEntryAction};
use super::shared_components::{BrowserSearchAction, FilterAction, SortAction};
use super::songsearch::BrowserSongsAction;
use crate::config::Config;
use crate::config::keymap::Keymap;
use async_callback_manager::{AsyncTask, BackendTask};
use tracing::info;
use ytmapi_rs::parse::{SearchResultAlbum, AlbumSong, ParsedSongAlbum, ParsedSongArtist};
use ytmapi_rs::common::{AlbumID, Thumbnail, YoutubeID};

pub struct AlbumSearchBrowser {
    pub albums: Vec<SearchResultAlbum>,
    pub album_selected: usize,
    pub track_list: BrowserSongsList,
    pub track_selected: usize,
    pub show_tracks: bool,
    pub fetched: bool,
    pub album_year: String,
    pub album_artist: String,
}

impl_youtui_component!(AlbumSearchBrowser);

impl AlbumSearchBrowser {
    pub fn new() -> Self {
        Self {
            albums: Vec::new(),
            album_selected: 0,
            track_list: BrowserSongsList::default(),
            track_selected: 0,
            show_tracks: false,
            fetched: false,
            album_year: String::new(),
            album_artist: String::new(),
        }
    }

    pub fn fetch_albums(&mut self) -> (ComponentEffect<Self>, Option<AppCallback>) {
        self.fetched = true;
        let task = AsyncTask::new_future_try(
            crate::app::server::GetAllLibraryAlbums,
            HandleLibraryAlbumsOk,
            HandleLibraryAlbumsError,
            None,
        ).map_frontend(|this: &mut Self| {
            this.track_list.clear();
            this
        });
        (task, None)
    }

    pub fn get_selected_album(&self) -> Option<&SearchResultAlbum> {
        self.albums.get(self.album_selected)
    }

    pub fn play_selected_album(&mut self) -> (ComponentEffect<Self>, Option<AppCallback>) {
        use crate::app::server::api;
        use async_callback_manager::BackendTask;
        let Some(album) = self.get_selected_album().cloned() else { return (AsyncTask::new_no_op(), None); };
        let album_id = album.album_id.clone();
        let task = AsyncTask::new_future_try(
            FetchAlbumTracks { album_id },
            HandleFetchAlbumTracksOk,
            HandleFetchAlbumTracksError,
            None,
        ).map_frontend(|this: &mut Self| &mut *this);
        (task, None)
    }

    pub fn left(&mut self) {
        if self.show_tracks {
            self.show_tracks = false;
            self.track_list.clear();
        }
    }

    pub fn right(&mut self) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if !self.show_tracks {
            return self.play_selected_album();
        }
        (AsyncTask::new_no_op(), None)
    }

    pub fn is_text_handling(&self) -> bool { false }
    pub fn handle_toggle_search(&mut self) {}
    pub fn handle_text_entry_action(&mut self, _action: TextEntryAction) -> ComponentEffect<Self> { AsyncTask::new_no_op() }
    pub fn revert_routing(&mut self) {}
    pub fn text_editor_mode(&self) -> Option<String> { None }
    pub fn go_to_first(&mut self) { self.album_selected = 0; self.track_selected = 0; }
    pub fn go_to_last(&mut self) {
        if self.show_tracks { self.track_selected = self.track_list.get_list_iter().count().saturating_sub(1); }
        else { self.album_selected = self.albums.len().saturating_sub(1); }
    }
}

impl ActionHandler<BrowserSongsAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, action: BrowserSongsAction) -> impl Into<YoutuiEffect<Self>> {
        let cur = self.track_selected;
        match action {
            BrowserSongsAction::PlaySong => {
                if let Some(song) = self.track_list.get_list_iter().nth(cur) {
                    return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylistAndPlay(vec![song.clone()])));
                }
            }
            BrowserSongsAction::PlaySongs => {
                let songs: Vec<_> = self.track_list.get_list_iter().cloned().collect();
                if !songs.is_empty() {
                    return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylistAndPlay(songs)));
                }
            }
            BrowserSongsAction::AddSongToPlaylist => {
                if let Some(song) = self.track_list.get_list_iter().nth(cur) {
                    return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylist(vec![song.clone()])));
                }
            }
            BrowserSongsAction::AddSongsToPlaylist => {
                let songs: Vec<_> = self.track_list.get_list_iter().cloned().collect();
                if !songs.is_empty() {
                    return (AsyncTask::new_no_op(), Some(AppCallback::AddSongsToPlaylist(songs)));
                }
            }
            BrowserSongsAction::SaveToExistingPlaylist => {
                let video_ids: Vec<_> = self.track_list.get_list_iter().map(|s| s.video_id.clone()).collect();
                if !video_ids.is_empty() {
                    return (AsyncTask::new_no_op(), Some(AppCallback::OpenPlaylistUpdatePopup(video_ids)));
                }
            }
            BrowserSongsAction::InsertNext => {
                let songs: Vec<_> = self.track_list.get_list_iter().cloned().collect();
                if !songs.is_empty() {
                    return (AsyncTask::new_no_op(), Some(AppCallback::InsertNext(songs)));
                }
            }
            BrowserSongsAction::ViewLyrics => {
                if let Some(song) = self.track_list.get_list_iter().nth(cur) {
                    let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                    return (AsyncTask::new_no_op(), Some(AppCallback::ViewLyrics { artist, title: song.title.clone() }));
                }
            }
            BrowserSongsAction::CopySongUrl => {
                if let Some(song) = self.track_list.get_list_iter().nth(cur) {
                    let url = format!("https://music.youtube.com/watch?v={}", song.video_id.get_raw());
                    let _ = std::process::Command::new("wl-copy").arg(&url).spawn();
                    info!("Copied URL: {url}");
                }
                return (AsyncTask::new_no_op(), None);
            }
            BrowserSongsAction::GoToArtist => {
                if let Some(song) = self.track_list.get_list_iter().nth(cur) {
                    if let Some(cb) = super::shared_components::navigate_to_artist(song) {
                        return (AsyncTask::new_no_op(), Some(cb));
                    }
                }
            }
            BrowserSongsAction::GoToAlbum => {}
            _ => {}
        }
        (AsyncTask::new_no_op(), None)
    }
}

impl KeyRouter<AppAction> for AlbumSearchBrowser {
    fn get_all_keybinds<'a>(&self, config: &'a Config) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        [&config.keybinds.browser_songs, &config.keybinds.list].into_iter()
    }
    fn get_active_keybinds<'a>(&self, config: &'a Config) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        [&config.keybinds.browser_songs, &config.keybinds.list].into_iter()
    }
}

impl Scrollable for AlbumSearchBrowser {
    fn increment_list(&mut self, amount: isize) {
        let max = if self.show_tracks {
            self.track_list.get_list_iter().count().saturating_sub(1)
        } else {
            self.albums.len().saturating_sub(1)
        };
        if self.show_tracks {
            self.track_selected = self.track_selected.saturating_add_signed(amount).min(max);
        } else {
            self.album_selected = self.album_selected.saturating_add_signed(amount).min(max);
        }
    }
    fn is_scrollable(&self) -> bool { true }
}

impl TextHandler for AlbumSearchBrowser {
    fn get_text(&self) -> Option<&str> { None }
    fn clear_text(&mut self) -> bool { false }
    fn replace_text(&mut self, _text: impl Into<String>) {}
    fn is_text_handling(&self) -> bool { false }
    fn handle_text_event_impl(&mut self, _event: &crossterm::event::Event) -> Option<AsyncTask<Self, Self::Bkend, Self::Md>> { None }
}

impl ActionHandler<BrowserSearchAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, _action: BrowserSearchAction) -> impl Into<YoutuiEffect<Self>> {
        (AsyncTask::new_no_op(), None)
    }
}

impl ActionHandler<FilterAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, _action: FilterAction) -> impl Into<YoutuiEffect<Self>> {
        (AsyncTask::new_no_op(), None)
    }
}

impl ActionHandler<SortAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, _action: SortAction) -> impl Into<YoutuiEffect<Self>> {
        (AsyncTask::new_no_op(), None)
    }
}

// ---- Backend tasks ----
#[derive(Debug, PartialEq)]
pub struct FetchAlbumTracks {
    pub album_id: AlbumID<'static>,
}

impl BackendTask<crate::app::server::ArcServer> for FetchAlbumTracks {
    type Output = std::result::Result<AlbumFetchResult, anyhow::Error>;
    type MetadataType = crate::app::server::TaskMetadata;
    fn into_future(self, backend: &crate::app::server::ArcServer) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use crate::app::server::api;
            let api_guard = backend.api.get_api().await?;
            let query = ytmapi_rs::query::GetAlbumQuery::new(&self.album_id);
            let album = api::query_api_with_retry(&api_guard, query).await?;
            Ok(AlbumFetchResult {
                album_id: ytmapi_rs::common::AlbumID::from_raw(self.album_id.get_raw().to_owned()),
                title: album.title,
                year: album.year,
                artists: album.artists,
                thumbnails: album.thumbnails,
                tracks: album.tracks,
            })
        }
    }
}

pub struct AlbumFetchResult {
    pub album_id: AlbumID<'static>,
    pub title: String,
    pub year: String,
    pub artists: Vec<ParsedSongArtist>,
    pub thumbnails: Vec<Thumbnail>,
    pub tracks: Vec<AlbumSong>,
}

#[derive(Debug, PartialEq)]
pub struct HandleLibraryAlbumsOk;
#[derive(Debug, PartialEq)]
pub struct HandleLibraryAlbumsError;
#[derive(Debug, PartialEq)]
pub struct HandleFetchAlbumTracksOk;
#[derive(Debug, PartialEq)]
pub struct HandleFetchAlbumTracksError;

impl_youtui_task_handler!(HandleLibraryAlbumsOk, Vec<SearchResultAlbum>, AlbumSearchBrowser, |_, a: Vec<SearchResultAlbum>| {
    move |target: &mut AlbumSearchBrowser| {
        target.albums = a;
        target.album_selected = 0;
        AsyncTask::new_no_op()
    }
});

impl_youtui_task_handler!(HandleLibraryAlbumsError, anyhow::Error, AlbumSearchBrowser, |_, _err: anyhow::Error| {
    |_target: &mut AlbumSearchBrowser| AsyncTask::new_no_op()
});

impl_youtui_task_handler!(HandleFetchAlbumTracksOk, AlbumFetchResult, AlbumSearchBrowser, |_, result: AlbumFetchResult| {
    move |target: &mut AlbumSearchBrowser| {
        target.track_list.clear();
        target.track_selected = 0;
        target.show_tracks = true;
        target.album_year = result.year.clone();
        target.album_artist = result.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
        let album = ParsedSongAlbum { name: result.title, id: result.album_id };
        target.track_list.append_raw_album_songs(result.tracks, album, result.year, result.artists, result.thumbnails);
        AsyncTask::new_no_op()
    }
});

impl_youtui_task_handler!(HandleFetchAlbumTracksError, anyhow::Error, AlbumSearchBrowser, |_, _err: anyhow::Error| {
    |_target: &mut AlbumSearchBrowser| AsyncTask::new_no_op()
});