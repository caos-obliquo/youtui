use super::library::{InputRouting, LibraryBrowser, LibraryCategory};
use super::Browser;
use super::artistsearch::search_panel::ArtistInputRouting;
use super::artistsearch::songs_panel::AlbumSongsInputRouting;
use super::artistsearch::{self, ArtistSearchBrowser};
use super::playlistsearch::PlaylistSearchBrowser;
use super::shared_components::SearchBlock;
use super::songsearch::SongSearchBrowser;
use crate::app::component::actionhandler::Suggestable;
use crate::app::ui::browser::albumsearch::AlbumSearchBrowser;
use crate::app::view::draw::{draw_advanced_table, draw_list, draw_loadable, draw_panel_mut};
use crate::drawutils::{
    ROW_HIGHLIGHT_COLOUR, SELECTED_BORDER_COLOUR, TEXT_COLOUR, below_left_rect, bottom_of_rect,
};
use crate::widgets::{ScrollingList, ScrollingListState};
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
        super::BrowserVariant::Album => draw_album_search_browser(
            f,
            &mut browser.album_search_browser,
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
        super::BrowserVariant::PlaylistSearch => draw_playlist_search_browser(
            f,
            &mut browser.playlist_search_browser,
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
pub fn draw_album_search_browser(
    f: &mut Frame,
    browser: &mut AlbumSearchBrowser,
    chunk: Rect,
    selected: bool,
    cur_tick: u64,
) {
    let [left_chunk, right_chunk] = Layout::new(
        Direction::Horizontal,
        [Constraint::Percentage(30), Constraint::Percentage(70)],
    ).areas(chunk);
    let show_tracks = browser.show_tracks;
    let left_selected = selected && !show_tracks;
    let right_selected = selected && show_tracks;

    // Left panel: search box + album list below (when searching), or just album list
    let left_album_chunk = if browser.search_popped {
        let [search_box_chunk, rest_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .areas(left_chunk);
        let search_block = Block::default()
            .title(" Search Albums ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(SELECTED_BORDER_COLOUR));
        let text_chunk = search_block.inner(search_box_chunk);
        let display = browser.search.search_contents.render_simple("");
        f.render_widget(Clear, search_box_chunk);
        f.render_widget(search_block, search_box_chunk);
        f.render_widget(Paragraph::new(display).style(Style::default().fg(TEXT_COLOUR)), text_chunk);
        if browser.has_search_suggestions() {
            draw_search_suggestions(f, &browser.search, search_box_chunk, left_chunk);
        }
        rest_chunk
    } else {
        left_chunk
    };

    // Left panel: album list with count
    let count = browser.albums.len();
    let title_str = format!(" Albums - {} ", count);
    let left_block = Block::default()
        .title(title_str.as_str())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if left_selected { SELECTED_BORDER_COLOUR } else { ratatui::style::Color::DarkGray }));
    let left_inner = left_block.inner(left_album_chunk);
    f.render_widget(Clear, left_album_chunk);
    f.render_widget(left_block, left_album_chunk);

    if browser.albums.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "No albums found",
            Style::default().fg(ratatui::style::Color::DarkGray),
        )))
        .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(empty_msg, left_inner);
    } else {
        let items: Vec<String> = browser.albums.iter().enumerate().map(|(i, a)| {
            let label = format!("{} - {}", a.title, a.artist);
            label
        }).collect();
        browser.album_list_state.select(Some(browser.album_selected), cur_tick);
        let scrolling_list = ScrollingList::new(items, cur_tick)
            .highlight_style(Style::default().bg(ROW_HIGHLIGHT_COLOUR))
            .max_times_to_scroll(Some(2));
        f.render_stateful_widget(
            scrolling_list,
            left_inner,
            &mut browser.album_list_state,
        );
    }

    // Right panel: tracks via advanced table, even when no album selected
    draw_panel_mut(f, browser, right_chunk, right_selected, |t, f, chunk| {
        draw_loadable(f, t, chunk, |t, f, chunk| {
            Some(draw_advanced_table(f, t, chunk, cur_tick))
        })
    });
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
                    let label = format!("{} - {}", s.title, s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "));
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
                let col_width = inner.width.saturating_sub(2) as usize;
                let num_w = 4usize;
                let dur_w = 8usize;
                let title_w = (col_width.saturating_sub(num_w + dur_w + 2)).max(20);
                let header_style = Style::default().fg(ratatui::style::Color::Cyan).add_modifier(Modifier::BOLD);
                let mut rows: Vec<ListItem> = Vec::new();
                rows.push(ListItem::new(Line::from(vec![
                    ratatui::text::Span::styled(format!("{:>width$}", "#", width = num_w.saturating_sub(1)), header_style),
                    ratatui::text::Span::styled(format!(" {:width$}", "Song", width = title_w), header_style),
                    ratatui::text::Span::styled(format!(" {:>width$}", "Duration", width = dur_w.saturating_sub(1)), header_style),
                ])));
                for (i, s) in browser.playlist_tracks.iter().enumerate() {
                    let sel = i == browser.playlist_tracks_selected && right_selected;
                    let style = if sel {
                        Style::default().fg(ratatui::style::Color::Black).bg(ROW_HIGHLIGHT_COLOUR)
                    } else {
                        Style::default().fg(TEXT_COLOUR)
                    };
                    let track_no = s.track_no.map_or(String::new(), |n| n.to_string());
                    let artist_str = s.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                    rows.push(ListItem::new(Line::from(vec![
                        ratatui::text::Span::styled(format!("{:>width$}", track_no, width = num_w.saturating_sub(1)), style),
                        ratatui::text::Span::styled(format!(" {}", s.title), style),
                        ratatui::text::Span::styled(format!(" {}", artist_str), ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray)),
                        ratatui::text::Span::styled(format!(" {:>width$}", s.duration_string, width = dur_w.saturating_sub(1)), style),
                    ])));
                }
                let list = List::new(rows)
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
                    let label = format!("{} - {}", a.title, a.artist);
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
pub fn draw_playlist_search_browser(
    f: &mut Frame,
    browser: &mut PlaylistSearchBrowser,
    chunk: Rect,
    selected: bool,
    _cur_tick: u64,
) {
    use super::playlistsearch::InputRouting;
    use super::playlistsearch::search_panel::PlaylistInputRouting;

    let [left_chunk, right_chunk] = Layout::new(
        Direction::Horizontal,
        [Constraint::Percentage(30), Constraint::Percentage(70)],
    ).areas(chunk);

    let left_selected = selected && browser.input_routing == InputRouting::Playlist;
    let right_selected = selected && browser.input_routing == InputRouting::Song;

    // Left panel: playlist search list with optional search box
    let left_playlist_chunk = if browser.playlist_search_panel.route == PlaylistInputRouting::Search {
        let [search_box_chunk, rest_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .areas(left_chunk);
        let search_block = Block::default()
            .title(" Search Playlists ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(SELECTED_BORDER_COLOUR));
        let text_chunk = search_block.inner(search_box_chunk);
        let display = browser.playlist_search_panel.search.search_contents.render_simple("");
        f.render_widget(Clear, search_box_chunk);
        f.render_widget(search_block, search_box_chunk);
        f.render_widget(Paragraph::new(display).style(Style::default().fg(TEXT_COLOUR)), text_chunk);
        rest_chunk
    } else {
        left_chunk
    };

    draw_panel_mut(
        f,
        &mut browser.playlist_search_panel,
        left_playlist_chunk,
        left_selected,
        |t, f, chunk| {
            if t.list.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "No playlists found",
                        Style::default().fg(ratatui::style::Color::DarkGray),
                    )))
                    .alignment(ratatui::layout::Alignment::Center),
                    chunk,
                );
            } else {
                draw_list(f, t, chunk, _cur_tick);
            }
            None
        },
    );

    // Right panel: songs table (always show headers)
    let _ = draw_panel_mut(
        f,
        &mut browser.playlist_songs_panel,
        right_chunk,
        right_selected,
        |t, f, chunk| {
            Some(draw_advanced_table(f, t, chunk, _cur_tick))
        },
    );
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
