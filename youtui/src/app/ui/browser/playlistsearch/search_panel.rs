use crate::app::component::actionhandler::{
    Action, Component, ComponentEffect, KeyRouter, Scrollable, Suggestable, TextHandler,
};
use crate::app::server::{ArcServer, TaskMetadata};
use crate::app::ui::action::AppAction;
use crate::app::ui::browser::shared_components::SearchBlock;
use crate::app::structures::fuzzy_match;
use crate::app::view::{HasTitle, ListView};
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::widgets::ScrollingListState;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::iter::Iterator;
use tracing::warn;
use ytmapi_rs::common::{PlaylistID, SearchSuggestion};
use ytmapi_rs::parse::SearchResultPlaylist;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum PlaylistInputRouting {
    Search,
    #[default]
    List,
}

/// Consolidation of the two SearchResultPlaylist types.
#[derive(Clone, Debug)]
pub struct NonPodcastSearchResultPlaylist {
    pub title: String,
    pub playlist_id: PlaylistID<'static>,
}

impl NonPodcastSearchResultPlaylist {
    pub fn new(p: SearchResultPlaylist) -> Option<NonPodcastSearchResultPlaylist> {
        match p {
            SearchResultPlaylist::Featured(p) => Some(NonPodcastSearchResultPlaylist {
                title: p.title,
                playlist_id: p.playlist_id,
            }),
            SearchResultPlaylist::Community(p) => Some(NonPodcastSearchResultPlaylist {
                title: p.title,
                playlist_id: p.playlist_id,
            }),
            SearchResultPlaylist::Podcast(_) => None,
            other => {
                warn!(
                    "New SearchResultPlaylist type {:?} has been implemented by ytmapi-rs and this is currently ignored by youtui",
                    other
                );
                None
            }
        }
    }
}

pub struct PlaylistSearchPanel {
    pub list: Vec<NonPodcastSearchResultPlaylist>,
    // Duplicate of search popped?
    // Could be a function instead.
    pub route: PlaylistInputRouting,
    selected: usize,
    pub search_popped: bool,
    pub search: SearchBlock,
    pub widget_state: ScrollingListState,
    pub local_filter_text: String,
}

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserPlaylistsAction {
    DisplaySelectedPlaylist,
}

impl Action for BrowserPlaylistsAction {
    fn context(&self) -> std::borrow::Cow<'_, str> {
        "Playlist Search Panel".into()
    }
    fn describe(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Self::DisplaySelectedPlaylist => "Show playlist tracks",
        }
        .into()
    }
}

impl PlaylistSearchPanel {
    pub fn new() -> Self {
        Self {
            list: Default::default(),
            route: Default::default(),
            selected: Default::default(),
            search_popped: false,
            search: SearchBlock::default(),
            widget_state: Default::default(),
            local_filter_text: String::new(),
        }
    }
    pub fn open_search(&mut self) {
        self.search_popped = true;
        self.route = PlaylistInputRouting::Search;
    }
    pub fn close_search(&mut self) {
        self.search_popped = false;
        self.route = PlaylistInputRouting::List;
    }
}
impl Component for PlaylistSearchPanel {
    type Bkend = ArcServer;
    type Md = TaskMetadata;
}

impl TextHandler for PlaylistSearchPanel {
    fn is_text_handling(&self) -> bool {
        self.route == PlaylistInputRouting::Search
    }
    fn get_text(&self) -> std::option::Option<&str> {
        self.search.get_text()
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        self.search.replace_text(text)
    }
    fn clear_text(&mut self) -> bool {
        self.search.clear_text()
    }
    fn handle_text_event_impl(
        &mut self,
        event: &crossterm::event::Event,
    ) -> Option<ComponentEffect<Self>> {
        self.search
            .handle_text_event_impl(event)
            .map(|effect| effect.map_frontend(|this: &mut Self| &mut this.search))
    }
}

impl Suggestable for PlaylistSearchPanel {
    fn get_search_suggestions(&self) -> &[SearchSuggestion] {
        self.search.get_search_suggestions()
    }
    fn has_search_suggestions(&self) -> bool {
        self.search.has_search_suggestions()
    }
}

impl KeyRouter<AppAction> for PlaylistSearchPanel {
    fn get_all_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        [
            &config.keybinds.browser_playlists,
            &config.keybinds.browser_search,
        ]
        .into_iter()
    }
    fn get_active_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        match self.route {
            PlaylistInputRouting::List => std::iter::once(&config.keybinds.browser_playlists),
            PlaylistInputRouting::Search => std::iter::once(&config.keybinds.browser_search),
        }
    }
}

impl Scrollable for PlaylistSearchPanel {
    fn increment_list(&mut self, amount: isize) {
        self.selected = self
            .selected
            .checked_add_signed(amount)
            .unwrap_or(0)
            .min(self.len().checked_add_signed(-1).unwrap_or(0));
    }
    fn is_scrollable(&self) -> bool {
        self.route == PlaylistInputRouting::List
    }
}
impl ListView for PlaylistSearchPanel {
    fn get_selected_item(&self) -> usize {
        self.selected
    }
    fn get_state(&self) -> &ScrollingListState {
        &self.widget_state
    }
    fn get_mut_state(&mut self) -> &mut ScrollingListState {
        &mut self.widget_state
    }
    fn get_items(&self) -> impl ExactSizeIterator<Item = Cow<'_, str>> + '_ {
        self.get_filtered_list_iter()
            .map(|search_result| (&search_result.title).into())
            .collect::<Vec<Cow<'_, str>>>()
            .into_iter()
    }
}
impl PlaylistSearchPanel {
    /// Filter `self.list` by `local_filter_text` (set by `/` fuzzy finder or F1 filter).
    /// Empty filter returns all items. Used by `get_items` so the visible list tracks the filter.
    pub fn get_filtered_list_iter(&self) -> impl Iterator<Item = &NonPodcastSearchResultPlaylist> {
        let filter = self.local_filter_text.trim();
        self.list
            .iter()
            .filter(move |search_result| filter.is_empty() || fuzzy_match(filter, &search_result.title).is_some())
    }
}
impl HasTitle for PlaylistSearchPanel {
    fn get_title(&self) -> Cow<'_, str> {
        if self.local_filter_text.trim().is_empty() {
            format!("Playlists - {} results", self.list.len()).into()
        } else {
            format!(
                "Playlists - {} results [SEARCH: {} ({})]",
                self.get_filtered_list_iter().count(),
                self.local_filter_text,
                self.list.len()
            )
            .into()
        }
    }
}
