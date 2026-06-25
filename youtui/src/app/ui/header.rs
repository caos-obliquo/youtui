use crate::app::component::actionhandler::{KeyRouter, get_global_keybinds_as_readable_iter};
use crate::app::ui::WindowContext;
use crate::app::view::HasTabs;
use crate::drawutils::{BUTTON_BG_COLOUR, BUTTON_FG_COLOUR};
use crate::keyaction::DisplayableKeyAction;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

const TAB_ROWS: u16 = 2;

/// Helper to dynamically resize header based on content.
/// Currently hardcoded as the logic is simple - but in future should be more
/// dynamic, perhaps creating header as a widget.
pub fn header_required_height(w: &super::YoutuiWindow) -> u16 {
    if matches!(w.context, WindowContext::Browser) {
        4
    } else {
        3
    }
}

pub fn draw_header(f: &mut Frame, w: &super::YoutuiWindow, chunk: Rect) {
    let keybinds = get_global_keybinds_as_readable_iter(w.get_active_keybinds(&w.config));

    let mut spans: Vec<Span> = Vec::new();

    // Prepend vi mode indicator at the very start — always visible
    let vi_mode: Option<String> = if w.command_mode {
        Some(w.command_editor.mode_char().to_string())
    } else if let Some(ref popup) = w.config_editor_popup {
        Some(popup.mode_char().to_string())
    } else if w.playlist.visual_mode {
        Some("[V]".to_string())
    } else if matches!(w.context, crate::app::ui::WindowContext::Browser) {
        w.browser.text_editor_mode()
    } else {
        None
    };
    if let Some(ref mode) = vi_mode {
        spans.push(Span::styled(mode.as_str(), Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(" "));
    }

    spans.extend(keybinds.flat_map(
        |DisplayableKeyAction {
             keybinds,
             description,
             ..
         }| {
            let label = if description.is_empty() {
                "Action".to_string()
            } else {
                description.into_owned()
            };
            vec![
                Span::styled(
                    keybinds,
                    Style::default().bg(BUTTON_BG_COLOUR).fg(BUTTON_FG_COLOUR),
                ),
                Span::raw(" ("),
                Span::raw(label),
                Span::raw(")"),
                Span::raw(" "),
            ]
        },
    ));
    // Append 'o (Menu)' hint for contexts with context menu support
    if matches!(w.context, WindowContext::Playlist | WindowContext::Browser) {
        spans.push(Span::styled(
            "o",
            Style::default().bg(BUTTON_BG_COLOUR).fg(BUTTON_FG_COLOUR),
        ));
        spans.push(Span::raw(" (Menu) "));
    }
    let help_string = Line::from_iter(spans);
    let commands_block = Block::default().borders(Borders::ALL).title("Commands");
    let commands_widget = Paragraph::new(help_string).wrap(Wrap { trim: true });
    if !matches!(w.context, WindowContext::Browser) {
        f.render_widget(commands_widget, commands_block.inner(chunk));
        f.render_widget(commands_block, chunk);
        return;
    }
    let title = w.browser.tabs_block_title();
    let items = w.browser.tab_items();
    let selected_item = w.browser.selected_tab_idx();
    let tabs_block = Block::default().borders(Borders::ALL).title(title);
    let tabs_widget = crate::widgets::TabGrid::new_with_max_rows(items, TAB_ROWS)
        .select(selected_item)
        .highlight_style(Style::new().fg(BUTTON_FG_COLOUR).bg(BUTTON_BG_COLOUR));
    let [commands_chunk, tabs_chunk] = Layout::horizontal([
        Constraint::Min(0),
        // Add two to accommodate block
        Constraint::Max(tabs_widget.required_width().try_into().unwrap_or(u16::MAX) + 2),
    ])
    .areas(chunk);
    f.render_widget(commands_widget, commands_block.inner(commands_chunk));
    f.render_widget(commands_block, commands_chunk);
    f.render_widget(tabs_widget, tabs_block.inner(tabs_chunk));
    f.render_widget(tabs_block, tabs_chunk);
}
