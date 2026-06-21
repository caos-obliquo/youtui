/// Traits related to viewable application components.
use super::structures::{ListSong, ListSongDisplayableField, Percentage};
use crate::widgets::{ScrollingListState, ScrollingTableState};
use vi_text_editor::ViTextEditor;
use ratatui::Frame;
use ratatui::prelude::{Constraint, Rect};
use ratatui::widgets::ListState;
use std::borrow::Cow;

pub mod draw;

#[derive(Clone, Debug)]
pub struct TableSortCommand {
    pub column: usize,
    pub direction: SortDirection,
}

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Clone, Debug)]
pub enum TableFilterCommand {
    All(Filter),
}
#[derive(Clone, Debug)]
pub enum Filter {
    Contains(FilterString),
}
#[derive(Clone, Debug)]
pub enum FilterString {
    CaseInsensitive { original: String, lowercased: String },
}

impl TableFilterCommand {
    fn as_readable(&self) -> String {
        match self {
            TableFilterCommand::All(f) => format!("ALL{}", f.as_readable()),
        }
    }
    pub fn matches_row<const N: usize>(
        &self,
        row: &ListSong,
        fields_in_table: [ListSongDisplayableField; N],
        filterable_colums: &[usize],
    ) -> bool {
        match self {
            TableFilterCommand::All(filter) => match filter {
                Filter::Contains(filter_string) => filterable_colums
                    .iter()
                    .any(|col| filter_string.is_in(row.get_field(fields_in_table[*col]).as_ref())),
            },
        }
    }
}
impl Filter {
    fn as_readable(&self) -> String {
        match self {
            Filter::Contains(f) => format!("~{}", f.as_readable()),
        }
    }
}
impl FilterString {
    /// Constructs a CaseInsensitive filter, pre-computing the lowercased form
    /// so it is not recomputed on every call to is_in/is_equal.
    pub fn case_insensitive(s: String) -> Self {
        let lowercased = s.to_ascii_lowercase();
        Self::CaseInsensitive {
            original: s,
            lowercased,
        }
    }
    fn as_readable(&self) -> String {
        match self {
            FilterString::CaseInsensitive { original, .. } => format!("a=A:{original}"),
        }
    }
    pub fn is_in<S: AsRef<str>>(&self, test_str: S) -> bool {
        match self {
            FilterString::CaseInsensitive { lowercased, .. } => test_str
                .as_ref()
                .to_ascii_lowercase()
                .contains(lowercased.as_str()),
        }
    }
}

/// Basic wrapper around constraint to allow mixing of percentage and length.
pub enum BasicConstraint {
    Length(u16),
    Percentage(Percentage),
}

// TODO: Add more tests
/// Use basic constraints to construct dynamic column widths for a table.
pub fn basic_constraints_to_table_constraints(
    basic_constraints: &[BasicConstraint],
    length: u16,
    margin: u16,
) -> Vec<Constraint> {
    let sum_lengths = basic_constraints.iter().fold(0, |acc, c| {
        acc + match c {
            BasicConstraint::Length(l) => *l,
            BasicConstraint::Percentage(_) => 0,
        } + margin
    });
    basic_constraints
        .iter()
        .map(|bc| match bc {
            BasicConstraint::Length(l) => Constraint::Length(*l),
            BasicConstraint::Percentage(p) => {
                Constraint::Length(p.0 as u16 * length.saturating_sub(sum_lengths) / 100)
            }
        })
        .collect()
}

