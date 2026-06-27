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
use std::iter::Iterator;
use tracing::warn;
use ytmapi_rs::common::Thumbnail;
use ytmapi_rs::parse::{AlbumSong, ParsedSongAlbum, ParsedSongArtist};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum AlbumSongsInputRouting {
    #[default]
    List,
    Sort,
    Filter,
}

#[derive(Clone)]
pub struct AlbumSongsPanel {
    pub list: BrowserSongsList,
    view_indices: Vec<usize>,
    pub route: AlbumSongsInputRouting,
    pub sort: SortManager,
    pub filter: FilterManager,
    cur_selected: usize,
    pub widget_state: ScrollingTableState,
    pub category_filter: Option<&'static str>,
    filtered_cache: Vec<ListSong>,
    pub local_filter_text: String,
    pub cur_playing_video_id: Option<ytmapi_rs::common::VideoID<'static>>,
}
impl_youtui_component!(AlbumSongsPanel);

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserArtistSongsAction {
    Filter,
    Sort,
    PlaySong,
    PlaySongs,
    PlayAlbum,
    AddSongToPlaylist,
    AddSongsToPlaylist,
    AddAlbumToPlaylist,
    ViewLyrics,
    CopySongUrl,
    ToggleCategoryFilter,
    GoToArtist,
    GoToAlbum,
    GetRelatedTracks,
}

