use self::draw::draw_browser;
use super::action::{AppAction, TextEntryAction};
use super::{AppCallback, WindowContext};
use crate::app::{NavTarget};
use crate::app::component::actionhandler::{
    Action, ActionHandler, ComponentEffect, DelegateScrollable, DominantKeyRouter, KeyRouter,
    Scrollable, TextHandler, YoutuiEffect, apply_action_mapped,
};
use crate::app::ui::browser::library::{BrowserLibraryAction, LibraryBrowser};
use vi_text_editor::ViTextEditor;
use crate::app::ui::browser::albumsearch::AlbumSearchBrowser;
use crate::app::view::{DrawableMut, HasTabs};
use crate::config::Config;
use crate::config::keymap::Keymap;
use artistsearch::ArtistSearchBrowser;
use artistsearch::search_panel::BrowserArtistsAction;
use artistsearch::songs_panel::BrowserArtistSongsAction;
use async_callback_manager::AsyncTask;
use itertools::Either;
use serde::{Deserialize, Serialize};
use shared_components::{BrowserSearchAction, FilterAction, SortAction};
use songsearch::{BrowserSongsAction, SongSearchBrowser};
use playlistsearch::PlaylistSearchBrowser;
use playlistsearch::search_panel::BrowserPlaylistsAction;
use playlistsearch::songs_panel::BrowserPlaylistSongsAction;
use std::borrow::Cow;
use std::convert::Into;
use std::iter::{IntoIterator, Iterator};
use tracing::warn;

pub mod artistsearch;
mod draw;
pub mod library;
pub mod albumsearch;
pub mod playlistsearch;
pub mod shared_components;
pub mod songsearch;

#[derive(Default, Copy, Clone, PartialEq)]
enum BrowserVariant {
    #[default]
    Artist,
    Album,
    Song,
    PlaylistSearch,
    LibraryPlaylist,
}

pub struct Browser {
    variant: BrowserVariant,
    artist_search_browser: ArtistSearchBrowser,
    song_search_browser: SongSearchBrowser,
    album_search_browser: AlbumSearchBrowser,
    pub library_browser: LibraryBrowser,
    pub playlist_search_browser: PlaylistSearchBrowser,
    state_stack: Vec<BrowserSnapshot>,
    filter_editor: ViTextEditor,
    filter_active: bool,
    filter_text: String,
}

#[derive(Clone)]
struct BrowserSnapshot {
    variant: BrowserVariant,
    library_category: Option<library::LibraryCategory>,
}
impl_youtui_component!(Browser);

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAction {
    ViewPlaylist,
    Search,
    LocalFilter,
    Left,
    Right,
    ChangeSearchType,
    Back,
}

impl Action for BrowserAction {
    fn context(&self) -> Cow<'_, str> {
        "Browser".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            BrowserAction::ViewPlaylist => "View Playlist",
            BrowserAction::Search => "Toggle Search",
            BrowserAction::LocalFilter => "Local Filter",
            BrowserAction::Left => "Left",
            BrowserAction::Right => "Right",
            BrowserAction::ChangeSearchType => "Change Search Type",
            BrowserAction::Back => "Go Back",
        }
        .into()
    }
}

