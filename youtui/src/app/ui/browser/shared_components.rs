use crate::app::component::actionhandler::{Action, ComponentEffect, Suggestable, TextHandler};
use crate::app::server::{GetSearchSuggestions, HandleApiError};
use crate::app::structures::ListSong;
use crate::app::ui::AppCallback;
use crate::app::NavTarget;
use vi_text_editor::ViTextEditor;
use crate::app::view::{TableFilterCommand, TableSortCommand};
use anyhow::Context;
use async_callback_manager::{AsyncTask, Constraint, NoOpHandler};
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};
use ytmapi_rs::common::SearchSuggestion;

#[derive(Default)]
pub struct SearchBlock {
    pub search_contents: ViTextEditor,
    pub search_suggestions: Vec<SearchSuggestion>,
    pub suggestions_cur: Option<usize>,
}
impl_youtui_component!(SearchBlock);

// TODO: refactor
#[derive(Clone, Default)]
pub struct FilterManager {
    pub filter_commands: Vec<TableFilterCommand>,
    pub filter_text: ViTextEditor,
    pub shown: bool,
}
impl_youtui_component!(FilterManager);

// TODO: refactor
#[derive(Clone, Default)]
pub struct SortManager {
    pub sort_commands: Vec<TableSortCommand>,
    pub shown: bool,
    pub cur: usize,
    pub state: ListState,
}
impl_youtui_component!(SortManager);

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterAction {
    Close,
    ClearFilter,
    Apply,
}

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortAction {
    Close,
    ClearSort,
    SortSelectedAsc,
    SortSelectedDesc,
}

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserSearchAction {
    PrevSearchSuggestion,
    NextSearchSuggestion,
    Close,
}

impl Action for FilterAction {
    fn context(&self) -> std::borrow::Cow<'_, str> {
        "Filter".into()
    }
    fn describe(&self) -> std::borrow::Cow<'_, str> {
        match self {
            FilterAction::Close => "Close Filter",
            FilterAction::Apply => "Apply filter",
            FilterAction::ClearFilter => "Clear filter",
        }
        .into()
    }
}

impl Action for SortAction {
    fn context(&self) -> std::borrow::Cow<'_, str> {
        "Sort".into()
    }
    fn describe(&self) -> std::borrow::Cow<'_, str> {
        match self {
            SortAction::Close => "Close sort",
            SortAction::ClearSort => "Clear sort",
            SortAction::SortSelectedAsc => "Sort ascending",
            SortAction::SortSelectedDesc => "Sort descending",
        }
        .into()
    }
}

impl Action for BrowserSearchAction {
    fn context(&self) -> std::borrow::Cow<'_, str> {
        "Browser Search Panel".into()
    }
    fn describe(&self) -> std::borrow::Cow<'_, str> {
        match self {
            BrowserSearchAction::PrevSearchSuggestion => "Prev Search Suggestion",
            BrowserSearchAction::NextSearchSuggestion => "Next Search Suggestion",
            BrowserSearchAction::Close => "Close Search",
        }
        .into()
    }
}

impl SortManager {
    pub fn new() -> Self {
        SortManager {
            sort_commands: Default::default(),
            shown: Default::default(),
            cur: Default::default(),
            state: Default::default(),
        }
    }
}
impl FilterManager {
    pub fn new() -> Self {
        Self {
            filter_text: Default::default(),
            filter_commands: Default::default(),
            shown: Default::default(),
        }
    }
}
impl TextHandler for FilterManager {
    fn is_text_handling(&self) -> bool {
        true
    }
    fn get_text(&self) -> std::option::Option<&str> {
        Some(self.filter_text.get_text())
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        self.filter_text.set_text(&text.into())
    }
    fn clear_text(&mut self) -> bool {
        self.filter_text.clear();
        true
    }
    fn handle_text_event_impl(
        &mut self,
        event: &crossterm::event::Event,
    ) -> Option<ComponentEffect<Self>> {
        if let crossterm::event::Event::Key(key) = event {
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                return None;
            }
            // Let '3' (Close Filter), Enter (Apply Filter), Esc reach keybind dispatch
            use crossterm::event::KeyCode;
            if matches!(key.code, KeyCode::Char('3') | KeyCode::Enter | KeyCode::Esc) {
                return None;
            }
            use vi_text_editor::ViMode;
            if self.filter_text.mode == ViMode::Normal
                && matches!(key.code, KeyCode::Char('j')
                    | KeyCode::Char('k')
                    | KeyCode::Up
                    | KeyCode::Down)
            {
                return None;
            }
            self.filter_text.handle_key(key.code, false, false);
        }
        Some(AsyncTask::new_no_op())
    }
}

