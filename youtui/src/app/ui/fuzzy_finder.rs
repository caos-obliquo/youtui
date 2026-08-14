use crate::app::structures::fuzzy_match;
use crate::app::ui::WindowContext;
use crate::app::view::HasTabs;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use vi_text_editor::ViTextEditor;

/// A single fuzzy-finder entry: display label plus a jump target.
#[derive(Clone)]
pub struct FuzzyEntry {
    pub label: String,
    pub kind: FuzzyKind,
}

/// What jumping to this entry should do.
#[derive(Clone, Copy, PartialEq)]
pub enum FuzzyKind {
    /// Set the playlist cursor to this visual index.
    Playlist(usize),
    /// Select a browser item: (tab idx, item idx).
    Browser(usize, usize),
    /// Open a browser tab (0..4).
    OpenTab(usize),
}

/// Header-spawned fuzzy finder (neovim-style `/`).
pub struct FuzzyFinder {
    pub editor: ViTextEditor,
    pub entries: Vec<FuzzyEntry>,
    /// Indices into `entries` that match the current query, best first.
    pub matches: Vec<usize>,
    pub cur: usize,
    pub shown: bool,
}

impl FuzzyFinder {
    pub fn new() -> Self {
        Self {
            editor: ViTextEditor::new(),
            entries: Vec::new(),
            matches: Vec::new(),
            cur: 0,
            shown: false,
        }
    }

    pub fn open(&mut self) {
        self.shown = true;
        self.editor = ViTextEditor::new();
        self.entries.clear();
        self.matches.clear();
        self.cur = 0;
    }

    pub fn close(&mut self) {
        self.shown = false;
        self.entries.clear();
        self.matches.clear();
        self.cur = 0;
    }

    pub fn query(&self) -> &str {
        self.editor.get_text()
    }

    /// Set the corpus to search against, then recompute matches.
    pub fn set_entries(&mut self, entries: Vec<FuzzyEntry>) {
        self.entries = entries;
        self.recompute();
    }

    /// Recompute fuzzy matches for the current query.
    pub fn recompute(&mut self) {
        let q = self.query().trim();
        if q.is_empty() {
            // Show everything (capped) in original order.
            self.matches = (0..self.entries.len().min(50)).collect();
        } else {
            let mut scored: Vec<(u64, usize)> = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| fuzzy_match(q, &e.label).map(|score| (score, i)))
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            self.matches = scored.into_iter().map(|(_, i)| i).take(50).collect();
        }
        self.cur = 0;
    }

    pub fn move_selection(&mut self, amount: isize) {
        if self.matches.is_empty() {
            return;
        }
        let max = self.matches.len() as isize - 1;
        self.cur = (self.cur as isize + amount).clamp(0, max) as usize;
    }

    pub fn selected(&self) -> Option<&FuzzyEntry> {
        self.matches.get(self.cur).map(|&i| &self.entries[i])
    }
}

