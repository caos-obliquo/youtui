use crate::app::ui::WindowContext;
use crate::app::view::HasTabs;
use crate::drawutils::{BUTTON_BG_COLOUR, BUTTON_FG_COLOUR};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

const TAB_ROWS: u16 = 1;

/// Minimal header: only F1/F2/F3, o menu and ? help are shown inline.
/// Everything else is discoverable via the ? help menu.
pub fn header_required_height(_w: &super::YoutuiWindow) -> u16 {
    3
}

fn button_span(label: &str) -> Span<'static> {
    Span::styled(
        label.to_string(),
        Style::default().bg(BUTTON_BG_COLOUR).fg(BUTTON_FG_COLOUR),
    )
}

pub fn draw_header(f: &mut Frame, w: &super::YoutuiWindow, chunk: Rect) {
    let mut spans: Vec<Span> = Vec::new();

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

    // Minimal command surface. Full list lives in ? help.
    spans.push(button_span("F1"));
    spans.push(Span::raw(" (Search) "));
    spans.push(button_span("F2"));
    spans.push(Span::raw(" (Browser) "));
    spans.push(button_span("F3"));
    spans.push(Span::raw(" (Playlist) "));
    if matches!(w.context, WindowContext::Playlist | WindowContext::Browser) {
        spans.push(button_span("o"));
        spans.push(Span::raw(" (Menu) "));
    }
    spans.push(button_span("?"));
    spans.push(Span::raw(" (Help) "));

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
        Constraint::Max(tabs_widget.required_width().try_into().unwrap_or(u16::MAX) + 2),
    ])
    .areas(chunk);
    f.render_widget(commands_widget, commands_block.inner(commands_chunk));
    f.render_widget(commands_block, commands_chunk);
    f.render_widget(tabs_widget, tabs_block.inner(tabs_chunk));
    f.render_widget(tabs_block, tabs_chunk);
}