impl DelegateScrollable for Browser {
    fn delegate_mut(&mut self) -> &mut dyn Scrollable {
        match self.variant {
            BrowserVariant::Artist => &mut self.artist_search_browser as &mut dyn Scrollable,
            BrowserVariant::Song => &mut self.song_search_browser as &mut dyn Scrollable,
            BrowserVariant::Album => &mut self.album_search_browser as &mut dyn Scrollable,
            BrowserVariant::LibraryPlaylist => &mut self.library_browser as &mut dyn Scrollable,
            BrowserVariant::PlaylistSearch => &mut self.playlist_search_browser as &mut dyn Scrollable,
        }
    }
    fn delegate_ref(&self) -> &dyn Scrollable {
        match self.variant {
            BrowserVariant::Artist => &self.artist_search_browser as &dyn Scrollable,
            BrowserVariant::Song => &self.song_search_browser as &dyn Scrollable,
            BrowserVariant::Album => &self.album_search_browser as &dyn Scrollable,
            BrowserVariant::LibraryPlaylist => &self.library_browser as &dyn Scrollable,
            BrowserVariant::PlaylistSearch => &self.playlist_search_browser as &dyn Scrollable,
        }
    }
}
impl ActionHandler<BrowserSearchAction> for Browser {
    fn apply_action(&mut self, action: BrowserSearchAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::Artist => apply_action_mapped(self, action, |this: &mut Self| {
                &mut this.artist_search_browser
            }),
            BrowserVariant::Song => apply_action_mapped(self, action, |this: &mut Self| {
                &mut this.song_search_browser
            }),
            BrowserVariant::Album => apply_action_mapped(self, action, |this: &mut Self| {
                &mut this.album_search_browser
            }),
            BrowserVariant::LibraryPlaylist => apply_action_mapped(self, action, |this: &mut Self| {
                &mut this.library_browser
            }),
            BrowserVariant::PlaylistSearch => apply_action_mapped(self, action, |this: &mut Self| {
                &mut this.playlist_search_browser
            }),
        }
    }
}
impl ActionHandler<BrowserArtistSongsAction> for Browser {
    fn apply_action(&mut self, action: BrowserArtistSongsAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::Artist => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.artist_search_browser
                });
            }
            _ => warn!(
                "Received action {:?} but artist search browser not active",
                action
            ),
        };
        YoutuiEffect::new_no_op()
    }
}
impl ActionHandler<BrowserArtistsAction> for Browser {
    fn apply_action(&mut self, action: BrowserArtistsAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::Artist => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.artist_search_browser
                });
            }
            _ => warn!(
                "Received action {:?} but artist search browser not active",
                action
            ),
        }
        YoutuiEffect::new_no_op()
    }
}
impl ActionHandler<BrowserSongsAction> for Browser {
    fn apply_action(&mut self, action: BrowserSongsAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::Song => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.song_search_browser
                });
            }
            BrowserVariant::LibraryPlaylist => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.library_browser
                });
            }
            BrowserVariant::Album => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.album_search_browser
                });
            }
            _ => warn!(
                "Received action {:?} but song search browser not active",
                action
            ),
        }
        YoutuiEffect::new_no_op()
    }
}
impl ActionHandler<BrowserLibraryAction> for Browser {
    fn apply_action(&mut self, action: BrowserLibraryAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::LibraryPlaylist => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.library_browser
                });
            }
            _ => warn!(
                "Received action {:?} but library browser not active",
                action
            ),
        }
        YoutuiEffect::new_no_op()
    }
}
impl ActionHandler<BrowserAction> for Browser {
    fn apply_action(&mut self, action: BrowserAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            BrowserAction::Left => {
                if let Some(task) = self.left() {
                    return (task, None);
                }
            }
            BrowserAction::Right => {
                if let Some(task) = self.right() {
                    return (task, None);
                }
            }
            BrowserAction::ViewPlaylist => {
                return (
                    AsyncTask::new_no_op(),
                    Some(AppCallback::ChangeContext(WindowContext::Playlist)),
                );
            }
            BrowserAction::Search => self.handle_toggle_search(),
            BrowserAction::LocalFilter => self.handle_toggle_local_filter(),
            BrowserAction::ChangeSearchType => {
                if let Some(task) = self.handle_change_search_type() {
                    return (task, None);
                }
            }
            BrowserAction::Back => {
                self.navigate_back();
            }
        }
        (AsyncTask::new_no_op(), None)
    }
}
impl ActionHandler<FilterAction> for Browser {
    fn apply_action(&mut self, action: FilterAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::Artist => self
                .artist_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.artist_search_browser),
            BrowserVariant::Song => self
                .song_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.song_search_browser),
            BrowserVariant::Album => self
                .album_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.album_search_browser),
            BrowserVariant::LibraryPlaylist => self
                .library_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.library_browser),
            BrowserVariant::PlaylistSearch => self
                .playlist_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.playlist_search_browser),
        }
    }
}
impl ActionHandler<SortAction> for Browser {
    fn apply_action(&mut self, action: SortAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::Artist => self
                .artist_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.artist_search_browser),
            BrowserVariant::Song => self
                .song_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.song_search_browser),
            BrowserVariant::Album => self
                .album_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.album_search_browser),
            BrowserVariant::LibraryPlaylist => self
                .library_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.library_browser),
            BrowserVariant::PlaylistSearch => self
                .playlist_search_browser
                .apply_action(action)
                .into()
                .map(|this: &mut Self| &mut this.playlist_search_browser),
        }
    }
}
impl ActionHandler<BrowserPlaylistsAction> for Browser {
    fn apply_action(&mut self, action: BrowserPlaylistsAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::PlaylistSearch => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.playlist_search_browser
                });
            }
            _ => warn!(
                "Received action {:?} but playlist search browser not active",
                action
            ),
        }
        YoutuiEffect::new_no_op()
    }
}
impl ActionHandler<BrowserPlaylistSongsAction> for Browser {
    fn apply_action(&mut self, action: BrowserPlaylistSongsAction) -> impl Into<YoutuiEffect<Self>> {
        match self.variant {
            BrowserVariant::PlaylistSearch => {
                return apply_action_mapped(self, action, |this: &mut Self| {
                    &mut this.playlist_search_browser
                });
            }
            _ => warn!(
                "Received action {:?} but playlist search browser not active",
                action
            ),
        }
        YoutuiEffect::new_no_op()
    }
}
impl TextHandler for Browser {
    fn is_text_handling(&self) -> bool {
        if self.filter_active {
            return true;
        }
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.is_text_handling(),
            BrowserVariant::Song => self.song_search_browser.is_text_handling(),
            BrowserVariant::Album => self.album_search_browser.is_text_handling(),
            BrowserVariant::LibraryPlaylist => self.library_browser.is_text_handling(),
            BrowserVariant::PlaylistSearch => self.playlist_search_browser.is_text_handling(),
        }
    }
    fn get_text(&self) -> std::option::Option<&str> {
        if self.filter_active {
            return Some(self.filter_editor.get_text());
        }
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.get_text(),
            BrowserVariant::Song => self.song_search_browser.get_text(),
            BrowserVariant::Album => self.album_search_browser.get_text(),
            BrowserVariant::LibraryPlaylist => self.library_browser.get_text(),
            BrowserVariant::PlaylistSearch => self.playlist_search_browser.get_text(),
        }
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        if self.filter_active {
            self.filter_editor.set_text(&text.into());
            return;
        }
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.replace_text(text),
            BrowserVariant::Song => self.song_search_browser.replace_text(text),
            BrowserVariant::Album => self.album_search_browser.replace_text(text),
            BrowserVariant::LibraryPlaylist => self.library_browser.replace_text(text),
            BrowserVariant::PlaylistSearch => self.playlist_search_browser.replace_text(text),
        }
    }
    fn clear_text(&mut self) -> bool {
        if self.filter_active {
            let had_text = !self.filter_editor.get_text().is_empty();
            self.filter_editor.clear();
            return had_text;
        }
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.clear_text(),
            BrowserVariant::Song => self.song_search_browser.clear_text(),
            BrowserVariant::Album => self.album_search_browser.clear_text(),
            BrowserVariant::LibraryPlaylist => self.library_browser.clear_text(),
            BrowserVariant::PlaylistSearch => self.playlist_search_browser.clear_text(),
        }
    }
    fn handle_text_event_impl(
        &mut self,
        event: &crossterm::event::Event,
    ) -> Option<ComponentEffect<Self>> {
        if self.filter_active {
            if let crossterm::event::Event::Key(k) = event {
                if k.kind == crossterm::event::KeyEventKind::Press {
                    match k.code {
                        crossterm::event::KeyCode::Esc => {
                            self.filter_text.clear();
                            self.filter_active = false;
                            self.filter_editor.clear();
                            self.sync_local_filter();
                        }
                        crossterm::event::KeyCode::Enter => {
                            self.filter_active = false;
                        }
                        _ => {
                            self.filter_editor.handle_key(k.code, k.modifiers.contains(crossterm::event::KeyModifiers::SHIFT), false);
                            self.sync_local_filter();
                        }
                    }
                }
            }
            return None;
        }
        match self.variant {
            BrowserVariant::Artist => self
                .artist_search_browser
                .handle_text_event_impl(event)
                .map(|effect| {
                    effect.map_frontend(|this: &mut Self| &mut this.artist_search_browser)
                }),
            BrowserVariant::Song => self
                .song_search_browser
                .handle_text_event_impl(event)
                .map(|effect| effect.map_frontend(|this: &mut Self| &mut this.song_search_browser)),
            BrowserVariant::Album => self
                .album_search_browser
                .handle_text_event_impl(event)
                .map(|effect| {
                    effect.map_frontend(|this: &mut Self| &mut this.album_search_browser)
                }),
            BrowserVariant::LibraryPlaylist => self
                .library_browser
                .handle_text_event_impl(event)
                .map(|effect| effect.map_frontend(|this: &mut Self| &mut this.library_browser)),
            BrowserVariant::PlaylistSearch => self
                .playlist_search_browser
                .handle_text_event_impl(event)
                .map(|effect| effect.map_frontend(|this: &mut Self| &mut this.playlist_search_browser)),
        }
    }
}

