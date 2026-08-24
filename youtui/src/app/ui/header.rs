use crate::app::ui::WindowContext;
use crate::app::view::HasTabs;
use crate::drawutils::{BUTTON_BG_COLOUR, BUTTON_FG_COLOUR};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Minimal header: two bordered blocks side by side.
/// - Commands block: F1/F2/F3, o (Context Menu), ? help only.
/// - Browser block (browser context): the five tabs on one line.
/// Everything else lives in the ? help menu.
///
/// When the fuzzy finder or the browser local filter is active we grow the
/// header by one line so the search query has its own space WITHOUT wiping the
/// tabs/commands (the previous implementation cleared the whole header).
pub fn header_required_height(w: &super::YoutuiWindow) -> u16 {
    if w.fuzzy_finder.shown
        || (matches!(w.context, WindowContext::Browser) && w.browser.filter_active())
    {
        4
    } else {
        3
    }
}

fn button_span(label: &str) -> Span<'static> {
    Span::styled(
        label.to_string(),
        Style::default().bg(BUTTON_BG_COLOUR).fg(BUTTON_FG_COLOUR),
    )
}

pub fn draw_header(f: &mut Frame, w: &super::YoutuiWindow, chunk: Rect) {
    // Fuzzy finder active: keep the normal header visible and show the query
    // on its own line at the bottom of the header block, in the project-rule
    // `[SEARCH: text (N/M)]` format.
    if w.fuzzy_finder.shown {
        let [main_chunk, input_chunk] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(chunk);
        draw_normal_header(f, w, main_chunk);
        draw_fuzzy_input(f, w, input_chunk);
        return;
    }
    // Browser local filter active: same treatment, keep the header intact.
    if matches!(w.context, WindowContext::Browser) && w.browser.filter_active() {
        let [main_chunk, input_chunk] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(chunk);
        draw_normal_header(f, w, main_chunk);
        let editor = w.browser.filter_editor();
        let text = format!("[FILTER: {}]", editor.get_text());
        let p = Paragraph::new(text).style(Style::default().fg(Color::Cyan));
        f.render_widget(p, input_chunk);
        f.set_cursor_position((input_chunk.x + 9 + editor.cursor as u16, input_chunk.y));
        return;
    }
    draw_normal_header(f, w, chunk);
}

fn draw_fuzzy_input(f: &mut Frame, w: &super::YoutuiWindow, chunk: Rect) {
    let q = w.fuzzy_finder.query();
    let total = w.fuzzy_finder.entries.len();
    let shown = w.fuzzy_finder.matches.len();
    let text = format!("[SEARCH: {q} ({shown}/{total})]");
    let p = Paragraph::new(text).style(Style::default().fg(Color::Cyan));
    f.render_widget(p, chunk);
    // Cursor sits right after the typed query (skip the "[SEARCH: " prefix = 9 chars).
    f.set_cursor_position((chunk.x + 9 + w.fuzzy_finder.editor.cursor as u16, chunk.y));
}

fn draw_normal_header(f: &mut Frame, w: &super::YoutuiWindow, chunk: Rect) {
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

    let commands_block = Block::default().borders(Borders::ALL).title("Commands");
    let commands_widget = Paragraph::new(Line::from(spans));
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
