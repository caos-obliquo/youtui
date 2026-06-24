use crate::app::component::actionhandler::{
    Action, ComponentEffect, KeyRouter, Scrollable, TextHandler,
};
use crate::app::structures::{
    BrowserSongsList, ListSong, ListSongDisplayableField, ListStatus, Percentage, SongListComponent, fuzzy_match,
};
use crate::app::ui::action::AppAction;
use crate::app::ui::browser::get_sort_keybinds;
use crate::app::ui::browser::shared_components::{
    FilterManager, SortManager, get_adjusted_list_column,
};
use crate::app::view::{
    AdvancedTableView, BasicConstraint, FilterString, HasTitle, Loadable, SortDirection,
    TableFilterCommand, TableSortCommand, TableView,
};
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::drawutils::get_offset_after_list_resize;
use crate::widgets::ScrollingTableState;
use anyhow::{Result, bail};
use itertools::Either;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::iter::{ExactSizeIterator, Iterator};
use tracing::warn;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum PlaylistSongsInputRouting {
    #[default]
    List,
    Sort,
    Filter,
}

#[derive(Clone)]
pub struct PlaylistSongsPanel {
    pub list: BrowserSongsList,
    pub route: PlaylistSongsInputRouting,
    pub sort: SortManager,
    pub filter: FilterManager,
    cur_selected: usize,
    pub widget_state: ScrollingTableState,
    pub local_filter_text: String,
    pub cur_playing_video_id: Option<ytmapi_rs::common::VideoID<'static>>,
}
impl_youtui_component!(PlaylistSongsPanel);

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserPlaylistSongsAction {
    Filter,
    Sort,
    PlaySong,
    PlaySongs,
    AddSongToPlaylist,
    AddSongsToPlaylist,
    ViewLyrics,
    CopySongUrl,
    GoToArtist,
    GoToAlbum,
    GetRelatedTracks,
}