impl DrawableMut for Browser {
    fn draw_mut_chunk(
        &mut self,
        f: &mut ratatui::Frame,
        chunk: ratatui::prelude::Rect,
        selected: bool,
        cur_tick: u64,
    ) {
        self.sync_local_filter();
        draw_browser(f, self, chunk, selected, cur_tick);
    }
}
impl HasTabs for Browser {
    fn tabs_block_title(&'_ self) -> Cow<'_, str> {
        "Browser".into()
    }
    fn tab_items(&'_ self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> + '_ {
        ["Artists", "Albums", "Songs", "Playlists", "Library"]
    }
    fn selected_tab_idx(&self) -> usize {
        self.variant as usize
    }
}
impl KeyRouter<AppAction> for Browser {
    fn get_all_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        [
            &config.keybinds.browser,
            &config.keybinds.browser_search,
            &config.keybinds.filter,
            &config.keybinds.sort,
        ]
        .into_iter()
        .chain(self.artist_search_browser.get_all_keybinds(config))
        .chain(self.song_search_browser.get_all_keybinds(config))
        .chain(self.album_search_browser.get_all_keybinds(config))
        .chain(self.library_browser.get_all_keybinds(config))
        .chain(self.playlist_search_browser.get_all_keybinds(config))
    }
    fn get_active_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        if self.dominant_keybinds_active() {
            return Either::Left(self.get_dominant_keybinds(config));
        }
        let base: Vec<&Keymap<AppAction>> = vec![&config.keybinds.browser];
        let extra: Vec<&Keymap<AppAction>> = match self.variant {
            BrowserVariant::Song => {
                self.song_search_browser.get_active_keybinds(config).collect()
            }
            BrowserVariant::Artist => {
                self.artist_search_browser.get_active_keybinds(config).collect()
            }
            BrowserVariant::Album => {
                self.album_search_browser.get_active_keybinds(config).collect()
            }
            BrowserVariant::LibraryPlaylist => {
                self.library_browser.get_active_keybinds(config).collect()
            }
            BrowserVariant::PlaylistSearch => {
                self.playlist_search_browser.get_active_keybinds(config).collect()
            }
        };
        Either::Right(base.into_iter().chain(extra.into_iter()))
    }
}
impl DominantKeyRouter<AppAction> for Browser {
    fn dominant_keybinds_active(&self) -> bool {
        match self.variant {
            BrowserVariant::Song => {
                self.song_search_browser.sort.shown || self.song_search_browser.filter.shown
            }
            BrowserVariant::Artist => {
                self.artist_search_browser.album_songs_panel.sort.shown
                    || self.artist_search_browser.album_songs_panel.filter.shown
            }
            BrowserVariant::Album => false,
            BrowserVariant::LibraryPlaylist => {
                self.library_browser.sort.shown || self.library_browser.filter.shown
            }
            BrowserVariant::PlaylistSearch => false,
        }
    }
    fn get_dominant_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        match self.variant {
            BrowserVariant::Artist => match self.artist_search_browser.album_songs_panel.route {
                artistsearch::songs_panel::AlbumSongsInputRouting::List => {
                    Either::Left(std::iter::empty())
                }
                artistsearch::songs_panel::AlbumSongsInputRouting::Sort => {
                    Either::Right(std::iter::once(&config.keybinds.sort))
                }
                artistsearch::songs_panel::AlbumSongsInputRouting::Filter => {
                    Either::Right(std::iter::once(&config.keybinds.filter))
                }
            },
            BrowserVariant::Album => Either::Left(std::iter::empty()),
            BrowserVariant::Song => match self.song_search_browser.input_routing {
                songsearch::InputRouting::List | songsearch::InputRouting::Search => {
                    Either::Left(std::iter::empty())
                }
                songsearch::InputRouting::Filter => {
                    Either::Right(std::iter::once(&config.keybinds.filter))
                }
                songsearch::InputRouting::Sort => {
                    Either::Right(std::iter::once(&config.keybinds.sort))
                }
            },
            BrowserVariant::LibraryPlaylist => match self.library_browser.input_routing {
                library::InputRouting::Search => {
                    Either::Right(std::iter::once(&config.keybinds.browser_search))
                }
                library::InputRouting::Category => Either::Left(std::iter::empty()),
                library::InputRouting::Content => match self.library_browser.sort.shown {
                    true => Either::Right(std::iter::once(&config.keybinds.sort)),
                    false => match self.library_browser.filter.shown {
                        true => Either::Right(std::iter::once(&config.keybinds.filter)),
                        false => Either::Left(std::iter::empty()),
                    },
                },
            },
            BrowserVariant::PlaylistSearch => Either::Left(std::iter::empty()),
        }
    }
}