impl TextHandler for SearchBlock {
    fn is_text_handling(&self) -> bool {
        true
    }
    fn get_text(&self) -> std::option::Option<&str> {
        Some(self.search_contents.get_text())
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        self.search_contents.set_text(&text.into());
    }
    fn clear_text(&mut self) -> bool {
        self.search_suggestions.clear();
        self.search_contents.clear();
        true
    }
    fn handle_text_event_impl(
        &mut self,
        event: &crossterm::event::Event,
    ) -> Option<ComponentEffect<Self>> {
        if let crossterm::event::Event::Key(key) = event {
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                return None;
            }
            use vi_text_editor::ViMode;
            // Check mode BEFORE handling the key
            let was_normal = self.search_contents.mode == ViMode::Normal;
            let changed = self.search_contents.handle_key(key.code, false, false);
            if changed {
                return None;
            }
            // Esc in Normal mode closes search.
            // Esc in Insert mode switches to Normal (handled by ViTextEditor).
            if key.code == crossterm::event::KeyCode::Esc && was_normal {
                return None;
            }
            if matches!(key.code, crossterm::event::KeyCode::Char(_) | crossterm::event::KeyCode::Backspace) {
                return Some(self.fetch_search_suggestions());
            }
        }
        Some(AsyncTask::new_no_op())
    }
}

impl Suggestable for SearchBlock {
    fn get_search_suggestions(&self) -> &[SearchSuggestion] {
        self.search_suggestions.as_slice()
    }
    fn has_search_suggestions(&self) -> bool {
        !self.search_suggestions.is_empty()
    }
}

impl SearchBlock {
    pub fn delete_word(&mut self) {
        let text = self.search_contents.get_text().to_string();
        if !text.is_empty() {
            let pos = text[..self.search_contents.cursor].rfind(' ').unwrap_or(0);
            let new_text = text[..pos].to_string();
            self.search_contents.set_text(&new_text);
            self.search_contents.cursor = new_text.len();
        }
    }

    // Ask the UI for search suggestions for the current query
    fn fetch_search_suggestions(&mut self) -> ComponentEffect<Self> {
        let text = self.search_contents.get_text().to_owned();
        if text.is_empty() {
            self.search_suggestions.clear();
            return AsyncTask::new_no_op();
        }
        AsyncTask::new_future_try(
            GetSearchSuggestions(text),
            HandleSearchSuggestionsOk,
            HandleSearchSuggestionsErr,
            Some(Constraint::new_kill_same_type()),
        )
    }
    fn replace_search_suggestions(
        &mut self,
        search_suggestions: Vec<SearchSuggestion>,
        search: String,
    ) {
        if self.get_text() == Some(&search) {
            self.search_suggestions = search_suggestions;
            self.suggestions_cur = None;
        }
    }
    pub fn increment_list(&mut self, amount: isize) {
        if !self.search_suggestions.is_empty() {
            self.suggestions_cur = Some(
                self.suggestions_cur
                    .map(|cur| {
                        cur.saturating_add_signed(amount)
                            .min(self.search_suggestions.len() - 1)
                    })
                    .unwrap_or_default(),
            );
            self.replace_text(
                self.search_suggestions[self.suggestions_cur.expect("Set to non-None value above")]
                    .get_text(),
            );
        }
    }
}