impl Action for BrowserPlaylistSongsAction {
    fn context(&self) -> Cow<'_, str> {
        "Playlist Songs Panel".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match &self {
            BrowserPlaylistSongsAction::PlaySong => "Play song",
            BrowserPlaylistSongsAction::PlaySongs => "Play songs",
            BrowserPlaylistSongsAction::AddSongToPlaylist => "Add song to playlist",
            BrowserPlaylistSongsAction::AddSongsToPlaylist => "Add songs to playlist",
            BrowserPlaylistSongsAction::Sort => "Sort",
            BrowserPlaylistSongsAction::Filter => "Filter",
            BrowserPlaylistSongsAction::ViewLyrics => "View Lyrics",
            BrowserPlaylistSongsAction::CopySongUrl => "Copy Song URL",
            BrowserPlaylistSongsAction::GoToArtist => "Go to Artist",
            BrowserPlaylistSongsAction::GoToAlbum => "Go to Album",
            BrowserPlaylistSongsAction::GetRelatedTracks => "Get Related Tracks",
        }
        .into()
    }
}
impl PlaylistSongsPanel {
    pub fn new() -> PlaylistSongsPanel {
        PlaylistSongsPanel {
            cur_selected: Default::default(),
            list: Default::default(),
            route: Default::default(),
            sort: SortManager::new(),
            filter: FilterManager::new(),
            widget_state: Default::default(),
            local_filter_text: String::new(),
            cur_playing_video_id: None,
        }
    }
    pub fn subcolumns_of_vec() -> [ListSongDisplayableField; 5] {
        [
            ListSongDisplayableField::TrackNo,
            ListSongDisplayableField::Artists,
            ListSongDisplayableField::Album,
            ListSongDisplayableField::Song,
            ListSongDisplayableField::Duration,
        ]
    }
    /// Re-apply all sort commands in the stack in the order they were stored.
    pub fn apply_all_sort_commands(&mut self) -> Result<()> {
        for c in self.sort.sort_commands.iter() {
            if !self.get_sortable_columns().contains(&c.column) {
                bail!(format!("Unable to sort column {}", c.column,));
            }
            self.list.sort(
                get_adjusted_list_column(c.column, Self::subcolumns_of_vec())?,
                c.direction,
            );
        }
        Ok(())
    }
    pub fn get_filtered_list_iter(&self) -> impl Iterator<Item = &ListSong> {
        let filter_text = self.local_filter_text.clone();
        self.list.get_list_iter().filter(move |ls| {
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
    pub fn apply_filter(&mut self) {
        self.filter.shown = false;
        self.route = PlaylistSongsInputRouting::List;
        let Some(filter) = self.filter.get_text().map(|s| s.to_string()) else {
            // Do nothing if no filter text
            return;
        };
        let cmd = TableFilterCommand::All(crate::app::view::Filter::Contains(
            FilterString::case_insensitive(filter),
        ));
        let prev_max_cur = self.get_filtered_list_iter().count().saturating_sub(1);
        let prev_cur = self.cur_selected;
        let prev_offset = self.widget_state.offset();
        self.filter.filter_commands.push(cmd);
        // Clamp current selected row to length of list.
        let new_max_cur = self.get_filtered_list_iter().count().saturating_sub(1);
        self.cur_selected = self.cur_selected.min(new_max_cur);
        // Adjust offset accordingly to ensure if list fits on the screen, offset is
        // zero.
        *self.widget_state.offset_mut() = get_offset_after_list_resize(
            prev_offset,
            prev_cur,
            prev_max_cur,
            self.cur_selected,
            new_max_cur,
        );
    }
    pub fn clear_filter(&mut self) {
        self.filter.shown = false;
        self.route = PlaylistSongsInputRouting::List;
        self.filter.filter_commands.clear();
    }
    fn open_sort(&mut self) {
        self.sort.shown = true;
        self.route = PlaylistSongsInputRouting::Sort;
    }
    pub fn toggle_filter(&mut self) {
        let shown = self.filter.shown;
        if !shown {
            // We need to set cur back to 0  and clear text somewhere and I'd prefer to do
            // it at the time of showing, so it cannot be missed.
            self.filter.filter_text.clear();
            self.route = PlaylistSongsInputRouting::Filter;
        } else {
            self.clear_filter_commands();
            self.route = PlaylistSongsInputRouting::List;
        }
        self.filter.shown = !shown;
    }
    pub fn close_sort(&mut self) {
        self.sort.shown = false;
        self.route = PlaylistSongsInputRouting::List;
    }
    pub fn handle_pop_sort(&mut self) {
        // If no sortable columns, should we not handle this command?
        self.sort.cur = 0;
        self.open_sort();
    }
    pub fn handle_clear_sort(&mut self) {
        self.close_sort();
        self.clear_sort_commands();
    }
    pub fn handle_sort_cur_asc(&mut self) {
        // TODO: Better error handling
        let Some(column) = self.get_sortable_columns().get(self.sort.cur) else {
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
        // TODO: Better error handling
        let Some(column) = self.get_sortable_columns().get(self.sort.cur) else {
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
    pub fn get_song_from_idx(&self, idx: usize) -> Option<&ListSong> {
        self.get_filtered_list_iter().nth(idx)
    }

    #[allow(dead_code)]
    pub fn go_to_first(&mut self) {
        match self.route {
            PlaylistSongsInputRouting::List => {
                self.cur_selected = 0;
            }
            PlaylistSongsInputRouting::Sort => {
                self.cur_selected = 0;
            }
            PlaylistSongsInputRouting::Filter => {
                warn!("go_to_first called while in filter mode")
            }
        }
    }

    #[allow(dead_code)]
    pub fn go_to_last(&mut self) {
        match self.route {
            PlaylistSongsInputRouting::List => {
                self.cur_selected = self.get_filtered_list_iter().count().saturating_sub(1);
            }
            PlaylistSongsInputRouting::Sort => {
                self.cur_selected = self.get_sortable_columns().len().saturating_sub(1);
            }
            PlaylistSongsInputRouting::Filter => {
                warn!("go_to_last called while in filter mode")
            }
        }
    }
}
impl SongListComponent for PlaylistSongsPanel {
    fn get_song_from_idx(&self, idx: usize) -> Option<&crate::app::structures::ListSong> {
        self.get_filtered_list_iter().nth(idx)
    }
}
impl TextHandler for PlaylistSongsPanel {
    fn get_text(&self) -> std::option::Option<&str> {
        self.filter.get_text()
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        self.filter.replace_text(text)
    }
    fn is_text_handling(&self) -> bool {
        self.route == PlaylistSongsInputRouting::Filter
    }
    fn clear_text(&mut self) -> bool {
        self.filter.clear_text()
    }
    fn handle_text_event_impl(
        &mut self,
        event: &crossterm::event::Event,
    ) -> Option<ComponentEffect<Self>> {
        self.filter
            .handle_text_event_impl(event)
            .map(|effect| effect.map_frontend(|this: &mut PlaylistSongsPanel| &mut this.filter))
    }
}

impl KeyRouter<AppAction> for PlaylistSongsPanel {
    fn get_all_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        std::iter::once(&config.keybinds.browser_playlist_songs)
    }
    fn get_active_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        match self.route {
            PlaylistSongsInputRouting::List => {
                // TODO: Make unique
                Either::Left(std::iter::once(&config.keybinds.browser_playlist_songs))
            }
            PlaylistSongsInputRouting::Filter => {
                Either::Left(std::iter::once(&config.keybinds.filter))
            }
            PlaylistSongsInputRouting::Sort => Either::Right(get_sort_keybinds(config)),
        }
    }
}

// Is this still relevant?
impl Loadable for PlaylistSongsPanel {
    fn is_loading(&self) -> bool {
        matches!(self.list.state, crate::app::structures::ListStatus::Loading)
    }
}
impl Scrollable for PlaylistSongsPanel {
    fn increment_list(&mut self, amount: isize) {
        if self.sort.shown {
            self.sort.cur = self
                .sort
                .cur
                .saturating_add_signed(amount)
                .min(self.get_sortable_columns().len().saturating_sub(1));
        } else {
            // Naive check using iterator - consider using exact size iterator
            self.cur_selected = self
                .cur_selected
                .saturating_add_signed(amount)
                .min(self.get_filtered_list_iter().count().saturating_sub(1))
        }
    }
    fn is_scrollable(&self) -> bool {
        !self.filter.shown
    }
}

impl TableView for PlaylistSongsPanel {
    fn get_selected_item(&self) -> usize {
        self.cur_selected
    }
    fn get_state(&self) -> &ScrollingTableState {
        &self.widget_state
    }
    fn get_layout(&self) -> &[BasicConstraint] {
        &[
            BasicConstraint::Length(6),
            BasicConstraint::Percentage(Percentage(25)),
            BasicConstraint::Percentage(Percentage(30)),
            BasicConstraint::Percentage(Percentage(45)),
            BasicConstraint::Length(8),
        ]
    }
    fn get_items(&self) -> impl ExactSizeIterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        self.list
            .get_list_iter()
            .map(|ls| ls.get_fields(Self::subcolumns_of_vec()).into_iter())
    }
    fn get_headings(&self) -> impl Iterator<Item = &'static str> {
        ["#", "Artist", "Album", "Song", "Duration"].into_iter()
    }
    fn get_highlighted_row(&self) -> Option<usize> {
        self.cur_playing_video_id.as_ref().and_then(|vid| {
            self.list.get_list_iter().position(|s| s.video_id == *vid)
        })
    }
    fn get_mut_state(&mut self) -> &mut ScrollingTableState {
        &mut self.widget_state
    }
}
impl AdvancedTableView for PlaylistSongsPanel {
    fn get_filtered_count(&self) -> usize {
        // Cheaper than get_filtered_items().count() — no field extraction.
        self.get_filtered_list_iter().count()
    }
    // TODO: Consider if perhaps this table should not be sortable or filterable!
    fn get_sortable_columns(&self) -> &[usize] {
        &[0, 1, 2, 3]
    }
    fn push_sort_command(&mut self, sort_command: TableSortCommand) -> Result<()> {
        // TODO: Maintain a view only struct, for easier rendering of this.
        if !self.get_sortable_columns().contains(&sort_command.column) {
            bail!(format!("Unable to sort column {}", sort_command.column,));
        }
        // Map the column of ArtistAlbums to a column of List and sort
        self.list.sort(
            get_adjusted_list_column(sort_command.column, Self::subcolumns_of_vec())?,
            sort_command.direction,
        );
        // Remove commands that already exist for the same column, as this new command
        // will trump the old ones. Slightly naive - loops the whole vec, could
        // short circuit.
        self.sort
            .sort_commands
            .retain(|cmd| cmd.column != sort_command.column);
        self.sort.sort_commands.push(sort_command);
        Ok(())
    }
    fn clear_sort_commands(&mut self) {
        self.sort.sort_commands.clear();
    }
    fn get_sort_commands(&self) -> &[TableSortCommand] {
        &self.sort.sort_commands
    }
    fn get_filtered_items(&self) -> impl Iterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        // We are doing a lot here every draw cycle!
        self.get_filtered_list_iter()
            .map(|ls| ls.get_fields(Self::subcolumns_of_vec()).into_iter())
    }
    fn get_filterable_columns(&self) -> &[usize] {
        &[1, 2, 3]
    }
    fn get_filter_commands(&self) -> &[TableFilterCommand] {
        &self.filter.filter_commands
    }
    fn clear_filter_commands(&mut self) {
        self.filter.filter_commands.clear()
    }
    fn get_sort_popup_cur(&self) -> usize {
        self.sort.cur
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
    fn get_mut_filter_state(&mut self) -> &mut vi_text_editor::ViTextEditor {
        &mut self.filter.filter_text
    }
}

impl HasTitle for PlaylistSongsPanel {
    fn get_title(&self) -> Cow<'_, str> {
        match self.list.state {
            ListStatus::New => "Songs".into(),
            ListStatus::Loading => "Songs - loading".into(),
            ListStatus::InProgress => format!(
                "Songs - {} results - loading",
                self.list.get_list_iter().len()
            )
            .into(),
            ListStatus::Loaded => {
                format!("Songs - {} results", self.list.get_list_iter().len()).into()
            }
            ListStatus::Error => "Songs - Error receieved".into(),
        }
    }
}
