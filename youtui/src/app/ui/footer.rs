use crate::app::structures::PlayState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::picker::Picker;

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
        ytmapi_rs::common::LikeStatus::Liked => " \u{EC14}",
        ytmapi_rs::common::LikeStatus::Disliked => " \u{EC13}",
        _ => "",
    }
}

/// Single-line footer: [play icon] artist - title · album [status] [like] and
/// volume right-aligned in the block title.
pub fn draw_footer(
    f: &mut Frame,
    w: &mut super::YoutuiWindow,
    chunk: Rect,
    _terminal_image_capabilities: &Picker,
) {
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
                artist_song.push_str(&crate::app::structures::normalize_artist_name(&artist.name));
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
    let heart = cur_active_song
        .map(|s| like_icon(s.like_status.clone()))
        .unwrap_or("");
    // Nerd Font MDI volume icons: mute / low / medium / high (footer MDI exception).
    let volume_icon = match w.playlist.volume.0 {
        0 => "\u{f075f}",
        1..=33 => "\u{f057f}",
        34..=66 => "\u{f0580}",
        _ => "\u{f057e}",
    };
    let volume_pct = format!("{} {}%", volume_icon, w.playlist.volume.0);
    let block = Block::default()
        .title("Status")
        .title(Line::from(volume_pct).right_aligned())
        .borders(Borders::ALL);
    let block_inner = block.inner(chunk);
    let [footer_line] = Layout::vertical([Constraint::Length(1)]).areas(block_inner);
    let status_prefix = format!(" {} {}{}{}", scrobble_indicator, repeat_icon, radio_icon, shuffle_icon);
    let mut song_spans: Vec<Span> = Vec::new();
    song_spans.push(Span::raw(song_artist_line));
    if !album_line.is_empty() {
        song_spans.push(Span::styled(
            format!(" · {album_line}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    song_spans.push(Span::raw(status_prefix));
    if !heart.is_empty() {
        song_spans.push(Span::raw(heart));
    }
    f.render_widget(Paragraph::new(Line::from(song_spans)), footer_line);
    f.render_widget(block, chunk);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_icon_liked() {
        assert_eq!(like_icon(ytmapi_rs::common::LikeStatus::Liked), " \u{EC14}");
    }

    #[test]
    fn like_icon_indifferent() {
        assert_eq!(like_icon(ytmapi_rs::common::LikeStatus::Indifferent), "");
    }

    #[test]
    fn like_icon_disliked() {
        assert_eq!(like_icon(ytmapi_rs::common::LikeStatus::Disliked), " \u{EC13}");
    }

    #[test]
    fn parse_time() {
        assert_eq!(parse_simple_time_to_secs("1:30"), 90);
        assert_eq!(parse_simple_time_to_secs("1:02:03"), 3723);
        assert_eq!(parse_simple_time_to_secs("0:00"), 0);
    }
}