impl Browser {
    pub fn new() -> Self {
        Self {
            variant: Default::default(),
            artist_search_browser: ArtistSearchBrowser::new(),
            song_search_browser: SongSearchBrowser::new(),
            album_search_browser: AlbumSearchBrowser::new(),
            library_browser: LibraryBrowser::new(),
            playlist_search_browser: PlaylistSearchBrowser::new(),
            state_stack: Vec::new(),
            filter_editor: ViTextEditor::new(),
            filter_active: false,
            filter_text: String::new(),
        }
    }
    pub fn sync_local_filter(&mut self) {
        let text = if self.filter_active {
            let t = self.filter_editor.get_text().to_string();
            self.filter_text = t.clone();
            t
        } else {
            self.filter_text.clone()
        };
        match self.variant {
            BrowserVariant::Artist => {
                self.artist_search_browser.artist_search_panel.local_filter_text = text.clone();
                self.artist_search_browser.album_songs_panel.local_filter_text = text;
            }
            BrowserVariant::Song => self.song_search_browser.local_filter_text = text,
            BrowserVariant::Album => self.album_search_browser.local_filter_text = text,
            BrowserVariant::LibraryPlaylist => self.library_browser.local_filter_text = text,
            BrowserVariant::PlaylistSearch => {
                self.playlist_search_browser.playlist_search_panel.local_filter_text = text.clone();
                self.playlist_search_browser.playlist_songs_panel.local_filter_text = text;
            }
        }
    }
    pub fn set_cur_playing_video_id(&mut self, video_id: Option<ytmapi_rs::common::VideoID<'static>>) {
        self.library_browser.cur_playing_video_id = video_id.clone();
        self.song_search_browser.cur_playing_video_id = video_id.clone();
        self.album_search_browser.cur_playing_video_id = video_id.clone();
        self.artist_search_browser.album_songs_panel.cur_playing_video_id = video_id.clone();
        self.playlist_search_browser.playlist_songs_panel.cur_playing_video_id = video_id;
    }
    pub fn navigate_to(&mut self, target: NavTarget) -> Option<AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>> {
        self.push_snapshot();
        match target {
            NavTarget::Artist(name) => {
                self.variant = BrowserVariant::Artist;
                self.artist_search_browser.artist_search_panel.search.replace_text(name);
                let effect = self.artist_search_browser.search();
                Some(effect.map_frontend(|this: &mut Self| &mut this.artist_search_browser))
            }
            NavTarget::Album { artist, album } => {
                self.variant = BrowserVariant::Album;
                let query = format!("{artist} {album}");
                let (search_task, _) = self.album_search_browser.search_albums_query(query);
                Some(search_task.map_frontend(|this: &mut Self| &mut this.album_search_browser))
            }
            NavTarget::ArtistChannel(channel_id) => {
                self.variant = BrowserVariant::Artist;
                self.artist_search_browser.artist_search_panel.search_popped = false;
                self.artist_search_browser.input_routing = artistsearch::InputRouting::Song;
                Some(self.artist_search_browser.load_artist_by_id(channel_id).map_frontend(|b: &mut Self| &mut b.artist_search_browser))
            }
            NavTarget::SongSearch(query) => {
                self.variant = BrowserVariant::Song;
                self.song_search_browser.search.replace_text(query);
                self.song_search_browser.handle_toggle_search();
                None
            }
        }
    }
    fn push_snapshot(&mut self) {
        let snapshot = BrowserSnapshot {
            variant: self.variant,
            library_category: match self.variant {
                BrowserVariant::LibraryPlaylist => {
                    Some(self.library_browser.category)
                }
                _ => None,
            },
        };
        self.state_stack.push(snapshot);
    }
    pub fn navigate_back(&mut self) {
        if let Some(snapshot) = self.state_stack.pop() {
            self.variant = snapshot.variant;
            if let Some(cat) = snapshot.library_category {
                if self.variant == BrowserVariant::LibraryPlaylist {
                    self.library_browser.category = cat;
                    self.library_browser.input_routing = library::InputRouting::Content;
                }
            }
        }
    }
    pub fn text_editor_mode(&self) -> Option<String> {
        if self.filter_active {
            let mode = self.filter_editor.mode_char();
            return Some(format!("{} SEARCH: {}", mode, self.filter_editor.get_text()));
        }
        if !self.filter_text.is_empty() {
            return Some(format!("SEARCH: {}", self.filter_text));
        }
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.text_editor_mode(),
            BrowserVariant::Song => self.song_search_browser.text_editor_mode(),
            BrowserVariant::Album => self.album_search_browser.text_editor_mode(),
            BrowserVariant::LibraryPlaylist => self.library_browser.text_editor_mode(),
            BrowserVariant::PlaylistSearch => self.playlist_search_browser.text_editor_mode(),
        }
    }
    pub fn left(&mut self) -> Option<AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>> {
        match self.variant {
            BrowserVariant::Artist => { self.artist_search_browser.left(); None }
            BrowserVariant::Album => { self.album_search_browser.left(); None }
            BrowserVariant::Song => None,
            BrowserVariant::LibraryPlaylist => {
                if self.library_browser.show_playlist_tracks {
                    self.library_browser.show_playlist_tracks = false;
                    self.library_browser.playlist_tracks.clear();
                } else {
                    self.library_browser.focus_category();
                }
                None
            }
            BrowserVariant::PlaylistSearch => { self.playlist_search_browser.left(); None }
        }
    }
    pub fn right(&mut self) -> Option<AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>> {
        match self.variant {
            BrowserVariant::Artist => { self.artist_search_browser.right(); None }
            BrowserVariant::Album => { let _ = self.album_search_browser.right(); None }
            BrowserVariant::Song => None,
            BrowserVariant::LibraryPlaylist => {
                let task = self.library_browser.focus_content();
                Some(task.map_frontend(|this: &mut Self| &mut this.library_browser))
            }
            BrowserVariant::PlaylistSearch => { self.playlist_search_browser.right(); None }
        }
    }
    pub fn handle_text_entry_action(&mut self, action: TextEntryAction) -> ComponentEffect<Self> {
        match self.variant {
            BrowserVariant::Artist => self
                .artist_search_browser
                .handle_text_entry_action(action)
                .map_frontend(|this: &mut Self| &mut this.artist_search_browser),
            BrowserVariant::Song => self
                .song_search_browser
                .handle_text_entry_action(action)
                .map_frontend(|this: &mut Self| &mut this.song_search_browser),
            BrowserVariant::Album => self
                .album_search_browser
                .handle_text_entry_action(action)
                .map_frontend(|this: &mut Self| &mut this.album_search_browser),
            BrowserVariant::LibraryPlaylist => self
                .library_browser
                .handle_text_entry_action(action)
                .map_frontend(|this: &mut Self| &mut this.library_browser),
            BrowserVariant::PlaylistSearch => self
                .playlist_search_browser
                .handle_text_entry_action(action)
                .map_frontend(|this: &mut Self| &mut this.playlist_search_browser),
        }
    }
    pub fn handle_toggle_search(&mut self) {
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.handle_toggle_search(),
            BrowserVariant::Song => self.song_search_browser.handle_toggle_search(),
            BrowserVariant::Album => self.album_search_browser.handle_toggle_search(),
            BrowserVariant::LibraryPlaylist => self.library_browser.handle_toggle_search(),
            BrowserVariant::PlaylistSearch => self.playlist_search_browser.handle_toggle_search(),
        }
    }
    pub fn handle_toggle_local_filter(&mut self) {
        if self.filter_text.is_empty() {
            self.filter_active = !self.filter_active;
            if self.filter_active {
                self.filter_editor = ViTextEditor::new();
            }
        } else {
            self.filter_text.clear();
            self.filter_active = false;
            self.filter_editor.clear();
            self.sync_local_filter();
        }
    }
    pub fn dismiss_search(&mut self) {
        if self.filter_active {
            self.filter_active = false;
            self.filter_editor.clear();
        }
        match self.variant {
            BrowserVariant::Artist => {
                if self.artist_search_browser.artist_search_panel.search_popped {
                    self.artist_search_browser.artist_search_panel.close_search();
                    self.artist_search_browser.revert_routing();
                }
            }
            BrowserVariant::Song => {
                if self.song_search_browser.search_popped {
                    self.song_search_browser.handle_toggle_search();
                }
            }
            BrowserVariant::Album => {
                if self.album_search_browser.search_popped {
                    self.album_search_browser.handle_toggle_search();
                }
            }
            BrowserVariant::LibraryPlaylist => {
                if self.library_browser.input_routing == library::InputRouting::Search {
                    self.library_browser.handle_toggle_search();
                }
            }
            BrowserVariant::PlaylistSearch => {
                if self.playlist_search_browser.is_text_handling() {
                    self.playlist_search_browser.handle_toggle_search();
                }
            }
        }
    }
    pub fn handle_change_search_type(&mut self) -> Option<AsyncTask<Self, crate::app::server::ArcServer, crate::app::TaskMetadata>> {
        self.push_snapshot();
        match self.variant {
            BrowserVariant::Artist => {
                self.variant = BrowserVariant::Album;
                let (task, _) = self.album_search_browser.fetch_albums();
                Some(task.map_frontend(|this: &mut Self| &mut this.album_search_browser))
            }
            BrowserVariant::Album => {
                self.variant = BrowserVariant::Song;
                None
            }
            BrowserVariant::Song => {
                self.variant = BrowserVariant::PlaylistSearch;
                None
            }
            BrowserVariant::PlaylistSearch => {
                self.variant = BrowserVariant::LibraryPlaylist;
                Some(
                    self.library_browser
                        .fetch_current_category()
                        .map_frontend(|this: &mut Self| &mut this.library_browser),
                )
            }
            BrowserVariant::LibraryPlaylist => {
                self.variant = BrowserVariant::Artist;
                None
            }
        }
    }
    pub fn go_to_first(&mut self) {
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.go_to_first(),
            BrowserVariant::Song => self.song_search_browser.go_to_first(),
            BrowserVariant::Album => self.album_search_browser.go_to_first(),
            BrowserVariant::LibraryPlaylist => {
                self.library_browser.increment_list(-isize::MAX);
            }
            BrowserVariant::PlaylistSearch => {
                self.playlist_search_browser.increment_list(-isize::MAX);
            }
        }
    }

    pub fn go_to_last(&mut self) {
        match self.variant {
            BrowserVariant::Artist => self.artist_search_browser.go_to_last(),
            BrowserVariant::Song => self.song_search_browser.go_to_last(),
            BrowserVariant::Album => self.album_search_browser.go_to_last(),
            BrowserVariant::LibraryPlaylist => {
                self.library_browser.increment_list(isize::MAX);
            }
            BrowserVariant::PlaylistSearch => {
                self.playlist_search_browser.increment_list(isize::MAX);
            }
        }
    }
}

