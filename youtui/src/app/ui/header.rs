use crate::app::ui::WindowContext;
use crate::app::view::HasTabs;
use crate::drawutils::{BUTTON_BG_COLOUR, BUTTON_FG_COLOUR};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Minimal header: one line with F1/F2/F3, o menu, ? help, and (in browser)
/// the tab list inline. Everything else lives in the ? help menu.
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
    spans.push(Span::raw(" "));
    spans.push(button_span("F2"));
    spans.push(Span::raw(" "));
    spans.push(button_span("F3"));
    spans.push(Span::raw(" "));
    if matches!(w.context, WindowContext::Playlist | WindowContext::Browser) {
        spans.push(button_span("o"));
        spans.push(Span::raw(" "));
    }
    spans.push(button_span("?"));
    spans.push(Span::raw(" "));

    // Browser tabs inline, same line.
    if matches!(w.context, WindowContext::Browser) {
        spans.push(Span::styled("|", Style::default().fg(Color::DarkGray)));
        spans.push(Span::raw(" "));
        let selected = w.browser.selected_tab_idx();
        for (i, item) in w.browser.tab_items().into_iter().enumerate() {
            let label: std::borrow::Cow<'_, str> = item.into();
            if i == selected {
                spans.push(Span::styled(
                    label,
                    Style::default()
                        .fg(BUTTON_FG_COLOUR)
                        .bg(BUTTON_BG_COLOUR)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    label,
                    Style::default().fg(Color::DarkGray),
                ));
            }
            spans.push(Span::raw("  "));
        }
    }

    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(chunk);
    f.render_widget(block, chunk);
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        inner,
    );
}
