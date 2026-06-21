use super::library::{InputRouting, LibraryBrowser, LibraryCategory};
use super::Browser;
use super::artistsearch::search_panel::ArtistInputRouting;
use super::artistsearch::songs_panel::AlbumSongsInputRouting;
use super::artistsearch::{self, ArtistSearchBrowser};
use super::shared_components::SearchBlock;
use super::songsearch::SongSearchBrowser;
use crate::app::component::actionhandler::Suggestable;
use crate::app::ui::browser::playlistsearch::search_panel::PlaylistInputRouting;
use crate::app::ui::browser::playlistsearch::songs_panel::PlaylistSongsInputRouting;
use crate::app::ui::browser::playlistsearch::{self, PlaylistSearchBrowser};
use crate::app::view::draw::{draw_advanced_table, draw_list, draw_loadable, draw_panel_mut};
use crate::drawutils::{
    ROW_HIGHLIGHT_COLOUR, SELECTED_BORDER_COLOUR, TEXT_COLOUR, below_left_rect, bottom_of_rect,
};
use vi_text_editor::ViTextEditor;
use ratatui::Frame;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ytmapi_rs::common::{SuggestionType, TextRun};

pub fn draw_browser(
    f: &mut Frame,
    browser: &mut Browser,
    chunk: Rect,
    selected: bool,
    cur_tick: u64,
) {
    match browser.variant {
        super::BrowserVariant::Artist => draw_artist_search_browser(
            f,
            &mut browser.artist_search_browser,
            chunk,
            selected,
            cur_tick,
        ),
        super::BrowserVariant::Song => draw_song_search_browser(
            f,
            &mut browser.song_search_browser,
            chunk,
            selected,
            cur_tick,
        ),
        super::BrowserVariant::Playlist => draw_playlist_search_browser(
            f,
            &mut browser.playlist_search_browser,
            chunk,
            selected,
            cur_tick,
        ),
        super::BrowserVariant::LibraryPlaylist => draw_library_browser(
            f,
            &mut browser.library_browser,
            chunk,
            selected,
            cur_tick,
        ),
    }
}
pub fn draw_artist_search_browser(
    f: &mut Frame,
    browser: &mut ArtistSearchBrowser,
    chunk: Rect,
    selected: bool,
    cur_tick: u64,
) {
    let [artists_chunk, songs_chunk] = Layout::new(
        ratatui::prelude::Direction::Horizontal,
        [Constraint::Max(30), Constraint::Min(0)],
    )
    .areas(chunk);
    // Potentially could handle this better.
    let albumsongsselected = selected
        && browser.input_routing == artistsearch::InputRouting::Song
        && browser.album_songs_panel.route == AlbumSongsInputRouting::List;
    let artistselected = !albumsongsselected
        && selected
        && browser.input_routing == artistsearch::InputRouting::Artist
        && browser.artist_search_panel.route == ArtistInputRouting::List;

    if !browser.artist_search_panel.search_popped {
        draw_panel_mut(
            f,
            &mut browser.artist_search_panel,
            artists_chunk,
            artistselected,
            |t, f, chunk| {
                draw_list(f, t, chunk, cur_tick);
                None
            },
        );
    } else {
        let [search_box_chunk, shrunk_artists_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .areas(artists_chunk);
        draw_panel_mut(
            f,
            &mut browser.artist_search_panel,
            shrunk_artists_chunk,
            artistselected,
            |t, f, chunk| {
                draw_list(f, t, chunk, cur_tick);
                None
            },
        );
        draw_search_box(
            f,
            "Search Artists",
            &mut browser.artist_search_panel.search,
            search_box_chunk,
        );
        // Should this be part of draw_search_box
        if browser.artist_search_panel.has_search_suggestions() {
            draw_search_suggestions(
                f,
                &browser.artist_search_panel.search,
                search_box_chunk,
                artists_chunk,
            )
        }
    }
    draw_panel_mut(
        f,
        &mut browser.album_songs_panel,
        songs_chunk,
        albumsongsselected,
        |t, f, chunk| {
            draw_loadable(f, t, chunk, |t, f, chunk| {
                Some(draw_advanced_table(f, t, chunk, cur_tick))
            })
        },
    );
}
pub fn draw_playlist_search_browser(
    f: &mut Frame,
    browser: &mut PlaylistSearchBrowser,
    chunk: Rect,
    selected: bool,
    cur_tick: u64,
) {
    let [playlists_chunk, songs_chunk] = Layout::new(
        ratatui::prelude::Direction::Horizontal,
        [Constraint::Percentage(30), Constraint::Percentage(70)],
    )
    .areas(chunk);
    // Potentially could handle this better.
    let songs_selected = selected
        && browser.input_routing == playlistsearch::InputRouting::Song
        && browser.playlist_songs_panel.route == PlaylistSongsInputRouting::List;
    let playlists_selected = !songs_selected
        && selected
        && browser.input_routing == playlistsearch::InputRouting::Playlist
        && browser.playlist_search_panel.route == PlaylistInputRouting::List;

    if !browser.playlist_search_panel.search_popped {
        draw_panel_mut(
            f,
            &mut browser.playlist_search_panel,
            playlists_chunk,
            playlists_selected,
            |t, f, chunk| {
                draw_list(f, t, chunk, cur_tick);
                None
            },
        );
    } else {
        let [search_box_chunk, shrunk_playlists_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .areas(playlists_chunk);
        draw_panel_mut(
            f,
            &mut browser.playlist_search_panel,
            shrunk_playlists_chunk,
            playlists_selected,
            |t, f, chunk| {
                draw_list(f, t, chunk, cur_tick);
                None
            },
        );
        draw_search_box(
            f,
            "Search Playlists",
            &mut browser.playlist_search_panel.search,
            search_box_chunk,
        );
        // Should this be part of draw_search_box
        if browser.playlist_search_panel.has_search_suggestions() {
            draw_search_suggestions(
                f,
                &browser.playlist_search_panel.search,
                search_box_chunk,
                playlists_chunk,
            )
        }
    }
    draw_panel_mut(
        f,
        &mut browser.playlist_songs_panel,
        songs_chunk,
        songs_selected,
        |t, f, chunk| {
            draw_loadable(f, t, chunk, |t, f, chunk| {
                Some(draw_advanced_table(f, t, chunk, cur_tick))
            })
        },
    );
}
pub fn draw_library_browser(
    f: &mut Frame,
    browser: &mut LibraryBrowser,
    chunk: Rect,
    selected: bool,
    _cur_tick: u64,
) {
    let [left_chunk, right_chunk] = Layout::new(
        Direction::Horizontal,
        [Constraint::Length(22), Constraint::Min(0)],
    )
    .areas(chunk);

    let left_selected = selected && browser.input_routing == InputRouting::Category;
    let right_selected = selected && browser.input_routing == InputRouting::Content;

    // Left panel: category list
    let cat_items: Vec<ListItem> = LibraryCategory::ALL
        .iter()
        .map(|cat| {
            let label = cat.label();
            let is_active = *cat == browser.category;
            let style = if is_active && browser.input_routing == InputRouting::Content {
                Style::default().fg(TEXT_COLOUR)
            } else if is_active {
                Style::default().fg(SELECTED_BORDER_COLOUR)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(format!(" {label}"), style)))
        })
        .collect();
    let cat_list = List::new(cat_items)
        .highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if left_selected {
                    Style::default().fg(SELECTED_BORDER_COLOUR)
                } else {
                    Style::default()
                })
                .title("Category"),
        );
    let mut cat_state = ListState::default().with_selected(Some(browser.category as usize));
    f.render_stateful_widget(cat_list, left_chunk, &mut cat_state);

    if browser.loading {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if right_selected {
                Style::default().fg(SELECTED_BORDER_COLOUR)
            } else {
                Style::default()
            })
            .title(browser.category.label());
        let inner = block.inner(right_chunk);
        f.render_widget(block, right_chunk);
        f.render_widget(
            Paragraph::new("Loading...")
                .style(Style::default().fg(TEXT_COLOUR))
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }
    if let Some(ref err) = browser.error {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if right_selected {
                Style::default().fg(SELECTED_BORDER_COLOUR)
            } else {
                Style::default()
            })
            .title(browser.category.label());
        let inner = block.inner(right_chunk);
        f.render_widget(block, right_chunk);
        f.render_widget(
            Paragraph::new(err.as_str())
                .style(Style::default().fg(TEXT_COLOUR))
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    let content_chunk = if browser.input_routing == InputRouting::Search {
        let [search_chunk, rest] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .areas(right_chunk);
        draw_text_box(f, "Search", &mut browser.search.search_contents, search_chunk);
        rest
    } else {
        right_chunk
    };

    match browser.category {
        LibraryCategory::LikedSongs => {
            let border_style = if right_selected {
                Style::default().fg(SELECTED_BORDER_COLOUR)
            } else {
                Style::default()
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(browser.category.label());
            let inner = block.inner(content_chunk);
            f.render_widget(block, content_chunk);

            let songs: Vec<_> = browser.song_list.get_list_iter().collect();
            let items: Vec<ListItem> = songs
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let label = format!("{} — {}", s.title, s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "));
                    if i == browser.cur_selected && right_selected {
                        ListItem::new(Line::from(Span::styled(
                            label,
                            Style::default().fg(SELECTED_BORDER_COLOUR),
                        )))
                    } else {
                        ListItem::new(Line::from(Span::raw(label)))
                    }
                })
                .collect();
            let list = List::new(items)
                .highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR));
            let mut state = ListState::default().with_selected(Some(browser.cur_selected));
            f.render_stateful_widget(list, inner, &mut state);
        }
        LibraryCategory::Playlists => {
            let title = if browser.show_playlist_tracks { "Playlist Tracks" } else { "Playlists" };
            let border_style = if right_selected {
                Style::default().fg(SELECTED_BORDER_COLOUR)
            } else {
                Style::default()
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title);
            let inner = block.inner(content_chunk);
            f.render_widget(block, content_chunk);

            if browser.show_playlist_tracks {
                let items: Vec<ListItem> = browser
                    .playlist_tracks
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let label = format!("{} — {}", s.title, s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "));
                        if i == browser.playlist_tracks_selected && right_selected {
                            ListItem::new(Line::from(Span::styled(
                                label,
                                Style::default().fg(SELECTED_BORDER_COLOUR),
                            )))
                        } else {
                            ListItem::new(Line::from(Span::raw(label)))
                        }
                    })
                    .collect();
                let list = List::new(items)
                    .highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR));
                let mut state = ListState::default().with_selected(Some(browser.playlist_tracks_selected));
                f.render_stateful_widget(list, inner, &mut state);
            } else {
                let items: Vec<ListItem> = browser
                    .playlist_data
                    .iter()
                    .enumerate()
                    .map(|(i, pl)| {
                        if i == browser.playlist_selected && right_selected {
                            ListItem::new(Line::from(Span::styled(
                                pl.title.clone(),
                                Style::default().fg(SELECTED_BORDER_COLOUR),
                            )))
                        } else {
                            ListItem::new(Line::from(Span::raw(pl.title.clone())))
                        }
                    })
                    .collect();
                let list = List::new(items)
                    .highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR));
                let mut state = ListState::default().with_selected(Some(browser.playlist_selected));
                f.render_stateful_widget(list, inner, &mut state);
            }
        }
        LibraryCategory::Artists => {
            let border_style = if right_selected {
                Style::default().fg(SELECTED_BORDER_COLOUR)
            } else {
                Style::default()
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Artists");
            let inner = block.inner(content_chunk);
            f.render_widget(block, content_chunk);

            let items: Vec<ListItem> = browser
                .artist_data
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    if i == browser.artist_selected && right_selected {
                        ListItem::new(Line::from(Span::styled(
                            a.artist.clone(),
                            Style::default().fg(SELECTED_BORDER_COLOUR),
                        )))
                    } else {
                        ListItem::new(Line::from(Span::raw(a.artist.clone())))
                    }
                })
                .collect();
            let list = List::new(items)
                .highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR));
            let mut state = ListState::default().with_selected(Some(browser.artist_selected));
            f.render_stateful_widget(list, inner, &mut state);
        }
        LibraryCategory::Albums => {
            let border_style = if right_selected {
                Style::default().fg(SELECTED_BORDER_COLOUR)
            } else {
                Style::default()
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Albums");
            let inner = block.inner(content_chunk);
            f.render_widget(block, content_chunk);

            let items: Vec<ListItem> = browser
                .album_data
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let label = format!("{} — {}", a.title, a.artist);
                    if i == browser.album_selected && right_selected {
                        ListItem::new(Line::from(Span::styled(
                            label,
                            Style::default().fg(SELECTED_BORDER_COLOUR),
                        )))
                    } else {
                        ListItem::new(Line::from(Span::raw(label)))
                    }
                })
                .collect();
            let list = List::new(items)
                .highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR));
            let mut state = ListState::default().with_selected(Some(browser.album_selected));
            f.render_stateful_widget(list, inner, &mut state);
        }
    }
}
pub fn draw_song_search_browser(
    f: &mut Frame,
    browser: &mut SongSearchBrowser,
    chunk: Rect,
    selected: bool,
    cur_tick: u64,
) {
    if !browser.search_popped {
        draw_panel_mut(f, browser, chunk, selected, |t, f, chunk| {
            draw_loadable(f, t, chunk, |t, f, chunk| {
                Some(draw_advanced_table(f, t, chunk, cur_tick))
            })
        });
    } else {
        let [search_box_chunk, new_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .areas(chunk);
        draw_panel_mut(f, browser, new_chunk, false, |t, f, chunk| {
            draw_loadable(f, t, chunk, |t, f, chunk| {
                Some(draw_advanced_table(f, t, chunk, cur_tick))
            })
        });
        draw_search_box(f, "Search Songs", &mut browser.search, search_box_chunk);
        // Should this be part of draw_search_box
        if browser.has_search_suggestions() {
            draw_search_suggestions(f, &browser.search, search_box_chunk, chunk)
        }
    }
}

/// Draw a text input box
// TODO: Shift to a more general module.
pub fn draw_text_box(
    f: &mut Frame,
    title: impl AsRef<str>,
    contents: &mut ViTextEditor,
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
}
fn draw_search_box(f: &mut Frame, title: impl AsRef<str>, search: &mut SearchBlock, chunk: Rect) {
    draw_text_box(f, title, &mut search.search_contents, chunk);
}

fn draw_search_suggestions(f: &mut Frame, search: &SearchBlock, chunk: Rect, max_bounds: Rect) {
    let suggestions = search.get_search_suggestions();
    let height = suggestions.len() + 1;
    let divider_chunk = bottom_of_rect(chunk);
    let suggestion_chunk = below_left_rect(
        height.try_into().unwrap_or(u16::MAX),
        chunk.width,
        chunk,
        max_bounds,
    );
    let [suggestion_side_borders_chunk, suggestion_list_chunk] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .areas(suggestion_chunk);
    let mut list_state = ListState::default().with_selected(search.suggestions_cur);
    let list_items = suggestions.iter().map(|s| {
        ListItem::new(Line::from_iter(
            std::iter::once(s.suggestion_type)
                .map(|ty| match ty {
                    SuggestionType::History => Span::raw(" "),
                    SuggestionType::Prediction => Span::raw(" "),
                })
                .chain(s.runs.iter().map(|s| match s {
                    TextRun::Bold(str) => {
                        Span::styled(str, Style::new().add_modifier(Modifier::BOLD))
                    }
                    TextRun::Normal(str) => Span::raw(str),
                })),
        ))
    });
    let block = List::new(list_items)
        .style(Style::new().fg(TEXT_COLOUR))
        .highlight_style(Style::new().bg(ROW_HIGHLIGHT_COLOUR))
        .block(
            Block::default()
                .borders(Borders::all().difference(Borders::TOP))
                .style(Style::new().fg(SELECTED_BORDER_COLOUR)),
        );
    let side_borders = Block::default()
        .borders(Borders::LEFT.union(Borders::RIGHT))
        .style(Style::new().fg(SELECTED_BORDER_COLOUR));
    let divider = Block::default().borders(Borders::TOP);
    f.render_widget(Clear, suggestion_chunk);
    f.render_widget(side_borders, suggestion_side_borders_chunk);
    f.render_widget(Clear, divider_chunk);
    f.render_widget(divider, divider_chunk);
    f.render_stateful_widget(block, suggestion_list_chunk, &mut list_state);
}
