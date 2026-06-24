use crate::app::AppCallback;
use crate::app::component::actionhandler::{
    ActionHandler, ComponentEffect, KeyRouter, Scrollable, TextHandler, YoutuiEffect,
};
use crate::app::server::SearchAlbums;
use crate::app::component::actionhandler::Suggestable;
use crate::app::structures::{
    BrowserSongsList, ListSong, ListSongDisplayableField, ListStatus, Percentage, fuzzy_match,
};
use crate::app::ui::action::{AppAction, TextEntryAction};
use crate::app::view::{
    AdvancedTableView, BasicConstraint, HasTitle, Loadable, SortDirection,
    TableFilterCommand, TableSortCommand, TableView,
};
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::widgets::{ScrollingListState, ScrollingTableState};
use super::shared_components::{
    BrowserSearchAction, FilterAction, FilterManager, SearchBlock, SortAction, SortManager, get_adjusted_list_column,
};
use super::songsearch::BrowserSongsAction;
use anyhow::{Result, bail};
use async_callback_manager::{AsyncTask, BackendTask};
use lru::LruCache;
use std::borrow::Cow;
use std::num::NonZeroUsize;
use tracing::{info, warn};
use vi_text_editor::ViTextEditor;
use ytmapi_rs::parse::{SearchResultAlbum, AlbumSong, ParsedSongAlbum, ParsedSongArtist};
use ytmapi_rs::common::{AlbumID, PlaylistID, SearchSuggestion, Thumbnail, YoutubeID, LikeStatus};

#[derive(Default)]
pub enum InputRouting {
    List,
    #[default]
    Search,
    Filter,
    Sort,
}

pub struct AlbumSearchBrowser {
    pub albums: Vec<SearchResultAlbum>,
    pub album_selected: usize,
    pub track_list: BrowserSongsList,
    pub track_selected: usize,
    pub show_tracks: bool,
    pub fetched: bool,
    pub album_year: String,
    pub album_artist: String,
    pub album_playlist_id: Option<PlaylistID<'static>>,
    pub album_artists: Vec<ParsedSongArtist>,
    pub input_routing: InputRouting,
    pub widget_state: ScrollingTableState,
    pub sort: SortManager,
    pub filter: FilterManager,
    pub search: SearchBlock,
    pub search_popped: bool,
    pub album_list_state: ScrollingListState,
    search_cache: LruCache<String, Vec<SearchResultAlbum>>,
    last_search_query: Option<String>,
    pub local_filter_text: String,
    pub cur_playing_video_id: Option<ytmapi_rs::common::VideoID<'static>>,
}

impl_youtui_component!(AlbumSearchBrowser);

