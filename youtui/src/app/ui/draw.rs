use super::{
    WindowContext, YoutuiWindow, footer, header,
};
use crate::app::view::draw::{draw_panel_mut_impl, draw_table_impl};
use crate::app::view::{BasicConstraint, Drawable, DrawableMut};
use crate::drawutils::{SELECTED_BORDER_COLOUR, TEXT_COLOUR, left_bottom_corner_rect};
use crate::keyaction::{DisplayableKeyAction, DisplayableMode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Row, Table};
use ratatui_image::picker::Picker;

// Add tests to try and draw app with oddly sized windows.
pub fn draw_app(f: &mut Frame, w: &mut YoutuiWindow, terminal_image_capabilities: &Picker) {
    // Clear sixel state; draw_footer will re-set if visible
    w.sixel_data = None;
    w.sixel_rect = None;
    let [header_chunk, window_chunk, footer_chunk] = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(header::header_required_height(w)),
            Constraint::Min(2),
            Constraint::Length(6),
        ])
        .areas(f.area());
    header::draw_header(f, w, header_chunk);
    let context_selected = !w.help.shown && !w.key_pending();
    match w.context {
        WindowContext::Browser => {
            w.browser
                .draw_mut_chunk(f, window_chunk, context_selected, w.tick);
        }
        WindowContext::Logs => w.logger.draw_chunk(f, window_chunk, context_selected),
        WindowContext::Playlist | WindowContext::PlaylistEditor => {
            w.playlist
                .draw_mut_chunk(f, window_chunk, context_selected, w.tick);
        }
        WindowContext::PlaylistSavePopup => {
            w.playlist
                .draw_mut_chunk(f, window_chunk, context_selected, w.tick);
        }
        WindowContext::PlaylistUpdatePopup => {
            w.playlist
                .draw_mut_chunk(f, window_chunk, context_selected, w.tick);
        }
        WindowContext::Lyrics => {
            w.playlist
                .draw_mut_chunk(f, window_chunk, context_selected, w.tick);
        }
        WindowContext::SongInfo => {
            w.playlist
                .draw_mut_chunk(f, window_chunk, context_selected, w.tick);
        }
        WindowContext::PlaylistRenamePopup
        | WindowContext::PlaylistEditPopup
        | WindowContext::PlaylistDetailsPopup
        | WindowContext::Notes => {
            w.playlist
                .draw_mut_chunk(f, window_chunk, context_selected, w.tick);
        }
    }
    if w.help.shown {
        draw_help(f, w, window_chunk);
    }
    if w.key_pending() {
        draw_popup(f, w, window_chunk);
    }
    if let Some(popup) = &mut w.playlist_save_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.playlist_update_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.lyrics_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.song_info_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.config_editor_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.playlist_rename_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.playlist_edit_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.playlist_details_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.playlist_editor_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.notes_popup {
        popup.draw(f, f.area());
    }
    if let Some((_, ref title)) = w.delete_confirm {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Clear, Paragraph};
        use ratatui::layout::{Alignment, Constraint, Direction, Layout};
        let area = f.area();
        f.render_widget(Clear, area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Length(5),
                Constraint::Min(3),
                Constraint::Percentage(40),
            ])
            .split(area);
        let you_died = Paragraph::new("Delete Playlist?")
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(you_died, chunks[1]);
        let prompt = Paragraph::new(format!("Delete \"{}\"? (y/N)", title))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(prompt, chunks[2]);
    }
    if w.quit_confirm {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Clear, Paragraph};
        use ratatui::layout::{Alignment, Constraint, Direction, Layout};
        let area = f.area();
        f.render_widget(Clear, area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),
            Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Percentage(40),
            ])
            .split(area);
        let you_died = Paragraph::new("YOU DIED")
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(you_died, chunks[1]);
        let prompt = Paragraph::new("Quit? (y/N)")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(prompt, chunks[2]);
    } else if w.command_mode {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Clear, Paragraph};
        use ratatui::layout::{Alignment, Constraint, Direction, Layout};
        let area = f.area();
        f.render_widget(Clear, area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Length(3),
                Constraint::Percentage(45),
            ])
            .split(area);
        let cmd_text = w.command_editor.render_simple(":");
        let cmd = Paragraph::new(cmd_text)
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);
        f.render_widget(cmd, chunks[1]);
    } else {
        footer::draw_footer(f, w, footer_chunk, terminal_image_capabilities);
    }
    if let Some(popup) = &mut w.playlist_rename_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.playlist_edit_popup {
        popup.draw(f, f.area());
    }
    if let Some(popup) = &mut w.playlist_details_popup {
        popup.draw(f, f.area());
    }
    if w.delete_confirm.is_some() {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Clear, Paragraph};
        use ratatui::layout::Alignment;
        let area = f.area();
        f.render_widget(Clear, area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Percentage(40),
            ])
            .split(area);
        let title = w.delete_confirm.as_ref().map(|(_, t)| t.as_str()).unwrap_or("this playlist");
        let text = Paragraph::new(format!("Delete \"{}\"?", title))
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(text, chunks[1]);
        let prompt = Paragraph::new("This cannot be undone. Proceed? (y/N)")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(prompt, chunks[2]);
    }
    if let Some(popup) = &mut w.album_art_popup {
        popup.draw(f, f.area(), terminal_image_capabilities);
    }
}