pub fn get_sort_keybinds(config: &Config) -> impl Iterator<Item = &Keymap<AppAction>> + '_ {
    [&config.keybinds.sort, &config.keybinds.list].into_iter()
}

#[cfg(test)]
mod tests {
    use super::Browser;
    use super::artistsearch::songs_panel::BrowserArtistSongsAction;
    use crate::app::component::actionhandler::{ActionHandler, KeyRouter};
    use crate::app::ui::action::AppAction;
    use crate::app::ui::browser::BrowserAction;
    use crate::app::ui::browser::shared_components::BrowserSearchAction;
    use crate::config::Config;
    use crate::config::keymap::KeyActionTree;
    use crate::keyaction::KeyActionVisibility;
    use crate::keybind::Keybind;
    use itertools::Itertools;
    #[tokio::test]
    async fn toggle_search_opens_popup() {
        let mut b = Browser::new();
        b.apply_action(BrowserArtistSongsAction::Filter);
        assert!(b.artist_search_browser.album_songs_panel.filter.shown);
    }
    #[tokio::test]
    async fn artist_search_panel_search_suggestions_has_correct_keybinds() {
        let cfg = Config::default();
        let b = Browser::new();
        // Toggle search to make search keybindings active
        // (ArtistInputRouting defaults to List now)
        let mut b2 = Browser::new();
        b2.artist_search_browser.handle_toggle_search();
        let actual_kb = b2.get_active_keybinds(&cfg);
        let expected_kb = (
            &Keybind::new(crossterm::event::KeyCode::Char('n'), crossterm::event::KeyModifiers::CONTROL),
            &KeyActionTree::new_key(AppAction::BrowserSearch(
                BrowserSearchAction::NextSearchSuggestion,
            )),
        );
        let kb_found = actual_kb
            .inspect(|kb| println!("{kb:#?}"))
            .any(|km| km.iter().contains(&expected_kb));
        assert!(kb_found);
    }
    #[tokio::test]
    async fn songs_search_panel_search_suggestions_has_correct_keybinds() {
        let cfg = Config::default();
        let mut b = Browser::new();
        b.apply_action(BrowserAction::ChangeSearchType);
        b.apply_action(BrowserAction::ChangeSearchType);
        b.song_search_browser.handle_toggle_search();
        let actual_kb = b.get_active_keybinds(&cfg);
        let expected_kb = (
            &Keybind::new(crossterm::event::KeyCode::Char('n'), crossterm::event::KeyModifiers::CONTROL),
            &KeyActionTree::new_key(AppAction::BrowserSearch(
                BrowserSearchAction::NextSearchSuggestion,
            )),
        );
        let kb_found = actual_kb
            .inspect(|kb| println!("{kb:#?}"))
            .any(|km| km.iter().contains(&expected_kb));
        assert!(kb_found);
    }
    #[tokio::test]
    async fn artist_songs_panel_has_correct_keybinds() {
        let cfg = Config::default();
        let mut b = Browser::new();
        b.apply_action(BrowserAction::Right);
        let actual_kb = b.get_active_keybinds(&cfg);
        let expected_kb = (
            &Keybind::new_unmodified(crossterm::event::KeyCode::Char('c')),
            &KeyActionTree::new_key_with_visibility(
                AppAction::BrowserArtistSongs(BrowserArtistSongsAction::ToggleCategoryFilter),
                KeyActionVisibility::Global,
            ),
        );
        let kb_found = actual_kb
            .inspect(|kb| println!("{kb:#?}"))
            .any(|km| km.iter().contains(&expected_kb));
        assert!(kb_found);
    }
}
