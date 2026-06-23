use crate::app::structures::{AlbumArtState, PlayState};
use crate::drawutils::{
    BUTTON_BG_COLOUR, BUTTON_FG_COLOUR, PROGRESS_BG_COLOUR, PROGRESS_FG_COLOUR, middle_of_rect,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui_image::Image;
use ratatui_image::picker::Picker;
use std::time::Duration;

pub const ALBUM_ART_WIDTH: u16 = 7;

pub fn parse_simple_time_to_secs<S: AsRef<str>>(time_string: S) -> usize {
    time_string
        .as_ref()
        .rsplit(':')
        .flat_map(|n| n.parse::<usize>().ok())
        .zip([1, 60, 3600])
        .fold(0, |acc, (time, multiplier)| acc + time * multiplier)
}

pub fn secs_to_time_string(secs: usize) -> String {
    // Naive implementation
    let hours = secs / 3600;
    let rem_mins = (secs - (hours * 3600)) / 60;
    let rem_secs = secs - (hours * 3600 + rem_mins * 60);
    if hours > 0 {
        format!("{hours}:{rem_mins:02}:{rem_secs:02}")
    } else {
        format!("{rem_mins:02}:{rem_secs:02}")
    }
}

pub fn draw_footer(
    f: &mut Frame,
    w: &mut super::YoutuiWindow,
    chunk: Rect,
    terminal_image_capabilities: &Picker,
) {
    let mut duration = 0;
    let mut progress = Duration::default();
    let play_ratio = match &w.playlist.play_status {
        PlayState::Playing(id) | PlayState::Paused(id) => {
            duration = w
                .playlist
                .get_song_from_id(*id)
                .map(|s| &s.duration_string)
                .map(parse_simple_time_to_secs)
                .unwrap_or(0);
            progress = w.playlist.cur_played_dur.unwrap_or_default();
            if duration == 0 { 0.0 }
            else { (progress.as_secs_f64() / duration as f64).clamp(0.0, 1.0) }
        }
        _ => 0.0,
    };
    let progress_str = secs_to_time_string(progress.as_secs() as usize);
    let duration_str = secs_to_time_string(duration);
    let bar_str = format!("{progress_str}/{duration_str}");

    let cur_active_song = match w.playlist.play_status {
        PlayState::Error(id)
        | PlayState::Playing(id)
        | PlayState::Paused(id)
        | PlayState::Buffering(id) => w.playlist.get_song_from_id(id),
        PlayState::NotPlaying | PlayState::Stopped => None,
    };
    let song_and_artists_string = cur_active_song
        .map(|song| {
            let mut s = w.playlist.play_status.list_icon().to_string();
            s.push(' ');
            for (i, artist) in song.artists.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&artist.name);
            }
            s.push_str(" - ");
            s.push_str(&song.title);
            if let Some(album) = song.album.as_ref() {
                let name = album.name.strip_prefix("Album: ").unwrap_or(&album.name);
                if !name.is_empty() {
                    s.push_str(" - ");
                    s.push_str(name);
                }
            }
            s
        })
        .unwrap_or_default();
    let repeat_icon = match w.playlist.repeat_mode {
        crate::app::structures::RepeatMode::All => " ↺",
        crate::app::structures::RepeatMode::One => " ↻₁",
        _ => "",
    };
    let radio_icon = if w.playlist.radio_mode { " ↻" } else { "" };
    let shuffle_icon = if w.playlist.shuffle_enabled { " ⇄" } else { "" };
    let scrobble_indicator = if w.playlist.scrobbling_config.enabled {
        if w.playlist.scrobble_state.is_some() { " [Scrobble]" } else { " [s]" }
    } else { "" };
    let album_art = cur_active_song.map(|s| &s.album_art);
    let last_art = w.last_album_art.clone();
    let footer = Paragraph::new(Line::from(format!(
        "{}{}{}{}{}",
        song_and_artists_string,
        repeat_icon,
        radio_icon,
        shuffle_icon,
        scrobble_indicator,
    )));
    let bar = Gauge::default()
        .label(bar_str)
        .gauge_style(
            Style::default()
                .fg(PROGRESS_FG_COLOUR)
                .bg(PROGRESS_BG_COLOUR),
        )
        .ratio(play_ratio);
    let left_arrow = Paragraph::new(Line::from(vec![
        Span::styled(
            "< [",
            Style::new()
                .fg(BUTTON_FG_COLOUR)
                .bg(BUTTON_BG_COLOUR)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]));
    let right_arrow = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "] >",
            Style::new()
                .fg(BUTTON_FG_COLOUR)
                .bg(BUTTON_BG_COLOUR)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    let block = Block::default()
        .title("Status")
        .title(Line::from("Youtui").right_aligned())
        .borders(Borders::ALL);
    let block_inner = block.inner(chunk);
    let get_progress_bar_and_text_layout = |r: Rect| {
        let [song_text_chunk, progress_bar_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(2), Constraint::Max(1)])
            .areas(r);
        (
            song_text_chunk,
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Max(4), Constraint::Min(1), Constraint::Max(4)])
                .areas(progress_bar_chunk),
        )
    };
    let [album_art_chunk, _, progress_bar_chunk] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(ALBUM_ART_WIDTH),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(block_inner);
    match album_art {
        Some(AlbumArtState::Downloaded(album_art)) => {
            w.last_album_art = Some(album_art.clone());
            // Center the image within album_art_chunk using Fit(None)
            let image = terminal_image_capabilities.new_protocol(
                album_art.in_mem_image.clone(),
                album_art_chunk,
                ratatui_image::Resize::Fit(None),
            );
            match image {
                Ok(protocol) => {
                    f.render_widget(Image::new(&protocol), album_art_chunk);
                    w.sixel_rect = Some(album_art_chunk);
                    if let ratatui_image::protocol::Protocol::Sixel(ref sixel) = protocol {
                        w.sixel_data = Some(sixel.data.clone());
                    } else {
                        w.sixel_data = None;
                    }
                }
                Err(_) => {
                    w.sixel_data = None;
                    let fallback_album_widget = Paragraph::new("").centered();
                    f.render_widget(fallback_album_widget, middle_of_rect(album_art_chunk));
                }
            }
        }
        Some(AlbumArtState::Error) => {
            w.sixel_data = None;
            let fallback_album_widget = Paragraph::new("").centered();
            f.render_widget(fallback_album_widget, middle_of_rect(album_art_chunk));
        }
        _ => {
            w.sixel_data = None;
            if let Some(last) = &last_art {
                let image = terminal_image_capabilities.new_protocol(
                    last.in_mem_image.clone(),
                    Rect {
                        x: 0,
                        y: 0,
                        width: ALBUM_ART_WIDTH,
                        height: ALBUM_ART_WIDTH - 1,
                    },
                    ratatui_image::Resize::Fit(None),
                );
                match image {
                    Ok(protocol) => {
                        f.render_widget(Image::new(&protocol), album_art_chunk);
                        w.sixel_rect = Some(album_art_chunk);
                        if let ratatui_image::protocol::Protocol::Sixel(ref sixel) = protocol {
                            w.sixel_data = Some(sixel.data.clone());
                        } else {
                            w.sixel_data = None;
                        }
                    }
                    Err(_) => {
                        let fallback_album_widget = Paragraph::new("").centered();
                        f.render_widget(fallback_album_widget, middle_of_rect(album_art_chunk));
                    }
                }
            } else {
                let fallback_album_widget = Paragraph::new(" ").centered();
                f.render_widget(fallback_album_widget, middle_of_rect(album_art_chunk));
            }
        }
    };
    let (song_text_chunk, [left_arrow_chunk, mid_bar_chunk, right_arrow_chunk]) =
        get_progress_bar_and_text_layout(progress_bar_chunk);
    f.render_widget(bar, mid_bar_chunk);
    f.render_widget(left_arrow, left_arrow_chunk);
    f.render_widget(right_arrow, right_arrow_chunk);
    f.render_widget(block, chunk);
    f.render_widget(footer, song_text_chunk);
}