fn draw_popup(f: &mut Frame, w: &YoutuiWindow, chunk: Rect) {
    // NOTE: if there are more commands than we can fit on the screen, some will be
    // cut off. If there are no commands, no need to draw anything.
    let Some(DisplayableMode {
        displayable_commands: commands,
        description: title,
    }) = w.get_cur_displayable_mode()
    else {
        return;
    };
    let shortcuts_descriptions = commands.collect::<Vec<_>>();
    // TODO: Make commands_vec an iterator instead of a vec
    let (shortcut_len, description_len, commands_vec) = shortcuts_descriptions.iter().fold(
        (0, 0, Vec::new()),
        |(acc1, acc2, mut commands_vec),
         DisplayableKeyAction {
             keybinds,
             context: _,
             description,
         }| {
            commands_vec.push(
                Row::new(vec![format!("{}", keybinds), format!("{}", description)])
                    .style(Style::new().fg(TEXT_COLOUR)),
            );
            (
                keybinds.len().max(acc1),
                description.len().max(acc2),
                commands_vec,
            )
        },
    );
    let width = shortcut_len + description_len + 3;
    let height = commands_vec.len() + 2;
    let table_constraints = [
        Constraint::Min(shortcut_len.try_into().unwrap_or(u16::MAX)),
        Constraint::Min(description_len.try_into().unwrap_or(u16::MAX)),
    ];
    let block = Table::new(commands_vec, table_constraints).block(
        Block::default()
            .title(title.as_ref())
            .borders(Borders::ALL)
            .style(Style::new().fg(SELECTED_BORDER_COLOUR)),
    );
    let area = left_bottom_corner_rect(
        height.try_into().unwrap_or(u16::MAX),
        width.try_into().unwrap_or(u16::MAX),
        chunk,
    );
    f.render_widget(Clear, area);
    f.render_widget(block, area);
}

/// Draw the help page. The help page should show all visible commands for the
/// current page.
fn draw_help(f: &mut Frame, w: &mut YoutuiWindow, chunk: Rect) {
    // XXX: Probably don't need to map then fold,
    // just fold.
    //
    // XXX: Fold closure could be written as a function, then becomes
    // testable.
    let (mut s_len, mut c_len, mut d_len, items) = w
        .get_help_list_items()
        .into_iter()
        .map(
            |DisplayableKeyAction {
                 keybinds,
                 context,
                 description,
             }| (keybinds.len(), context.len(), description.len()),
        )
        .fold((0, 0, 0, 0), |(smax, cmax, dmax, n), (s, c, d)| {
            (smax.max(s), cmax.max(c), dmax.max(d), n + 1)
        });
    // Ensure the width of each column is at least as wide as header.
    (s_len, c_len, d_len) = (s_len.max(3), c_len.max(7), d_len.max(7));
    // Total block width required, including padding and borders.
    let width = s_len + c_len + d_len + 4;
    // Total block height required, including header and borders.
    let height = items + 3;
    // Naive implementation
    // XXX: We're running get_help_list_items a second time here.
    // Better to move to the fold above.
    let table_constraints = [
        BasicConstraint::Length(s_len.try_into().unwrap_or(u16::MAX)),
        BasicConstraint::Length(c_len.try_into().unwrap_or(u16::MAX)),
        BasicConstraint::Length(d_len.try_into().unwrap_or(u16::MAX)),
    ];
    let headings = ["Key", "Context", "Command"].into_iter();
    let area = left_bottom_corner_rect(
        height.try_into().unwrap_or(u16::MAX),
        width.try_into().unwrap_or(u16::MAX),
        chunk,
    );
    f.render_widget(Clear, area);
    let cur_tick = w.tick;
    draw_panel_mut_impl(
        f,
        w,
        area,
        true,
        |_| "Help".into(),
        |t, f, chunk| {
            let commands_table = t.get_help_list_items().into_iter().map(
                |DisplayableKeyAction {
                     keybinds,
                     context,
                     description,
                 }| { [keybinds, context, description].into_iter() },
            );
            let (new_state, effect) = draw_table_impl(
                f,
                chunk,
                t.help.cur,
                None,
                None,
                &t.help.widget_state,
                commands_table,
                items,
                &table_constraints,
                headings,
                None,
                cur_tick,
            );
            t.help.widget_state = new_state;
            Some(effect)
        },
    );
}

/// Draw a text input box
pub fn draw_text_box(
    f: &mut Frame,
    title: impl AsRef<str>,
    contents: &mut vi_text_editor::ViTextEditor,
    chunk: Rect,
) {
    let block_widget = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SELECTED_BORDER_COLOUR))
        .title(title.as_ref());
    let text_chunk = block_widget.inner(chunk);
    let display = contents.render_simple("");
    let text_widget = Paragraph::new(display)
        .style(Style::default().fg(TEXT_COLOUR));
    f.render_widget(block_widget, chunk);
    f.render_widget(text_widget, text_chunk);
    // Position hardware cursor at text cursor for terminals
    f.set_cursor_position((
        text_chunk.x + contents.cursor as u16,
        text_chunk.y,
    ));
}