#[derive(PartialEq, Debug)]
struct HandleSearchSuggestionsOk;
#[derive(PartialEq, Debug)]
struct HandleSearchSuggestionsErr;
impl_youtui_task_handler!(
    HandleSearchSuggestionsOk,
    (Vec<SearchSuggestion>, String),
    SearchBlock,
    |_, (suggestions, text)| |this: &mut SearchBlock| this
        .replace_search_suggestions(suggestions, text)
);
impl_youtui_task_handler!(
    HandleSearchSuggestionsErr,
    anyhow::Error,
    SearchBlock,
    |_, error| |_: &mut SearchBlock| AsyncTask::new_future(
        HandleApiError {
            error,
            // To avoid needing to clone search query to use in the error message, this
            // error message is minimal.
            message: "Error recieved getting search suggestions".to_string(),
        },
        NoOpHandler,
        None,
    )
);

/// A table may display columns in a different order, adjust the index to a new
/// index based on a list of correct indexes.
pub fn get_adjusted_list_column<T: Copy, const N: usize>(
    target_col: usize,
    adjusted_cols: [T; N],
) -> anyhow::Result<T> {
    adjusted_cols
        .get(target_col)
        .with_context(|| {
            format!("Unable to sort column, doesn't match up with underlying list. {target_col}",)
        })
        .copied()
}

pub fn navigate_to_artist(song: &ListSong) -> Option<AppCallback> {
    let artist = song.artists.iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Some(AppCallback::Navigate(NavTarget::Artist(artist)))
}

pub fn navigate_to_album(song: &ListSong) -> Option<AppCallback> {
    let artist = song.artists.iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    song.album.as_ref().map(|album| {
        AppCallback::Navigate(NavTarget::Album { artist, album: album.name.clone() })
    })
}

#[cfg(test)]
mod tests {
    use crate::app::component::actionhandler::TextHandler;
    use crate::app::server::GetSearchSuggestions;
    use crate::app::ui::browser::shared_components::{
        HandleSearchSuggestionsErr, HandleSearchSuggestionsOk, SearchBlock,
        get_adjusted_list_column,
    };
    use async_callback_manager::{AsyncTask, Constraint};
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_get_adjusted_list_column() {
        assert_eq!(get_adjusted_list_column(2, [3, 1, 2]).unwrap(), 2);
        assert_eq!(get_adjusted_list_column(0, [3, 1, 2]).unwrap(), 3);
        assert_eq!(get_adjusted_list_column(1, [3, 1, 2]).unwrap(), 1);
    }
    #[test]
    fn test_get_adjusted_list_column_out_of_bounds() {
        assert!(get_adjusted_list_column(3, [3, 1, 2]).is_err())
    }
    #[test]
    fn test_dont_fetch_search_suggestions_when_empty() {
        let mut b = SearchBlock::default();
        let effect = b.fetch_search_suggestions();
        assert!(effect.is_no_op());
    }
    #[test]
    fn test_search_suggestions_fetch_effect() {
        let mut b = SearchBlock::default();
        b.search_contents.set_text("The beatles");
        let effect = b.fetch_search_suggestions();
        let expected_effect = AsyncTask::new_future_try(
            GetSearchSuggestions("The beatles".to_string()),
            HandleSearchSuggestionsOk,
            HandleSearchSuggestionsErr,
            Some(Constraint::new_kill_same_type()),
        );
        assert_eq!(effect, expected_effect);
    }
    #[test]
    fn test_search_suggestions_fetched_on_change() {
        let mut b = SearchBlock::default();
        let effect = b
            .try_handle_text(&crossterm::event::Event::Key(
                crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char('A'),
                    KeyModifiers::empty(),
                ),
            ))
            .unwrap();
        let expected_effect = AsyncTask::new_future_try(
            GetSearchSuggestions("A".to_string()),
            HandleSearchSuggestionsOk,
            HandleSearchSuggestionsErr,
            Some(Constraint::new_kill_same_type()),
        );
        assert_eq!(effect, expected_effect)
    }
}