/// Draw the fuzzy finder as a header-height overlay: one line for the query,
/// then a dropdown of matches.
pub fn draw_fuzzy_finder(f: &mut Frame, finder: &mut FuzzyFinder, chunk: Rect) {
    let shown_count = finder.matches.len();
    let input_height = 3u16;
    let dropdown_height = (shown_count as u16).min(10);
    let total = input_height + dropdown_height;

    let [input_chunk, list_chunk] = Layout::vertical([
        Constraint::Length(input_height),
        Constraint::Length(dropdown_height),
    ])
    .areas(Rect {
        x: chunk.x,
        y: chunk.y,
        width: chunk.width,
        height: total.min(chunk.height),
    });

    // Input box
    let block = Block::default()
        .title(" / ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(input_chunk);
    f.render_widget(Clear, input_chunk);
    f.render_widget(block, input_chunk);
    let display = finder.editor.render_simple("/");
    f.render_widget(
        Paragraph::new(display).style(Style::default().fg(Color::Cyan)),
        inner,
    );
    f.set_cursor_position((inner.x + finder.editor.cursor as u16, inner.y));

    // Match list
    if shown_count > 0 {
        let mut list_state = ListState::default().with_selected(Some(finder.cur));
        let items: Vec<ListItem> = finder
            .matches
            .iter()
            .take(10)
            .map(|&i| {
                let e = &finder.entries[i];
                ListItem::new(Line::from(Span::raw(&e.label)))
            })
            .collect();
        let list = List::new(items)
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
        f.render_stateful_widget(list, list_chunk, &mut list_state);
    }
}

/// Build the fuzzy-finder corpus for the current window context.
/// Browser tab indices: 0=Artists, 1=Albums, 2=Songs, 3=Playlists, 4=Library.
pub fn build_corpus(
    context: WindowContext,
    window: &crate::app::ui::YoutuiWindow,
) -> Vec<FuzzyEntry> {
    let mut entries = Vec::new();
    match context {
        WindowContext::Playlist => {
            let p = &window.playlist;
            let list_len = if !p.search_text.is_empty() {
                p.search_indices_len()
            } else {
                p.list.get_list_iter().len()
            };
            for visual_i in 0..list_len {
                let actual_i = p.visual_to_actual_index(visual_i);
                let Some(song) = p.list.get_song_from_idx(actual_i) else {
                    continue;
                };
                let artist = song
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                entries.push(FuzzyEntry {
                    label: format!("{artist} - {}", song.title),
                    kind: FuzzyKind::Playlist(visual_i),
                });
            }
        }
        WindowContext::Browser => {
            let cur_tab = window.browser.selected_tab_idx();
            match cur_tab {
                // Artists
                0 => {
                    let b = &window.browser.artist_browser();
                    for (i, a) in b.artist_search_panel.list.iter().enumerate() {
                        entries.push(FuzzyEntry {
                            label: a.artist.clone(),
                            kind: FuzzyKind::Browser(0, i),
                        });
                    }
                }
                // Albums
                1 => {
                    let b = &window.browser.album_browser();
                    if b.show_tracks {
                        for (i, s) in b.track_list.get_list_iter().enumerate() {
                            let artist = s
                                .artists
                                .iter()
                                .map(|a| a.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ");
                            entries.push(FuzzyEntry {
                                label: format!("{artist} - {}", s.title),
                                kind: FuzzyKind::Browser(1, i),
                            });
                        }
                    } else {
                        for (i, a) in b.albums.iter().enumerate() {
                            entries.push(FuzzyEntry {
                                label: format!("{} - {}", a.album.artist, a.album.title),
                                kind: FuzzyKind::Browser(1, i),
                            });
                        }
                    }
                }
                // Songs
                2 => {
                    let b = &window.browser.song_browser();
                    for (i, s) in b.get_filtered_list_iter().enumerate() {
                        let artist = s
                            .artists
                            .iter()
                            .map(|a| a.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        entries.push(FuzzyEntry {
                            label: format!("{artist} - {}", s.title),
                            kind: FuzzyKind::Browser(2, i),
                        });
                    }
                }
                // Playlists
                3 => {
                    let b = &window.browser.playlist_browser();
                    for (i, p) in b.playlist_search_panel.list.iter().enumerate() {
                        entries.push(FuzzyEntry {
                            label: p.title.clone(),
                            kind: FuzzyKind::Browser(3, i),
                        });
                    }
                }
                // Library
                4 => {
                    use crate::app::ui::browser::library::LibraryCategory;
                    let b = &window.browser.library_browser_ref();
                    match b.category {
                        LibraryCategory::LikedSongs => {
                            for (i, s) in b.song_list.get_list_iter().enumerate() {
                                let artist = s
                                    .artists
                                    .iter()
                                    .map(|a| a.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                entries.push(FuzzyEntry {
                                    label: format!("{artist} - {}", s.title),
                                    kind: FuzzyKind::Browser(4, i),
                                });
                            }
                        }
                        LibraryCategory::Playlists => {
                            for (i, p) in b.playlist_data.iter().enumerate() {
                                entries.push(FuzzyEntry {
                                    label: p.title.clone(),
                                    kind: FuzzyKind::Browser(4, i),
                                });
                            }
                        }
                        LibraryCategory::Artists => {
                            for (i, a) in b.artist_data.iter().enumerate() {
                                entries.push(FuzzyEntry {
                                    label: a.artist.clone(),
                                    kind: FuzzyKind::Browser(4, i),
                                });
                            }
                        }
                        LibraryCategory::Albums => {
                            for (i, a) in b.album_data.iter().enumerate() {
                                entries.push(FuzzyEntry {
                                    label: format!("{} - {}", a.artist, a.title),
                                    kind: FuzzyKind::Browser(4, i),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    entries
}