#[allow(dead_code)]
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
            album_playlist_id: None,
            album_artists: Vec::new(),
            sort: SortManager::default(),
            filter: FilterManager::default(),
            search: SearchBlock::default(),
            search_popped: false,
            input_routing: InputRouting::List,
            widget_state: ScrollingTableState::default(),
            album_list_state: ScrollingListState::default(),
            search_cache: LruCache::new(NonZeroUsize::new(50).unwrap()),
            last_search_query: None,
            local_filter_text: String::new(),
            cur_playing_video_id: None,
        }
    }

    pub fn fetch_albums(&mut self) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if self.fetched {
            return (AsyncTask::new_no_op(), None);
        }
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
    pub fn handle_toggle_search(&mut self) {
        if self.search_popped {
            self.search_popped = false;
            self.search = SearchBlock::default();
        } else {
            self.search_popped = true;
            self.search = SearchBlock::default();
        }
    }

    pub fn handle_text_entry_action(&mut self, action: TextEntryAction) -> ComponentEffect<Self> {
        if self.search_popped {
            match action {
                TextEntryAction::Submit => {
                    let query = self.search.search_contents.get_text().to_string();
                    self.search_popped = false;
                    self.search = SearchBlock::default();
                    if !query.is_empty() {
                        return self.search_albums_query(query).0;
                    }
                }
                TextEntryAction::DeleteWord => {
                    self.search.delete_word();
                }
                _ => {}
            }
        }
        AsyncTask::new_no_op()
    }

    pub fn has_search_suggestions(&self) -> bool {
        self.search.has_search_suggestions()
    }

    #[allow(dead_code)]
    pub fn get_search_suggestions(&self) -> &[SearchSuggestion] {
        self.search.get_search_suggestions()
    }

    pub fn search_albums_query(&mut self, query: String) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if let Some(cached) = self.search_cache.get(&query) {
            self.albums = cached.clone();
            self.album_selected = 0;
            self.show_tracks = false;
            return (AsyncTask::new_no_op(), None);
        }
        self.last_search_query = Some(query.clone());
        let task = AsyncTask::new_future_try(
            SearchAlbums(query),
            HandleSearchAlbumsOk,
            HandleSearchAlbumsError,
            None,
        ).map_frontend(|this: &mut Self| &mut *this);
        (task, None)
    }
    #[allow(dead_code)]
    pub fn revert_routing(&mut self) {}
    pub fn text_editor_mode(&self) -> Option<String> { None }
    pub fn go_to_first(&mut self) { self.album_selected = 0; self.track_selected = 0; }
    pub fn go_to_last(&mut self) {
        if self.show_tracks { self.track_selected = self.track_list.get_list_iter().count().saturating_sub(1); }
        else { self.album_selected = self.albums.len().saturating_sub(1); }
    }
    fn subcolumns_of_vec() -> [ListSongDisplayableField; 4] {
        [
            ListSongDisplayableField::TrackNo,
            ListSongDisplayableField::Song,
            ListSongDisplayableField::Duration,
            ListSongDisplayableField::Year,
        ]
    }
    pub fn get_filtered_list_iter(&self) -> impl Iterator<Item = &ListSong> + '_ {
        let filter_text = self.local_filter_text.clone();
        self.track_list.get_list_iter().filter(move |ls| {
            let fuzzy_pass = if filter_text.is_empty() {
                true
            } else {
                let title = ls.get_fields([ListSongDisplayableField::Song]).into_iter().next().unwrap_or_default();
                let album = ls.get_fields([ListSongDisplayableField::Album]).into_iter().next().unwrap_or_default();
                let artist = ls.get_fields([ListSongDisplayableField::Artists]).into_iter().next().unwrap_or_default();
                fuzzy_match(&filter_text, &title).is_some()
                    || fuzzy_match(&filter_text, &album).is_some()
                    || fuzzy_match(&filter_text, &artist).is_some()
            };
            if !fuzzy_pass { return false; }
            self.get_filter_commands()
                .iter()
                .fold(true, |acc, command| {
                    let match_found = command.matches_row(
                        ls,
                        Self::subcolumns_of_vec(),
                        self.get_filterable_columns(),
                    );
                    acc && match_found
                })
        })
    }
    #[allow(dead_code)]
    pub fn apply_all_sort_commands(&mut self) -> Result<()> {
        for c in self.sort.sort_commands.iter() {
            if !self.get_sortable_columns().contains(&c.column) {
                bail!(format!("Unable to sort column {}", c.column));
            }
            self.track_list.sort(
                get_adjusted_list_column(c.column, Self::subcolumns_of_vec())?,
                c.direction,
            );
        }
        Ok(())
    }
    #[allow(dead_code)]
    pub fn apply_filter(&mut self) {
        self.filter.shown = false;
        self.input_routing = InputRouting::List;
        let Some(filter) = self.filter.get_text().map(|s| s.to_string()) else {
            return;
        };
        let cmd = TableFilterCommand::All(crate::app::view::Filter::Contains(
            crate::app::view::FilterString::case_insensitive(filter),
        ));
        self.filter.filter_commands.push(cmd);
        let new_max_cur = self.get_filtered_list_iter().count().saturating_sub(1);
        self.track_selected = self.track_selected.min(new_max_cur);
    }
    #[allow(dead_code)]
    pub fn clear_filter(&mut self) {
        self.filter.shown = false;
        self.input_routing = InputRouting::List;
        self.clear_filter_commands();
    }
    pub fn toggle_filter(&mut self) {
        if !self.filter.shown {
            self.filter.filter_text.clear();
            self.input_routing = InputRouting::Filter;
        } else {
            self.clear_filter_commands();
            self.input_routing = InputRouting::List;
        }
        self.filter.shown = !self.filter.shown;
    }
    pub fn close_sort(&mut self) {
        self.sort.shown = false;
        self.input_routing = InputRouting::List;
    }
    pub fn handle_pop_sort(&mut self) {
        self.sort.cur = 0;
        self.sort.shown = true;
        self.input_routing = InputRouting::Sort;
    }
    pub fn handle_sort_cur_asc(&mut self) {
        let Some(column) = self.sortable_columns().get(self.sort.cur) else {
            warn!("Tried to index sortable columns but was out of range");
            return;
        };
        if let Err(e) = self.push_sort_command(TableSortCommand {
            column: *column,
            direction: SortDirection::Asc,
        }) {
            warn!("Tried to sort a column that is not sortable - error {e}")
        };
        self.close_sort();
    }
    pub fn handle_sort_cur_desc(&mut self) {
        let Some(column) = self.sortable_columns().get(self.sort.cur) else {
            warn!("Tried to index sortable columns but was out of range");
            return;
        };
        if let Err(e) = self.push_sort_command(TableSortCommand {
            column: *column,
            direction: SortDirection::Desc,
        }) {
            warn!("Tried to sort a column that is not sortable - error {e}")
        };
        self.close_sort();
    }
    fn sortable_columns(&self) -> &[usize] { &[1, 3] }
}

