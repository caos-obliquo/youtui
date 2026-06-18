use super::YoutuiWindow;
use super::footer::parse_simple_time_to_secs;
use crate::app::media_controls::{MediaControlsStatus, MediaControlsUpdate, MediaControlsVolume};
use crate::app::structures::{AlbumArtState, PlayState};
use itertools::Itertools;
use std::time::Duration;
use tracing::debug;

pub fn draw_app_media_controls(w: &YoutuiWindow) -> MediaControlsUpdate<'_> {
    let mut duration = 0;
    let mut progress = Duration::default();
    match &w.playlist.play_status {
        PlayState::Playing(id) | PlayState::Paused(id) => {
            duration = w
                .playlist
                .get_song_from_id(*id)
                .map(|s| &s.duration_string)
                .map(parse_simple_time_to_secs)
                .unwrap_or(0);
            progress = w.playlist.cur_played_dur.unwrap_or_default();
            if duration == 0 { 0.0 } else { (progress.as_secs_f64() / duration as f64).clamp(0.0, 1.0) }
        }
        _ => 0.0,
    };
    let cur_active_song = match w.playlist.play_status {
        PlayState::Error(id)
        | PlayState::Playing(id)
        | PlayState::Paused(id)
        | PlayState::Buffering(id) => w.playlist.get_song_from_id(id),
        PlayState::NotPlaying | PlayState::Stopped => None,
    };
    let song_title = cur_active_song
        .map(|s| s.title.as_str())
        .unwrap_or_default();
    let album_title = cur_active_song
        .and_then(|s| s.album.as_ref())
        .map(|s| s.name.as_str())
        .unwrap_or_default();
    
    let cover_url = cur_active_song.and_then(|s| {
        if let AlbumArtState::Downloaded(album_art) = &s.album_art {
            debug!("draw_media_controls: using local album art: {:?}", album_art.on_disk_path);
            Some(format!("file://{}", &album_art.on_disk_path.display()))
        } else {
            let thumb = s.thumbnails.iter().max_by_key(|t| t.height * t.width);
            if let Some(t) = thumb {
                debug!("draw_media_controls: using thumbnail URL: {}", t.url);
                Some(t.url.clone())
            } else {
                debug!("draw_media_controls: no thumbnail available");
                None
            }
        }
    });
    let artist_title = cur_active_song
        .map(|s| s.artists.as_ref())
        .map(|s| {
            Itertools::intersperse(s.iter().map(|s| s.name.as_str()), ", ").collect::<String>()
        })
        .unwrap_or("".to_string())
        .into();
    let playback_status = match w.playlist.play_status {
        PlayState::Playing(_) => MediaControlsStatus::Playing { progress },
        PlayState::Paused(_) => MediaControlsStatus::Paused { progress },
        _ => MediaControlsStatus::Stopped,
    };
    let volume = MediaControlsVolume::from_percentage_clamped(w.playlist.volume);
    MediaControlsUpdate {
        title: Some(song_title.into()),
        album: Some(album_title.into()),
        artist: Some(artist_title),
        cover_url: cover_url.map(Into::into),
        duration: Some(std::time::Duration::from_secs(duration as u64)),
        playback_status,
        volume,
    }
}
