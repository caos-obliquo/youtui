use crate::app::component::actionhandler::{ComponentEffect, Action, ActionHandler, YoutuiEffect};
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::{KeyCode, KeyModifiers};
use crate::app::structures::Percentage;
use crate::app::view::{BasicConstraint, basic_constraints_to_table_constraints};
use crate::drawutils::{ROW_HIGHLIGHT_COLOUR, SELECTED_BORDER_COLOUR, TABLE_HEADINGS_COLOUR, TEXT_COLOUR};
use crate::widgets::{ScrollingTable, ScrollingTableState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph};
use ratatui::Frame;
use std::borrow::Cow;
use tracing::{debug, info, warn};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RecommendationsAction {
    Close,
}

impl Action for RecommendationsAction {
    fn context(&self) -> Cow<'_, str> {
        "Recommendations".into()
    }
    fn describe(&self) -> Cow<'_, str> {
        match self {
            RecommendationsAction::Close => "Close",
        }
        .into()
    }
}

/// Context menu items; index matches `menu_selected`.
const MENU_ITEMS: &[&str] = &["Play", "Add to Queue", "Copy URL"];

pub struct RecommendationsPopup {
    pub kind: crate::lastfm_recommend::RecKind,
    pub kind_filter: Option<crate::lastfm_recommend::RecKind>,
    pub items: Vec<crate::lastfm_recommend::RecItem>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub loading: bool,
    pub filter: String,
    pub filter_active: bool,
    pub table_state: ScrollingTableState,
    pub tick: u64,
    pub menu_open: bool,
    pub menu_selected: usize,
}

impl_youtui_component!(RecommendationsPopup);

impl ActionHandler<RecommendationsAction> for RecommendationsPopup {
    fn apply_action(&mut self, action: RecommendationsAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            RecommendationsAction::Close => (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup)),
        }
    }
}

impl RecommendationsPopup {
    pub fn new(kind: crate::lastfm_recommend::RecKind, loading: bool) -> Self {
        info!("Opening recommendations popup: kind={:?} loading={}", kind, loading);
        Self {
            kind,
            kind_filter: None,
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            loading,
            filter: String::new(),
            filter_active: false,
            table_state: ScrollingTableState::default(),
            tick: 0,
            menu_open: false,
            menu_selected: 0,
        }
    }