impl Loadable for AlbumSearchBrowser {
    fn is_loading(&self) -> bool {
        matches!(self.track_list.state, ListStatus::Loading)
    }
}
impl HasTitle for AlbumSearchBrowser {
    fn get_title(&self) -> Cow<'_, str> {
        if self.show_tracks {
            let album = self.albums.get(self.album_selected);
            let name = album.map_or("", |a| a.title.as_str());
            let count = self.track_list.get_list_iter().count();
            format!(" {} - {} ({} tracks) ", self.album_artist, name, count).into()
        } else {
            " Album Tracks ".into()
        }
    }
}
impl TableView for AlbumSearchBrowser {
    fn get_selected_item(&self) -> usize {
        self.track_selected
    }
    fn get_state(&self) -> &ScrollingTableState {
        &self.widget_state
    }
    fn get_mut_state(&mut self) -> &mut ScrollingTableState {
        &mut self.widget_state
    }
    fn get_layout(&self) -> &[BasicConstraint] {
        &[
            BasicConstraint::Length(6),
            BasicConstraint::Percentage(Percentage(75)),
            BasicConstraint::Length(8),
            BasicConstraint::Length(5),
        ]
    }
    fn get_highlighted_row(&self) -> Option<usize> {
        self.cur_playing_video_id.as_ref().and_then(|vid| {
            self.track_list.get_list_iter().position(|s| s.video_id == *vid)
        })
    }
    fn get_items(&self) -> impl ExactSizeIterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        self.track_list
            .get_list_iter()
            .map(|ls| ls.get_fields(Self::subcolumns_of_vec()).into_iter())
    }
    fn get_headings(&self) -> impl Iterator<Item = &'static str> {
        ["#", "Song", "Duration", "Year"].into_iter()
    }
}
impl AdvancedTableView for AlbumSearchBrowser {
    fn get_filtered_count(&self) -> usize {
        self.get_filtered_list_iter().count()
    }
    fn get_sortable_columns(&self) -> &[usize] {
        &[1, 3]
    }
    fn get_sort_commands(&self) -> &[TableSortCommand] {
        &self.sort.sort_commands
    }
    fn push_sort_command(&mut self, sort_command: TableSortCommand) -> Result<()> {
        if !self.get_sortable_columns().contains(&sort_command.column) {
            bail!(format!("Unable to sort column {}", sort_command.column));
        }
        self.track_list.sort(
            get_adjusted_list_column(sort_command.column, Self::subcolumns_of_vec())?,
            sort_command.direction,
        );
        self.sort.sort_commands.retain(|cmd| cmd.column != sort_command.column);
        self.sort.sort_commands.push(sort_command);
        Ok(())
    }
    fn clear_sort_commands(&mut self) {
        self.sort.sort_commands.clear();
    }
    fn get_filter_commands(&self) -> &[TableFilterCommand] {
        &self.filter.filter_commands
    }
    fn clear_filter_commands(&mut self) {
        self.filter.filter_commands.clear()
    }
    fn get_filterable_columns(&self) -> &[usize] {
        &[1]
    }
    fn get_sort_popup_cur(&self) -> usize {
        self.sort.cur
    }
    fn get_filtered_items(&self) -> impl Iterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        self.get_filtered_list_iter()
            .map(|ls| ls.get_fields(Self::subcolumns_of_vec()).into_iter())
    }
    fn sort_popup_shown(&self) -> bool {
        self.sort.shown
    }
    fn filter_popup_shown(&self) -> bool {
        self.filter.shown
    }
    fn get_sort_state(&self) -> &ratatui::widgets::ListState {
        &self.sort.state
    }
    fn get_mut_sort_state(&mut self) -> &mut ratatui::widgets::ListState {
        &mut self.sort.state
    }
    fn get_mut_filter_state(&mut self) -> &mut ViTextEditor {
        &mut self.filter.filter_text
    }
}

