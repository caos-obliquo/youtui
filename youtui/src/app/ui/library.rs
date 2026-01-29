use crate::app::structures::LibraryState;
pub use crate::app::server::messages::GetLibraryPlaylists;
use crate::app::component::actionhandler::{ActionHandler, YoutuiEffect};
use crate::app::ui::action::LibraryAction;
use crate::config::Config;
use crate::config::keymap::Keymap;
use async_callback_manager::{AsyncTask, Constraint};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub struct Library {
    pub state: LibraryState,
}

impl_youtui_component!(Library);

impl Library {
    pub fn new() -> Self {
        Self {
            state: LibraryState::new(),
        }
    }

    pub fn get_all_keybinds<'a>(&'a self, _config: &'a Config) -> impl Iterator<Item = &'a Keymap<LibraryAction>> {
        std::iter::empty()
    }

    pub fn increment_list(&mut self, amount: isize) {
        if amount > 0 {
            for _ in 0..amount {
                self.state.move_down();
            }
        } else {
            for _ in 0..amount.abs() {
                self.state.move_up();
            }
        }
    }

    pub fn is_scrollable(&self) -> bool {
        !self.state.playlists.is_empty()
    }

    pub fn is_text_handling(&self) -> bool {
        false
    }

    pub fn get_text(&self) -> Option<&str> {
        None
    }

    pub fn replace_text(&mut self, _text: String) {}

    pub fn clear_text(&mut self) -> bool {
        false
    }

    pub fn handle_text_event_impl(&mut self, _event: &crossterm::event::Event) {}

    pub fn dominant_keybinds_active(&self) -> bool {
        false
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect) {
        if self.state.is_loading {
            let widget = Paragraph::new("Loading library playlists...")
                .block(Block::default().borders(Borders::ALL).title("Library"));
            f.render_widget(widget, area);
            return;
        }

        if self.state.playlists.is_empty() {
            let widget = Paragraph::new("No playlists found. Press 'r' to refresh.")
                .block(Block::default().borders(Borders::ALL).title("Library"));
            f.render_widget(widget, area);
            return;
        }

        let items: Vec<ListItem> = self
            .state
            .playlists
            .iter()
            .map(|playlist| {
                let count_str = playlist.count
                    .as_ref()
                    .map(|c| c.to_string())
                    .unwrap_or("?".to_string());
                let content = format!("{} ({})", playlist.title, count_str);
                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Library Playlists"))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        let mut list_state = ListState::default();
        list_state.select(Some(self.state.selected_index));

        f.render_stateful_widget(list, area, &mut list_state);
    }
}

impl ActionHandler<LibraryAction> for Library {
    fn apply_action(&mut self, action: LibraryAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            LibraryAction::Refresh => {
                self.state.start_loading();
                AsyncTask::new_future(
                    GetLibraryPlaylists,
                    HandleLibraryPlaylistsLoaded,
                    Some(Constraint::new_block_same_type()),
                )
            }
            LibraryAction::Down => {
                self.state.move_down();
                AsyncTask::new_no_op()
            }
            LibraryAction::Up => {
                self.state.move_up();
                AsyncTask::new_no_op()
            }
            LibraryAction::Select => {
                // TODO: Handle playlist selection
                AsyncTask::new_no_op()
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct HandleLibraryPlaylistsLoaded;

impl_youtui_task_handler!(
    HandleLibraryPlaylistsLoaded,
    Result<Vec<ytmapi_rs::parse::LibraryPlaylist>, anyhow::Error>,
    Library,
    |_, result| |this: &mut Library| {
        match result {
            Ok(playlists) => {
                this.state.set_playlists(playlists);
            }
            Err(e) => {
                tracing::error!("Failed to load library playlists: {:?}", e);
                this.state.is_loading = false;
            }
        }
        AsyncTask::new_no_op()
    }
);