/// A struct that we are able to draw a table from using the underlying data.
pub trait TableView {
    fn get_state(&self) -> &ScrollingTableState;
    fn get_mut_state(&mut self) -> &mut ScrollingTableState;
    /// An item will always be selected.
    fn get_selected_item(&self) -> usize;
    fn get_layout(&self) -> &[BasicConstraint];
    // A row can be highlighted.
    fn get_highlighted_row(&self) -> Option<usize>;
    fn get_items(&self) -> impl ExactSizeIterator<Item = impl Iterator<Item = Cow<'_, str>> + '_>;
    fn get_headings(&self) -> impl Iterator<Item = &'static str>;
    /// Visual selection range for vim-style visual mode: (start, end) inclusive.
    /// Rows in this range get visual_range_style applied.
    fn get_visual_range(&self) -> Option<(usize, usize)> { None }
}
/// TableView with built in filtering and sorting.
pub trait AdvancedTableView: TableView {
    fn get_mut_filter_state(&mut self) -> &mut ViTextEditor;
    fn filter_popup_shown(&self) -> bool;
    fn get_filterable_columns(&self) -> &[usize];
    // This can't be ExactSized as return type may be Filter<T>
    fn get_filtered_items(&self) -> impl Iterator<Item = impl Iterator<Item = Cow<'_, str>> + '_>;
    /// Returns the number of rows after filtering. Override this in impls that
    /// have a cheaper path (e.g. iterating &ListSong without field extraction).
    fn get_filtered_count(&self) -> usize {
        self.get_filtered_items().count()
    }
    fn get_filter_commands(&self) -> &[TableFilterCommand];
    fn clear_filter_commands(&mut self);
    // SortableTableView should maintain it's own popup state.
    fn get_sort_popup_cur(&self) -> usize;
    fn sort_popup_shown(&self) -> bool;
    fn get_sort_state(&self) -> &ListState;
    fn get_mut_sort_state(&mut self) -> &mut ListState;
    /// Add a new TableSortCommand and sort the table.
    /// This can fail if the TableSortCommand is not within the range of
    /// sortable columns.
    fn push_sort_command(&mut self, sort_command: TableSortCommand) -> anyhow::Result<()>;
    fn clear_sort_commands(&mut self);
    fn get_sortable_columns(&self) -> &[usize];
    fn get_sort_commands(&self) -> &[TableSortCommand];
}
// A struct that we are able to draw a list from using the underlying data.
pub trait ListView {
    /// An item will always be selected.
    fn get_selected_item(&self) -> usize;
    fn get_state(&self) -> &ScrollingListState;
    fn get_mut_state(&mut self) -> &mut ScrollingListState;
    fn get_items(&self) -> impl ExactSizeIterator<Item = Cow<'_, str>> + '_;
    fn len(&self) -> usize {
        self.get_items().len()
    }
}
// A drawable part of the application.
pub trait Drawable {
    // Helper function to draw.
    fn draw_chunk(&self, f: &mut Frame, chunk: Rect, selected: bool);
}
// A drawable part of the application that mutates its state on draw.
pub trait DrawableMut {
    // Helper function to draw.
    // TODO: Clean up function signature regarding mutable state.
    fn draw_mut_chunk(&mut self, f: &mut Frame, chunk: Rect, selected: bool, cur_tick: u64);
}
// A part of the application that can be in a Loading state.
pub trait Loadable {
    fn is_loading(&self) -> bool;
}
// A part of the application that has a title
pub trait HasTitle {
    fn get_title(&self) -> Cow<'_, str>;
}
// A part of the application that has a tabbed interface.
pub trait HasTabs {
    fn tabs_block_title(&'_ self) -> Cow<'_, str>;
    fn tab_items(&'_ self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> + '_;
    fn selected_tab_idx(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::{BasicConstraint, basic_constraints_to_table_constraints};
    use crate::app::structures::Percentage;
    use ratatui::prelude::Constraint;

    #[test]
    fn test_constraints() {
        let basic_constraints = &[
            BasicConstraint::Length(5),
            BasicConstraint::Length(5),
            BasicConstraint::Percentage(Percentage(100)),
        ];
        let constraints = vec![
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(10),
        ];
        let converted = basic_constraints_to_table_constraints(basic_constraints, 20, 0);
        assert_eq!(converted, constraints);
        let basic_constraints = &[
            BasicConstraint::Length(5),
            BasicConstraint::Length(5),
            BasicConstraint::Percentage(Percentage(50)),
            BasicConstraint::Percentage(Percentage(50)),
        ];
        let constraints = vec![
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ];
        let converted = basic_constraints_to_table_constraints(basic_constraints, 20, 0);
        assert_eq!(converted, constraints);
    }
}