impl ActionHandler<BrowserSongsAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, action: BrowserSongsAction) -> impl Into<YoutuiEffect<Self>> {
        let cur = self.track_selected;
        match action {
            BrowserSongsAction::PlaySong => {
                if !self.show_tracks {
                    return self.play_selected_album().into();
                }
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
            BrowserSongsAction::GoToAlbum => {
                if let Some(song) = self.track_list.get_list_iter().nth(cur) {
                    if let Some(cb) = super::shared_components::navigate_to_album(song) {
                        return (AsyncTask::new_no_op(), Some(cb));
                    }
                    warn!("Song has no album data, cannot navigate to album");
                }
            }
            BrowserSongsAction::GetRelatedTracks => {
                if let Some(song) = self.track_list.get_list_iter().nth(cur) {
                    return (AsyncTask::new_no_op(), Some(AppCallback::GetRelatedTracks(song.video_id.clone())));
                }
            }
            BrowserSongsAction::RatePlaylist => {
                if self.show_tracks {
                    if let Some(pl_id) = &self.album_playlist_id {
                        let was_liked = false; // No local cache, always send Liked
                        let rating = if was_liked { LikeStatus::Indifferent } else { LikeStatus::Liked };
                        return (AsyncTask::new_no_op(), Some(AppCallback::RatePlaylistFromLibrary(pl_id.clone(), rating)));
                    }
                }
            }
            BrowserSongsAction::SubscribeToArtist => {
                if self.show_tracks {
                    if let Some(artist) = self.album_artists.first() {
                        if let Some(channel_id) = &artist.id {
                            return (AsyncTask::new_no_op(), Some(AppCallback::SubscribeToArtistFromLibrary(channel_id.clone())));
                        }
                    }
                }
            }
            BrowserSongsAction::UnsubscribeFromArtist => {
                if self.show_tracks {
                    if let Some(artist) = self.album_artists.first() {
                        if let Some(channel_id) = &artist.id {
                            return (AsyncTask::new_no_op(), Some(AppCallback::UnsubscribeFromArtistFromLibrary(vec![channel_id.clone()])));
                        }
                    }
                }
            }
            BrowserSongsAction::Filter => {
                self.toggle_filter();
            }
            BrowserSongsAction::Sort => {
                self.handle_pop_sort();
            }
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
        match self.input_routing {
            InputRouting::List => {
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
            InputRouting::Sort => {
                self.sort.cur = self
                    .sort
                    .cur
                    .saturating_add_signed(amount)
                    .min(self.get_sortable_columns().len().saturating_sub(1));
            }
            InputRouting::Search | InputRouting::Filter => {}
        }
    }
    fn is_scrollable(&self) -> bool {
        matches!(self.input_routing, InputRouting::List | InputRouting::Sort)
    }
}

impl TextHandler for AlbumSearchBrowser {
    fn get_text(&self) -> Option<&str> {
        if matches!(self.input_routing, InputRouting::Filter) {
            self.filter.get_text()
        } else if self.search_popped {
            Some(self.search.search_contents.get_text())
        } else {
            None
        }
    }
    fn clear_text(&mut self) -> bool {
        if self.search_popped {
            let had = !self.search.search_contents.get_text().is_empty();
            self.search.search_contents.clear();
            had
        } else {
            false
        }
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        self.search.search_contents.set_text(&text.into());
    }
    fn is_text_handling(&self) -> bool {
        self.search_popped || matches!(self.input_routing, InputRouting::Filter)
    }
    fn handle_text_event_impl(&mut self, event: &crossterm::event::Event) -> Option<AsyncTask<Self, Self::Bkend, Self::Md>> {
        if self.search_popped {
            return self.search.handle_text_event_impl(event)
                .map(|t| t.map_frontend(|this: &mut Self| &mut this.search));
        } else if matches!(self.input_routing, InputRouting::Filter) {
            self.filter.handle_text_event_impl(event).map(|t| t.map_frontend(|this: &mut Self| &mut this.filter))
        } else {
            None
        }
    }
}

impl Suggestable for AlbumSearchBrowser {
    fn get_search_suggestions(&self) -> &[SearchSuggestion] {
        self.search.get_search_suggestions()
    }
    fn has_search_suggestions(&self) -> bool {
        self.search.has_search_suggestions()
    }
}

impl ActionHandler<BrowserSearchAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, action: BrowserSearchAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            BrowserSearchAction::PrevSearchSuggestion => {
                self.search.increment_list(-1);
            }
            BrowserSearchAction::NextSearchSuggestion => {
                self.search.increment_list(1);
            }
            BrowserSearchAction::Close => {
                self.handle_toggle_search();
            }
        }
        (AsyncTask::new_no_op(), None)
    }
}