    fn visible_items(&self) -> Vec<&crate::lastfm_recommend::RecItem> {
        let kind_match = |i: &&crate::lastfm_recommend::RecItem| match self.kind_filter {
            Some(k) => i.kind == k,
            None => true,
        };
        if self.filter_active && !self.filter.is_empty() {
            let f = self.filter.to_lowercase();
            self.items
                .iter()
                .filter(|i| {
                    kind_match(i)
                        && (i.title.to_lowercase().contains(&f)
                            || i.artist.to_lowercase().contains(&f)
                            || i.reason.clone().unwrap_or_default().to_lowercase().contains(&f))
                })
                .collect()
        } else {
            self.items.iter().filter(kind_match).collect()
        }
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        if self.menu_open {
            return self.handle_menu_key(event);
        }
        match event.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                info!("Closing recommendations popup: kind={:?} items={}", self.kind, self.items.len());
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let n = self.visible_items().len();
                if n > 0 {
                    self.selected = (self.selected + 1) % n;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let n = self.visible_items().len();
                if n > 0 {
                    self.selected = if self.selected == 0 { n - 1 } else { self.selected - 1 };
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('g') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.selected = 0;
                self.scroll_offset = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('G') => {
                let n = self.visible_items().len();
                if n > 0 {
                    self.selected = n - 1;
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('/') => {
                self.filter_active = !self.filter_active;
                if !self.filter_active {
                    self.filter.clear();
                    self.selected = 0;
                }
                debug!("Recommendations filter toggled: active={} items={}", self.filter_active, self.items.len());
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Backspace if self.filter_active => {
                self.filter.pop();
                self.selected = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char(c) if self.filter_active => {
                self.filter.push(c);
                self.selected = 0;
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Tab => {
                use crate::lastfm_recommend::RecKind;
                self.kind_filter = match self.kind_filter {
                    None => Some(RecKind::Artists),
                    Some(RecKind::Artists) => Some(RecKind::Albums),
                    Some(RecKind::Albums) => Some(RecKind::Tracks),
                    Some(RecKind::Tracks) => None,
                };
                self.selected = 0;
                self.scroll_offset = 0;
                info!("Recommendations kind filter: {:?}", self.kind_filter);
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('o') => {
                self.menu_open = true;
                self.menu_selected = 0;
                info!("Recommendations context menu opened");
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Enter => {
                let visible = self.visible_items();
                if visible.is_empty() {
                    return (AsyncTask::new_no_op(), None);
                }
                let idx = self.selected.min(visible.len() - 1);
                let item = visible[idx];
                let kind = item.kind;
                let title = item.title.clone();
                let artist = item.artist.clone();
                info!(
                    "Acting on recommendation: idx={} kind={:?} title={} artist={}",
                    idx, kind, title, artist
                );
                (
                    AsyncTask::new_no_op(),
                    Some(AppCallback::ActOnRecommendation(idx, kind, title, artist)),
                )
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    fn handle_menu_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.menu_selected = (self.menu_selected + 1) % MENU_ITEMS.len();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.menu_selected = (self.menu_selected + MENU_ITEMS.len() - 1) % MENU_ITEMS.len();
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Enter | KeyCode::Char('o') | KeyCode::Char(' ') => {
                self.menu_open = false;
                self.activate_menu_item()
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.menu_open = false;
                info!("Recommendations context menu closed");
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    fn activate_menu_item(&mut self) -> (ComponentEffect<Self>, Option<AppCallback>) {
        let visible = self.visible_items();
        if visible.is_empty() {
            return (AsyncTask::new_no_op(), None);
        }
        let idx = self.selected.min(visible.len() - 1);
        let item = visible[idx];
        match MENU_ITEMS[self.menu_selected] {
            "Play" => {
                info!(
                    "Recommendation menu: Play idx={} kind={:?} title={} artist={}",
                    idx, item.kind, item.title, item.artist
                );
                (
                    AsyncTask::new_no_op(),
                    Some(AppCallback::ActOnRecommendation(
                        idx,
                        item.kind,
                        item.title.clone(),
                        item.artist.clone(),
                    )),
                )
            }
            // Queue-only resolution (search then enqueue without playing) needs a
            // dedicated backend callback; for now reuse the act path so the action works.
            "Add to Queue" => {
                info!(
                    "Recommendation menu: Add to Queue idx={} kind={:?} title={} artist={}",
                    idx, item.kind, item.title, item.artist
                );
                (
                    AsyncTask::new_no_op(),
                    Some(AppCallback::ActOnRecommendation(
                        idx,
                        item.kind,
                        item.title.clone(),
                        item.artist.clone(),
                    )),
                )
            }
            "Copy URL" => {
                if item.url.is_empty() {
                    warn!("Recommendation menu: Copy URL skipped, empty url for idx={}", idx);
                } else {
                    crate::app::structures::copy_to_clipboard(&item.url);
                    info!("Copied recommendation URL: {}", item.url);
                }
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }

    pub fn set_items(&mut self, items: Vec<crate::lastfm_recommend::RecItem>) {
        info!("Recommendations popup loaded {} items for {:?}", items.len(), self.kind);
        self.items = items;
        self.loading = false;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.tick = self.tick.wrapping_add(1);

        let popup_area = area;
        frame.render_widget(Clear, popup_area);

        let title = match self.kind_filter {
            None => " Recommendations (All Kinds) ",
            Some(crate::lastfm_recommend::RecKind::Artists) => " Recommendations (Artists) ",
            Some(crate::lastfm_recommend::RecKind::Albums) => " Recommendations (Albums) ",
            Some(crate::lastfm_recommend::RecKind::Tracks) => " Recommendations (Tracks) ",
        };
        let block = Block::default()
            .title(title)
            .border_style(Style::default().fg(Color::Cyan))
            .borders(Borders::ALL);
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if self.loading {
            frame.render_widget(
                Paragraph::new("Fetching recommendations... (this may take ~10s)")
                    .style(Style::default().fg(Color::Gray)),
                inner,
            );
        } else {
                let visible = self.visible_items();
                if visible.is_empty() {
                    frame.render_widget(
                        Paragraph::new("No recommendations returned.").style(Style::default().fg(Color::Gray)),
                        inner,
                    );
                    return;
                }
                let count = visible.len();
                let items: Vec<Vec<Cow<'static, str>>> = visible
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| {
                        let name = if !item.title.is_empty() {
                            item.title.clone()
                        } else {
                            "-".to_string()
                        };
                        let artist = if !item.artist.is_empty() {
                            item.artist.clone()
                        } else {
                            "-".to_string()
                        };
                        let sim = item.reason.clone().unwrap_or_default();
                        let match_str = item
                            .match_score
                            .map(|m| format!("{:.2}", m))
                            .unwrap_or_else(|| "-".to_string());
                        let list_str = item
                            .playcount
                            .map(format_count)
                            .unwrap_or_else(|| "-".to_string());
                        let type_str = match item.kind {
                            crate::lastfm_recommend::RecKind::Tracks => "Track",
                            crate::lastfm_recommend::RecKind::Albums => "Album",
                            crate::lastfm_recommend::RecKind::Artists => "Artist",
                        };
                        vec![
                            Cow::Owned((idx + 1).to_string()),
                            Cow::Owned(type_str.to_string()),
                            Cow::Owned(name),
                            Cow::Owned(artist),
                            Cow::Owned(sim),
                            Cow::Owned(match_str),
                            Cow::Owned(list_str),
                        ]
                    })
                    .collect();
                let headings: Vec<Cell<'static>> = vec![
                    Cell::from(Line::raw("#")),
                    Cell::from(Line::raw("Type")),
                    Cell::from(Line::raw("Name")),
                    Cell::from(Line::raw("Artist")),
                    Cell::from(Line::raw("Similar To")),
                    Cell::from(Line::raw("Match")),
                    Cell::from(Line::raw("List")),
                ];
                let layout = [
                    BasicConstraint::Length(4),
                    BasicConstraint::Length(7),
                    BasicConstraint::Percentage(Percentage(22)),
                    BasicConstraint::Percentage(Percentage(18)),
                    BasicConstraint::Percentage(Percentage(28)),
                    BasicConstraint::Length(7),
                    BasicConstraint::Length(9),
                ];
                let table_widths = basic_constraints_to_table_constraints(&layout, inner.width, 1);
                let table = ScrollingTable::new(items, headings, table_widths, self.tick)
                    .style(Style::default().fg(Color::Reset))
                    .row_highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR))
                    .headings_style(Style::default().bold().fg(TABLE_HEADINGS_COLOUR))
                    .min_ticker_gap(6)
                    .column_spacing(1)
                    .total_items(count);
                self.table_state.select(Some(self.selected), self.tick);
                frame.render_stateful_widget(table, inner, &mut self.table_state);
        }

        if self.menu_open {
            self.draw_menu(frame, inner);
        }
    }

    fn draw_menu(&self, frame: &mut Frame, inner: Rect) {
        let menu_width = MENU_ITEMS.iter().map(|s| s.len()).max().unwrap_or(4) as u16 + 4;
        let menu_height = MENU_ITEMS.len() as u16 + 2;
        let menu_area = crate::drawutils::left_bottom_corner_rect(menu_height, menu_width, inner);
        frame.render_widget(Clear, menu_area);
        let menu_block = Block::default()
            .title(" Context Menu ")
            .border_style(Style::default().fg(SELECTED_BORDER_COLOUR))
            .borders(Borders::ALL);
        let menu_inner = menu_block.inner(menu_area);
        frame.render_widget(menu_block, menu_area);
        for (i, label) in MENU_ITEMS.iter().enumerate() {
            let style = if i == self.menu_selected {
                Style::default().fg(Color::Reset).bg(ROW_HIGHLIGHT_COLOUR)
            } else {
                Style::default().fg(TEXT_COLOUR)
            };
            frame.render_widget(
                Paragraph::new(*label).style(style),
                Rect {
                    x: menu_inner.x,
                    y: menu_inner.y + i as u16,
                    width: menu_inner.width,
                    height: 1,
                },
            );
        }
    }

}

fn format_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}m", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.0}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
