use crate::app::ui::WindowContext;
use crate::app::view::HasTabs;
use crate::drawutils::{BUTTON_BG_COLOUR, BUTTON_FG_COLOUR};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Header height. When a key mode is pending (e.g. `o` context menu), the
/// Commands block grows to list the menu items; otherwise one line.
pub fn header_required_height(w: &super::YoutuiWindow) -> u16 {
    if w.key_pending() {
        if let Some(mode) = w.get_cur_displayable_mode() {
            let n = mode.displayable_commands.count();
            return (n as u16 + 2).min(45); // borders + items
        }
    }
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

    spans.push(button_span("F1"));
    spans.push(Span::raw(" (Toggle Search) "));
    spans.push(button_span("F2"));
    spans.push(Span::raw(" (Toggle Browser) "));
    spans.push(button_span("F3"));
    spans.push(Span::raw(" (Toggle Playlist) "));
    if matches!(w.context, WindowContext::Playlist | WindowContext::Browser) {
        spans.push(button_span("o"));
        spans.push(Span::raw(" (Context Menu) "));
    }
    spans.push(button_span("?"));
    spans.push(Span::raw(" (Toggle Help) "));

    // When a key mode is pending (o → context menu etc), expand the Commands
    // block with the menu items so the header's empty space is used.
    let menu_lines: Vec<Line> = if w.key_pending() {
        w.get_cur_displayable_mode()
            .map(|mode| {
                let title = mode.description;
                let mut lines = vec![Line::from(Span::styled(
                    format!(" {} ", title),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))];
                lines.extend(mode.displayable_commands.map(|c| {
                    Line::from(vec![
                        Span::styled(
                            format!(" {:6} ", c.keybinds),
                            Style::default()
                                .fg(BUTTON_FG_COLOUR)
                                .bg(BUTTON_BG_COLOUR),
                        ),
                        Span::raw(format!(" {}", c.description)),
                    ])
                }));
                lines
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Top line: mode indicator + minimal commands.
    let mut all_lines: Vec<Line> = Vec::new();
    if menu_lines.is_empty() {
        all_lines.push(Line::from(spans));
    } else {
        // First line: the pending key + description; then the items.
        let mut it = menu_lines.into_iter();
        if let Some(first) = it.next() {
            all_lines.push(first);
        }
        all_lines.extend(it);
    }

    let commands_block = Block::default().borders(Borders::ALL).title("Commands");
    let commands_widget = Paragraph::new(all_lines.clone());
    if !matches!(w.context, WindowContext::Browser) {
        f.render_widget(commands_widget, commands_block.inner(chunk));
        f.render_widget(commands_block, chunk);
        return;
    }

    let title = w.browser.tabs_block_title();
    let selected_item = w.browser.selected_tab_idx();
    let mut tab_spans: Vec<Span> = Vec::new();
    for (i, item) in w.browser.tab_items().into_iter().enumerate() {
        let label: std::borrow::Cow<'_, str> = item.into();
        if i == selected_item {
            tab_spans.push(Span::styled(
                label,
                Style::default()
                    .fg(BUTTON_FG_COLOUR)
                    .bg(BUTTON_BG_COLOUR)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tab_spans.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
        }
        tab_spans.push(Span::raw("  "));
    }
    let tabs_block = Block::default().borders(Borders::ALL).title(title);

    let tab_width: u16 = tab_spans
        .iter()
        .map(|s| s.content.len() as u16)
        .sum::<u16>()
        .max(20);
    let tabs_widget = Paragraph::new(Line::from(tab_spans));
    let [commands_chunk, tabs_chunk] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Max(tab_width + 2),
    ])
    .areas(chunk);
    f.render_widget(commands_widget, commands_block.inner(commands_chunk));
    f.render_widget(commands_block, commands_chunk);
    f.render_widget(tabs_widget, tabs_block.inner(tabs_chunk));
    f.render_widget(tabs_block, tabs_chunk);
}
