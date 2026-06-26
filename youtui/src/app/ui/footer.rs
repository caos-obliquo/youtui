use crate::app::structures::{AlbumArtState, PlayState};
use crate::drawutils::{
    BUTTON_BG_COLOUR, BUTTON_FG_COLOUR, PROGRESS_BG_COLOUR, PROGRESS_FG_COLOUR, middle_of_rect,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui_image::Image;
use ratatui_image::picker::Picker;
use std::time::Duration;

pub fn parse_simple_time_to_secs<S: AsRef<str>>(time_string: S) -> usize {
    time_string
        .as_ref()
        .rsplit(':')
        .flat_map(|n| n.parse::<usize>().ok())
        .zip([1, 60, 3600])
        .fold(0, |acc, (time, multiplier)| acc + time * multiplier)
}

pub fn like_icon(status: ytmapi_rs::common::LikeStatus) -> &'static str {
    match status {
        ytmapi_rs::common::LikeStatus::Liked => "  󰋑",
        _ => "  ♥",
    }
}

pub const ALBUM_ART_WIDTH: u16 = 7;

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
    let play_button_label = match &w.playlist.play_status {
        PlayState::Playing(_) => " ⏸ ",
        _ => " ▶ ",
    };
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
    let (song_artist_line, album_line) = cur_active_song
        .map(|song| {
            let icon = w.playlist.play_status.list_icon().to_string();
            let mut artist_song = String::new();
            for (i, artist) in song.artists.iter().enumerate() {
                if i > 0 { artist_song.push_str(", "); }
                artist_song.push_str(&artist.name);
            }
            artist_song.push_str(" - ");
            artist_song.push_str(&song.title);
            let album = song.album.as_ref()
                .map(|a| a.name.strip_prefix("Album: ").unwrap_or(&a.name).to_string())
                .filter(|n| !n.is_empty());
            (format!("{} {}", icon, artist_song), album.unwrap_or_default())
        })
        .unwrap_or_default();
    let repeat_icon = match w.playlist.repeat_mode {
        crate::app::structures::RepeatMode::All => " \u{F0456}",
        crate::app::structures::RepeatMode::One => " \u{F0458}",
        _ => " \u{F0457}",
    };
    let radio_icon = if w.playlist.radio_mode { " \u{F0456}" } else { "" };
    let shuffle_icon = if w.playlist.shuffle_enabled { " \u{F049D}" } else { "" };
    let scrobble_indicator = if w.playlist.scrobbling_config.enabled {
        if w.playlist.scrobble_state.is_some() { " [Scrobble]" } else { " [s]" }
    } else { "" };
    let album_art = cur_active_song.map(|s| &s.album_art);
    let heart = cur_active_song.map(|s| like_icon(s.like_status.clone())).unwrap_or("");
    let bar = Gauge::default()
        .label(bar_str)
        .gauge_style(
            Style::default()
                .fg(PROGRESS_FG_COLOUR)
                .bg(PROGRESS_BG_COLOUR),
        )
        .ratio(play_ratio);
    let play_pause = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            play_button_label,
            Style::new()
                .fg(BUTTON_FG_COLOUR)
                .bg(BUTTON_BG_COLOUR)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]));
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
    let [album_art_chunk, _, right_area] = Layout::horizontal([
        Constraint::Length(ALBUM_ART_WIDTH),
        Constraint::Length(1),
        Constraint::Min(0),
    ]).areas(block_inner);
    fn render_album_protocol(
        f: &mut Frame,
        w: &mut super::YoutuiWindow,
        album_art_chunk: Rect,
        protocol: &ratatui_image::protocol::Protocol,
    ) {
        f.render_widget(Image::new(protocol), album_art_chunk);
        w.sixel_rect = Some(album_art_chunk);
        if let ratatui_image::protocol::Protocol::Sixel(sixel) = protocol {
            w.sixel_data = Some(sixel.data.clone());
        } else {
            w.sixel_data = None;
        }
    }
    fn encode_album_protocol(
        album_art_chunk: Rect,
        img: image::DynamicImage,
        terminal_image_capabilities: &Picker,
    ) -> Option<ratatui_image::protocol::Protocol> {
        match terminal_image_capabilities.new_protocol(img, album_art_chunk, ratatui_image::Resize::Fit(None)) {
            Ok(p) => Some(p),
            Err(_) => None,
        }
    }
    match album_art {
        Some(AlbumArtState::Downloaded(album_art)) => {
            let art_changed = w.last_album_art.as_ref()
                .map_or(true, |last| !std::rc::Rc::ptr_eq(last, album_art));
            w.last_album_art = Some(album_art.clone());
            if art_changed || w.cached_album_protocol.is_none() {
                if let Some(protocol) = encode_album_protocol(
                    album_art_chunk, album_art.in_mem_image.clone(), terminal_image_capabilities,
                ) {
                    w.cached_album_protocol = Some(protocol.clone());
                    render_album_protocol(f, w, album_art_chunk, &protocol);
                } else {
                    w.sixel_data = None;
                    f.render_widget(Paragraph::new("").centered(), middle_of_rect(album_art_chunk));
                }
            } else if let Some(protocol) = w.cached_album_protocol.take() {
                render_album_protocol(f, w, album_art_chunk, &protocol);
                w.cached_album_protocol = Some(protocol);
            } else {
                w.sixel_data = None;
            }
        }
        Some(AlbumArtState::Error) => {
            w.sixel_data = None;
            w.cached_album_protocol = None;
            f.render_widget(Paragraph::new("").centered(), middle_of_rect(album_art_chunk));
        }
        _ => {
            w.sixel_data = None;
            if let Some(cached) = w.cached_album_protocol.take() {
                render_album_protocol(f, w, album_art_chunk, &cached);
                w.cached_album_protocol = Some(cached);
            } else if let Some(ref last) = w.last_album_art {
                if let Some(protocol) = encode_album_protocol(
                    album_art_chunk, last.in_mem_image.clone(), terminal_image_capabilities,
                ) {
                    w.cached_album_protocol = Some(protocol.clone());
                    render_album_protocol(f, w, album_art_chunk, &protocol);
                } else {
                    f.render_widget(Paragraph::new("").centered(), middle_of_rect(album_art_chunk));
                }
            } else {
                f.render_widget(Paragraph::new(" ").centered(), middle_of_rect(album_art_chunk));
            }
        }
    };
    let [line1, album_icons_line, bar_chunk] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ]).areas(right_area);
    let [left_arrow_chunk, play_pause_chunk, mid_bar_chunk, right_arrow_chunk] = Layout::horizontal([
        Constraint::Max(4),
        Constraint::Max(4),
        Constraint::Min(1),
        Constraint::Max(4),
    ]).areas(bar_chunk);
    f.render_widget(bar, mid_bar_chunk);
    f.render_widget(left_arrow, left_arrow_chunk);
    f.render_widget(play_pause, play_pause_chunk);
    f.render_widget(right_arrow, right_arrow_chunk);
    f.render_widget(Paragraph::new(Line::from(song_artist_line)), line1);
    let status_prefix = format!("{} {}{}{}", scrobble_indicator, repeat_icon, radio_icon, shuffle_icon);
    let mut album_spans = Vec::new();
    if !album_line.is_empty() {
        let avail = album_icons_line.width.saturating_sub(3) as usize;
        if album_line.len() > avail {
            let mut s = format!("   {}", &album_line[..avail.saturating_sub(6).max(1)]);
            s.push_str("...");
            album_spans.push(Span::styled(s, Style::default().fg(Color::DarkGray)));
        } else {
            album_spans.push(Span::styled(
                format!("   {}", album_line),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
    album_spans.push(Span::raw(status_prefix));
    album_spans.push(Span::styled(heart, Style::default().fg(Color::Red)));
    f.render_widget(Paragraph::new(Line::from(album_spans)), album_icons_line);
    f.render_widget(block, chunk);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_icon_liked() {
        assert_eq!(like_icon(ytmapi_rs::common::LikeStatus::Liked), "  󰋑");
    }

    #[test]
    fn like_icon_indifferent() {
        assert_eq!(like_icon(ytmapi_rs::common::LikeStatus::Indifferent), "  ♥");
    }

    #[test]
    fn like_icon_disliked() {
        assert_eq!(like_icon(ytmapi_rs::common::LikeStatus::Disliked), "  ♥");
    }
}