impl ActionHandler<FilterAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, _action: FilterAction) -> impl Into<YoutuiEffect<Self>> {
        self.filter.shown = !self.filter.shown;
        (AsyncTask::new_no_op(), None)
    }
}

impl ActionHandler<SortAction> for AlbumSearchBrowser {
    fn apply_action(&mut self, _action: SortAction) -> impl Into<YoutuiEffect<Self>> {
        self.sort.shown = !self.sort.shown;
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
                audio_playlist_id: album.audio_playlist_id,
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
    pub audio_playlist_id: Option<PlaylistID<'static>>,
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
        if target.albums.is_empty() && !target.search_popped {
            target.search_popped = true;
            target.search = SearchBlock::default();
        }
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
        target.album_playlist_id = result.audio_playlist_id;
        target.album_artists = result.artists.clone();
        let album = ParsedSongAlbum { name: result.title, id: result.album_id };
        target.track_list.append_raw_album_songs(result.tracks, album, result.year, result.artists, result.thumbnails);
        AsyncTask::new_no_op()
    }
});

impl_youtui_task_handler!(HandleFetchAlbumTracksError, anyhow::Error, AlbumSearchBrowser, |_, _err: anyhow::Error| {
    |_target: &mut AlbumSearchBrowser| AsyncTask::new_no_op()
});

#[derive(Debug, PartialEq)]
pub struct HandleSearchAlbumsOk;
#[derive(Debug, PartialEq)]
pub struct HandleSearchAlbumsError;

impl_youtui_task_handler!(HandleSearchAlbumsOk, Vec<SearchResultAlbum>, AlbumSearchBrowser, |_, a: Vec<SearchResultAlbum>| {
    move |target: &mut AlbumSearchBrowser| {
        target.albums = a.clone();
        target.album_selected = 0;
        target.show_tracks = false;
        if let Some(query) = target.last_search_query.take() {
            target.search_cache.put(query, a);
        }
        AsyncTask::new_no_op()
    }
});

impl_youtui_task_handler!(HandleSearchAlbumsError, anyhow::Error, AlbumSearchBrowser, |_, _err: anyhow::Error| {
    |_target: &mut AlbumSearchBrowser| AsyncTask::new_no_op()
});