impl Action for BrowserArtistSongsAction {
    fn context(&self) -> std::borrow::Cow<'_, str> {
        "Artist Songs Panel".into()
    }
    fn describe(&self) -> std::borrow::Cow<'_, str> {
        match &self {
            BrowserArtistSongsAction::PlaySong => "Play song",
            BrowserArtistSongsAction::PlaySongs => "Play songs",
            BrowserArtistSongsAction::PlayAlbum => "Play album",
            BrowserArtistSongsAction::AddSongToPlaylist => "Add song to playlist",
            BrowserArtistSongsAction::AddSongsToPlaylist => "Add songs to playlist",
            BrowserArtistSongsAction::AddAlbumToPlaylist => "Add album to playlist",
            BrowserArtistSongsAction::Sort => "Sort",
            BrowserArtistSongsAction::Filter => "Filter",
            BrowserArtistSongsAction::ViewLyrics => "View Lyrics",
            BrowserArtistSongsAction::CopySongUrl => "Copy Song URL",
            BrowserArtistSongsAction::ToggleCategoryFilter => "Toggle Category Filter",
            BrowserArtistSongsAction::GoToArtist => "Go to Artist",
            BrowserArtistSongsAction::GoToAlbum => "Go to Album",
            BrowserArtistSongsAction::GetRelatedTracks => "Get Related Tracks",
        }
        .into()
    }
}
impl AlbumSongsPanel {
    pub fn new() -> AlbumSongsPanel {
        AlbumSongsPanel {
            cur_selected: Default::default(),
            list: Default::default(),
            view_indices: Vec::new(),
            route: Default::default(),
            sort: SortManager::new(),
            filter: FilterManager::new(),
            widget_state: Default::default(),
            category_filter: None,
            filtered_cache: Vec::new(),
            local_filter_text: String::new(),
            cur_playing_video_id: None,
        }
    }
    pub fn subcolumns_of_vec() -> [ListSongDisplayableField; 6] {
        [
            ListSongDisplayableField::TrackNo,
            ListSongDisplayableField::Album,
            ListSongDisplayableField::Song,
            ListSongDisplayableField::Duration,
            ListSongDisplayableField::Year,
            ListSongDisplayableField::LikeStatus,
        ]
    }
    /// Re-apply all sort commands in the stack in the order they were stored.
    pub fn apply_all_sort_commands(&mut self) -> Result<()> {
        for c in self.sort.sort_commands.iter() {
            if !self.get_sortable_columns().contains(&c.column) {
                bail!(format!("Unable to sort column {}", c.column,));
            }
            let field = get_adjusted_list_column(c.column, Self::subcolumns_of_vec())?;
            self.view_indices.sort_by(|&a, &b| {
                let a_val = self
                    .list
                    .get_song_from_idx(a)
                    .map(|s| s.get_field(field))
                    .unwrap_or_default();
                let b_val = self
                    .list
                    .get_song_from_idx(b)
                    .map(|s| s.get_field(field))
                    .unwrap_or_default();
                match c.direction {
                    SortDirection::Asc => a_val.partial_cmp(&b_val),
                    SortDirection::Desc => b_val.partial_cmp(&a_val),
                }
                .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        Ok(())
    }
    pub fn get_filtered_list_iter(&self) -> impl Iterator<Item = &ListSong> + '_ {
        let filter_text = &self.local_filter_text;
        self.view_indices.iter().filter_map(move |&idx| {
            let Some(ls) = self.list.get_song_from_idx(idx) else {
                return None;
            };
            if let Some(cat) = self.category_filter {
                let album_name = ls.album.as_ref().map(|a| a.name.as_str()).unwrap_or("");
                if !album_name.starts_with(cat) {
                    return None;
                }
            }
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
            if !fuzzy_pass { return None; }
            let pass = self.get_filter_commands()
                .iter()
                .fold(true, |acc, command| {
                    let match_found = command.matches_row(
                        ls,
                        Self::subcolumns_of_vec(),
                        self.get_filterable_columns(),
                    );
                    acc && match_found
                });
            if pass { Some(ls) } else { None }
        })
    }
    pub fn apply_filter(&mut self) {
        self.filter.shown = false;
        self.route = AlbumSongsInputRouting::List;
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
        self.route = AlbumSongsInputRouting::List;
        self.filter.filter_commands.clear();
    }
    fn open_sort(&mut self) {
        self.sort.shown = true;
        self.route = AlbumSongsInputRouting::Sort;
    }
    pub fn toggle_filter(&mut self) {
        let shown = self.filter.shown;
        if !shown {
            // We need to set cur back to 0  and clear text somewhere and I'd prefer to do
            // it at the time of showing, so it cannot be missed.
            self.filter.filter_text.clear();
            self.route = AlbumSongsInputRouting::Filter;
        } else {
            self.clear_filter_commands();
            self.route = AlbumSongsInputRouting::List;
        }
        self.filter.shown = !shown;
    }
    pub fn close_sort(&mut self) {
        self.sort.shown = false;
        self.route = AlbumSongsInputRouting::List;
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
    pub fn handle_toggle_category_filter(&mut self) {
        self.category_filter = match self.category_filter {
            None => Some("Album:"),
            Some("Album:") => Some("EP:"),
            Some("EP:") => Some("Single:"),
            _ => None,
        };
        self.rebuild_filtered_cache();
        self.cur_selected = self.cur_selected.min(self.filtered_cache.len().saturating_sub(1));
    }
    pub fn rebuild_filtered_cache(&mut self) {
        self.filtered_cache = self.get_filtered_list_iter().cloned().collect();
    }
    pub fn clear_songs(&mut self) {
        self.list.clear();
        self.view_indices.clear();
    }
    pub fn append_album_songs(
        &mut self,
        song_list: Vec<AlbumSong>,
        album: ParsedSongAlbum,
        year: String,
        artists: Vec<ParsedSongArtist>,
        thumbnails: Vec<Thumbnail>,
    ) {
        let old_len = self.list.len();
        self.list
            .append_raw_album_songs(song_list, album, year, artists, thumbnails);
        self.view_indices.extend(old_len..self.list.len());
    }
    pub fn handle_songs_found(&mut self) {
        self.clear_songs();
        // XXX: Consider clearing sort params here, so that we don't need to sort all
        // the incoming songs. Performance seems OK for now. XXX: Consider also
        // clearing filter params here.
        self.cur_selected = 0;
        self.list.state = ListStatus::InProgress;
    }
    pub fn get_song_from_idx(&self, idx: usize) -> Option<&ListSong> {
        self.get_filtered_list_iter().nth(idx)
    }

    pub fn go_to_first(&mut self) {
        match self.route {
            AlbumSongsInputRouting::List => {
                self.cur_selected = 0;
            }
            AlbumSongsInputRouting::Sort => {
                self.cur_selected = 0;
            }
            AlbumSongsInputRouting::Filter => {
                warn!("go_to_first called while in filter mode")
            }
        }
    }

    pub fn go_to_last(&mut self) {
        match self.route {
            AlbumSongsInputRouting::List => {
                self.cur_selected = self.get_filtered_list_iter().count().saturating_sub(1);
            }
            AlbumSongsInputRouting::Sort => {
                self.cur_selected = self.get_sortable_columns().len().saturating_sub(1);
            }
            AlbumSongsInputRouting::Filter => {
                warn!("go_to_last called while in filter mode")
            }
        }
    }
}
impl SongListComponent for AlbumSongsPanel {
    fn get_song_from_idx(&self, idx: usize) -> Option<&crate::app::structures::ListSong> {
        self.get_filtered_list_iter().nth(idx)
    }
}
impl TextHandler for AlbumSongsPanel {
    fn get_text(&self) -> std::option::Option<&str> {
        self.filter.get_text()
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        self.filter.replace_text(text)
    }
    fn is_text_handling(&self) -> bool {
        self.route == AlbumSongsInputRouting::Filter
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
            .map(|effect| effect.map_frontend(|this: &mut AlbumSongsPanel| &mut this.filter))
    }
}

impl KeyRouter<AppAction> for AlbumSongsPanel {
    fn get_all_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        std::iter::once(&config.keybinds.browser_artist_songs)
    }
    fn get_active_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        match self.route {
            AlbumSongsInputRouting::List => {
                Either::Left(std::iter::once(&config.keybinds.browser_artist_songs))
            }
            AlbumSongsInputRouting::Filter => {
                Either::Left(std::iter::once(&config.keybinds.filter))
            }
            AlbumSongsInputRouting::Sort => Either::Right(get_sort_keybinds(config)),
        }
    }
}

// Is this still relevant?
impl Loadable for AlbumSongsPanel {
    fn is_loading(&self) -> bool {
        matches!(self.list.state, crate::app::structures::ListStatus::Loading)
    }
}
impl Scrollable for AlbumSongsPanel {
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

impl TableView for AlbumSongsPanel {
    fn get_selected_item(&self) -> usize {
        self.cur_selected
    }
    fn get_state(&self) -> &ScrollingTableState {
        &self.widget_state
    }
    fn get_layout(&self) -> &[BasicConstraint] {
        &[
            BasicConstraint::Length(6),
            BasicConstraint::Percentage(Percentage(50)),
            BasicConstraint::Percentage(Percentage(50)),
            BasicConstraint::Length(8),
            BasicConstraint::Length(5),
            BasicConstraint::Length(4),
        ]
    }
    fn get_items(&self) -> impl ExactSizeIterator<Item = impl Iterator<Item = Cow<'_, str>> + '_> {
        self.filtered_cache
            .iter()
            .map(|ls| ls.get_fields(Self::subcolumns_of_vec()).into_iter())
    }
    fn get_headings(&self) -> impl Iterator<Item = &'static str> {
        ["#", "Album", "Song", "Duration", "Year", "Liked"].into_iter()
    }
    fn get_highlighted_row(&self) -> Option<usize> {
        let vid = self.cur_playing_video_id.as_ref()?;
        self.filtered_cache.iter().position(|s| s.video_id == *vid)
    }
    fn get_mut_state(&mut self) -> &mut ScrollingTableState {
        &mut self.widget_state
    }
}
impl AdvancedTableView for AlbumSongsPanel {
    fn get_filtered_count(&self) -> usize {
        // Cheaper than get_filtered_items().count() - no field extraction.
        self.get_filtered_list_iter().count()
    }
    fn get_sortable_columns(&self) -> &[usize] {
        &[1, 4, 5]
    }
    fn push_sort_command(&mut self, sort_command: TableSortCommand) -> Result<()> {
        if !self.get_sortable_columns().contains(&sort_command.column) {
            bail!(format!("Unable to sort column {}", sort_command.column,));
        }
        let field = get_adjusted_list_column(sort_command.column, Self::subcolumns_of_vec())?;
        // Sort view indices instead of the underlying list - preserves original order.
        self.view_indices.sort_by(|&a, &b| {
            let a_val = self
                .list
                .get_song_from_idx(a)
                .map(|s| s.get_field(field))
                .unwrap_or_default();
            let b_val = self
                .list
                .get_song_from_idx(b)
                .map(|s| s.get_field(field))
                .unwrap_or_default();
            match sort_command.direction {
                SortDirection::Asc => a_val.partial_cmp(&b_val),
                SortDirection::Desc => b_val.partial_cmp(&a_val),
            }
            .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Remove commands that already exist for the same column, as this new command
        // will trump the old ones. Slightly naive - loops the whole vec, could
        // short circuit.
        self.sort
            .sort_commands
            .retain(|cmd| cmd.column != sort_command.column);
        self.sort.sort_commands.push(sort_command);
        self.rebuild_filtered_cache();
        Ok(())
    }
    fn clear_sort_commands(&mut self) {
        self.sort.sort_commands.clear();
        self.view_indices = (0..self.list.len()).collect();
        self.rebuild_filtered_cache();
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
        &[1, 2, 4]
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
impl HasTitle for AlbumSongsPanel {
    fn get_title(&self) -> Cow<'_, str> {
        match self.list.state {
            ListStatus::New => "Songs".into(),
            ListStatus::Loading => "Songs - loading".into(),
            ListStatus::InProgress => format!(
                "Songs - {} results - loading",
                self.list.len()
            )
            .into(),
            ListStatus::Loaded => {
                let cat_indicator = match self.category_filter {
                    Some("Album:") => " [Albums]",
                    Some("EP:") => " [EPs]",
                    Some("Single:") => " [Singles]",
                    _ => "",
                };
                format!("Songs - {} results{}", self.list.len(), cat_indicator).into()
            }
            ListStatus::Error => "Songs - Error receieved".into(),
        }
    }
}
