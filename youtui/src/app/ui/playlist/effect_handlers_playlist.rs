use crate::app::component::actionhandler::{ComponentEffect, YoutuiEffect};
use crate::app::server::ValidatedMetadata;

use crate::app::server::{
    ArcServer, TaskMetadata, AddSongsToPlaylist, EnrichRelatedTracks, RemovePlaylistItems, ValidateMetadata,
    EnrichedPlaylistTracks,
};
use crate::app::structures::{AlbumOrUploadAlbumID, ListSong, ListSongID, ListSongArtist, MaybeRc, ListSongAlbum};
use crate::app::structures::{AlbumArtState, DownloadStatus};
use crate::app::ui::playlist::Playlist;
use crate::app::ui::playlist::lyrics_popup::LyricsPopup;
use crate::app::ui::playlist::playlist_update_popup::{PlaylistUpdatePopup, PlaylistUpdatePopupState};
use crate::app::ui::playlist::playlist_details_popup::PlaylistDetailsPopup;
use async_callback_manager::{AsyncTask, FrontendEffect};
use std::rc::Rc;
use tracing::{debug, error, info, warn};
use ytmapi_rs::common::{AlbumID, PlaylistID, VideoID, SetVideoID, YoutubeID};
use ytmapi_rs::parse::LibraryPlaylist;
use ytmapi_rs::parse::PlaylistSong;
use ytmapi_rs::parse::WatchPlaylistTrack;

/// Generate a CRUD OK handler: logs, sets `library_playlist_mutated = true`.
macro_rules! playlist_ok_handler {
    ($name:ident, $log:literal) => {
        impl_youtui_task_handler!($name, (), Playlist, |_, _: ()| {
            |this: &mut Playlist| {
                info!($log);
                this.library_playlist_mutated = true;
                AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
            }
        });
    };
}

/// Generate a CRUD error handler: logs error, sets `last_error`.
macro_rules! playlist_err_handler {
    ($name:ident, $op:literal, $label:literal) => {
        impl_youtui_task_handler!($name, anyhow::Error, Playlist, |_, err: anyhow::Error| {
            let msg = err.to_string();
            move |this: &mut Playlist| {
                error!("Failed to {}: {}", $op, msg);
                this.last_error = Some(format!("{}: {}", $label, msg));
                AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
            }
        });
    };
}

#[derive(Debug, PartialEq)]
pub struct HandleRateSongOk;
#[derive(Debug, PartialEq)]
pub struct HandleRateSongErr;
#[derive(Debug, PartialEq)]
pub struct HandleCreatePlaylistOk;
#[derive(Debug, PartialEq)]
pub struct HandleCreatePlaylistError;
#[derive(Debug, PartialEq)]
pub struct HandleAddSongsOk;
#[derive(Debug, PartialEq)]
pub struct HandleAddSongsError;
#[derive(Debug, PartialEq)]
pub struct HandleGetAllLibraryPlaylistsOk;
#[derive(Debug, PartialEq)]
pub struct HandleGetAllLibraryPlaylistsError;
#[derive(Debug, PartialEq)]
pub struct HandleDeletePlaylistOk;
#[derive(Debug, PartialEq)]
pub struct HandleDeletePlaylistError;
#[derive(Debug, PartialEq)]
pub struct HandleEditPlaylistDetailsOk;
#[derive(Debug, PartialEq)]
pub struct HandleEditPlaylistDetailsError;
#[derive(Debug, PartialEq)]
pub struct HandleRatePlaylistOk;
#[derive(Debug, PartialEq)]
pub struct HandleRatePlaylistError;
#[derive(Debug, PartialEq)]
pub struct HandleRenamePlaylistOk;
#[derive(Debug, PartialEq)]
pub struct HandleRenamePlaylistError;

#[derive(Debug, PartialEq)]
pub struct HandleOverwriteGetTracks(pub PlaylistID<'static>, pub Vec<VideoID<'static>>);
#[derive(Debug, PartialEq)]
pub struct HandleOverwriteGetTracksErr;
#[derive(Debug, PartialEq)]
pub struct HandleOverwriteRemoveDone(pub PlaylistID<'static>, pub Vec<VideoID<'static>>);
#[derive(Debug, PartialEq)]
pub struct HandleOverwriteRemoveDoneErr;

#[derive(Debug, PartialEq)]
pub struct HandleFetchPlaylistDetailsOk;
#[derive(Debug, PartialEq)]
pub struct HandleFetchPlaylistDetailsError;

#[derive(Debug, PartialEq)]
pub enum PlaylistDetailsEffect {
    DetailsFetched(ytmapi_rs::parse::GetPlaylistDetails),
    FetchError(String),
}

impl FrontendEffect<PlaylistDetailsPopup, ArcServer, TaskMetadata> for PlaylistDetailsEffect {
    fn apply(self, target: &mut PlaylistDetailsPopup) -> impl Into<ComponentEffect<PlaylistDetailsPopup>> {
        match self {
            PlaylistDetailsEffect::DetailsFetched(details) => {
                target.loaded = true;
                target.details = Some(details);
            }
            PlaylistDetailsEffect::FetchError(msg) => {
                target.loaded = true;
                target.error = Some(msg);
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleFetchPlaylistDetailsOk,
    ytmapi_rs::parse::GetPlaylistDetails,
    PlaylistDetailsPopup,
    |_, details: ytmapi_rs::parse::GetPlaylistDetails| {
        PlaylistDetailsEffect::DetailsFetched(details)
    }
);

impl_youtui_task_handler!(
    HandleFetchPlaylistDetailsError,
    anyhow::Error,
    PlaylistDetailsPopup,
    |_, err: anyhow::Error| {
        PlaylistDetailsEffect::FetchError(err.to_string())
    }
);

#[derive(Debug, PartialEq)]
pub enum PlaylistEffect {
    CreatePlaylistSuccess(PlaylistID<'static>),
    CreatePlaylistError,
    AddSongsSuccess,
    AddSongsError,
}

impl_youtui_task_handler!(
    HandleCreatePlaylistOk,
    PlaylistID<'static>,
    Playlist,
    |_, playlist_id: PlaylistID<'static>| {
        PlaylistEffect::CreatePlaylistSuccess(playlist_id)
    }
);

impl_youtui_task_handler!(
    HandleCreatePlaylistError,
    anyhow::Error,
    Playlist,
    |_, error: anyhow::Error| {
        error!("Failed to create playlist: {}", error);
        PlaylistEffect::CreatePlaylistError
    }
);
impl_youtui_task_handler!(
    HandleAddSongsOk,
    (),
    Playlist,
    |_, _| {
        info!("Successfully added songs to playlist!");
        PlaylistEffect::AddSongsSuccess
    }
);
playlist_ok_handler!(HandleRateSongOk, "Song rated successfully");

impl_youtui_task_handler!(
    HandleRateSongErr,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("Failed to rate song: {}", msg);
            // Clear the pending rating on error
            this.last_rated_video_id = None;
            this.last_rated_like_status = None;
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleAddSongsError,
    anyhow::Error,
    Playlist,
    |_, error: anyhow::Error| {
        error!("Error adding songs to playlist: {}", error);
        PlaylistEffect::AddSongsError
    }
);

playlist_ok_handler!(HandleDeletePlaylistOk, "Playlist deleted successfully");
playlist_err_handler!(HandleDeletePlaylistError, "delete playlist", "Delete failed");
playlist_ok_handler!(HandleEditPlaylistDetailsOk, "Playlist details updated");
playlist_err_handler!(HandleEditPlaylistDetailsError, "update playlist details", "Edit failed");
playlist_ok_handler!(HandleRatePlaylistOk, "Playlist rated successfully");
playlist_err_handler!(HandleRatePlaylistError, "rate playlist", "Rate failed");
playlist_ok_handler!(HandleRenamePlaylistOk, "Playlist renamed successfully");
playlist_err_handler!(HandleRenamePlaylistError, "rename playlist", "Rename failed");



impl_youtui_task_handler!(
    HandleOverwriteGetTracks,
    EnrichedPlaylistTracks,
    Playlist,
    |this: HandleOverwriteGetTracks, tracks: EnrichedPlaylistTracks| {
        let songs = tracks.songs;
        move |_target: &mut Playlist| {
            let set_ids: Vec<SetVideoID<'static>> = songs.iter()
                .map(|s| SetVideoID::from_raw(s.video_id.get_raw().to_string()))
                .collect();
            if set_ids.is_empty() {
                info!("Overwrite: no tracks to remove, adding directly");
                let add_effect = AsyncTask::new_future_try(
                    AddSongsToPlaylist { playlist_id: this.0, video_ids: this.1 },
                    HandleAddSongsOk,
                    HandleAddSongsError,
                    None,
                );
                return add_effect;
            }
            info!("Overwrite: removing {} old tracks, adding {} new tracks", set_ids.len(), this.1.len());
            let remove_effect = AsyncTask::new_future_try(
                RemovePlaylistItems { playlist_id: this.0.clone(), video_ids: set_ids },
                HandleOverwriteRemoveDone(this.0, this.1),
                HandleOverwriteRemoveDoneErr,
                None,
            );
            remove_effect
        }
    }
);

impl_youtui_task_handler!(
    HandleOverwriteGetTracksErr,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |target: &mut Playlist| {
            error!("Overwrite: failed to fetch playlist tracks: {}", msg);
            target.last_error = Some(format!("Overwrite failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleOverwriteRemoveDone,
    (),
    Playlist,
    |this: HandleOverwriteRemoveDone, _: ()| {
        move |_target: &mut Playlist| {
            info!("Overwrite: old tracks removed, adding new tracks");
            let add_effect = AsyncTask::new_future_try(
                AddSongsToPlaylist { playlist_id: this.0, video_ids: this.1 },
                HandleAddSongsOk,
                HandleAddSongsError,
                None,
            );
            add_effect
        }
    }
);

impl_youtui_task_handler!(
    HandleOverwriteRemoveDoneErr,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |target: &mut Playlist| {
            error!("Overwrite: failed to remove old tracks: {}", msg);
            target.last_error = Some(format!("Overwrite remove failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for PlaylistEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            PlaylistEffect::CreatePlaylistSuccess(playlist_id) => {
                info!("Playlist created: {:?}", playlist_id);
                target.library_playlist_mutated = true;
                // Check if there are more chunks to create (sequential chain)
                if let Some((chunks, ref title, ref description, ref privacy)) = target.pending_playlist_chunks.take() {
                    if let Some((i, chunk)) = chunks.iter().enumerate().next() {
                        let chunk_title = format!("{} pt{}", title, i + 1);
                        let remaining: Vec<Vec<_>> = chunks[i + 1..].to_vec();
                        target.pending_playlist_chunks = if remaining.is_empty() { None } else {
                            Some((remaining, title.clone(), description.clone(), privacy.clone()))
                        };
                        use crate::app::server::CreatePlaylistWithVideos;
                        use crate::app::ui::playlist::effect_handlers_playlist::{
                            HandleCreatePlaylistOk, HandleCreatePlaylistError,
                        };
                        return AsyncTask::new_future_try(
                            CreatePlaylistWithVideos {
                                title: chunk_title,
                                description: description.clone(),
                                video_ids: chunk.clone(),
                                privacy: privacy.clone(),
                            },
                            HandleCreatePlaylistOk,
                            HandleCreatePlaylistError,
                            None,
                        );
                    }
                }
            }
            PlaylistEffect::CreatePlaylistError => {
                error!("Failed to create playlist");
                target.pending_playlist_chunks = None;
            }
            PlaylistEffect::AddSongsSuccess => {
                info!("Successfully added songs to playlist!");
            }
            PlaylistEffect::AddSongsError => {
                error!("Error adding songs to playlist");
            }
        }
        AsyncTask::new_no_op()
    }
}

// PlaylistUpdatePopup effect handlers

#[derive(Debug, PartialEq)]
pub enum PlaylistUpdateEffect {
    FetchPlaylistsSuccess(Vec<LibraryPlaylist>),
    FetchPlaylistsError(String),
}

impl FrontendEffect<PlaylistUpdatePopup, ArcServer, TaskMetadata> for PlaylistUpdateEffect {
    fn apply(self, target: &mut PlaylistUpdatePopup) -> impl Into<ComponentEffect<PlaylistUpdatePopup>> {
        match self {
            PlaylistUpdateEffect::FetchPlaylistsSuccess(playlists) => {
                info!(
                    "Successfully fetched {} library playlists",
                    playlists.len()
                );
                target.state = PlaylistUpdatePopupState::Loaded(playlists);
                target.selected_idx = 0;
                target.refresh_filter();
            }
            PlaylistUpdateEffect::FetchPlaylistsError(msg) => {
                error!("Failed to fetch library playlists: {}", msg);
                target.state = PlaylistUpdatePopupState::Error(msg);
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleGetAllLibraryPlaylistsOk,
    Vec<LibraryPlaylist>,
    PlaylistUpdatePopup,
    |_, playlists: Vec<LibraryPlaylist>| {
        PlaylistUpdateEffect::FetchPlaylistsSuccess(playlists)
    }
);

impl_youtui_task_handler!(
    HandleGetAllLibraryPlaylistsError,
    anyhow::Error,
    PlaylistUpdatePopup,
    |_, error: anyhow::Error| {
        PlaylistUpdateEffect::FetchPlaylistsError(error.to_string())
    }
);

// LyricsPopup effect handlers

#[derive(PartialEq, Debug)]
pub struct HandleGetLyricsOk;
#[derive(PartialEq, Debug)]
pub struct HandleGetLyricsErr;

#[derive(Debug, PartialEq)]
pub enum LyricsEffect {
    FetchLyricsSuccess(String),
    FetchLyricsError(String),
}

impl FrontendEffect<LyricsPopup, ArcServer, TaskMetadata> for LyricsEffect {
    fn apply(self, target: &mut LyricsPopup) -> impl Into<ComponentEffect<LyricsPopup>> {
        match self {
            LyricsEffect::FetchLyricsSuccess(lyrics) => {
                target.set_lyrics(lyrics);
            }
            LyricsEffect::FetchLyricsError(err) => {
                target.set_error(err);
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleGetLyricsOk,
    String,
    LyricsPopup,
    |_, lyrics: String| {
        LyricsEffect::FetchLyricsSuccess(lyrics)
    }
);

impl_youtui_task_handler!(
    HandleGetLyricsErr,
    anyhow::Error,
    LyricsPopup,
    |_, error: anyhow::Error| {
        LyricsEffect::FetchLyricsError(error.to_string())
    }
);

// GetAnnotations effect handlers

#[derive(Debug, PartialEq)]
pub enum AnnotationsEffect {
    FetchAnnotationsSuccess(Vec<(String, String)>),
    FetchAnnotationsError(String),
}

impl FrontendEffect<LyricsPopup, ArcServer, TaskMetadata> for AnnotationsEffect {
    fn apply(self, target: &mut LyricsPopup) -> impl Into<ComponentEffect<LyricsPopup>> {
        match self {
            AnnotationsEffect::FetchAnnotationsSuccess(anns) => {
                let count = anns.len();
                target.set_annotations(anns.into_iter().map(|(f, e)| crate::app::ui::playlist::lyrics_popup::Annotation { fragment: f, explanation: e }).collect());
                info!("AnnotationsEffect: set {} annotations on popup", count);
            }
            AnnotationsEffect::FetchAnnotationsError(err) => {
                tracing::warn!("Annotations fetch error: {}", err);
            }
        }
        AsyncTask::new_no_op()
    }
}

#[derive(PartialEq, Debug)]
pub struct HandleGetAnnotationsOk;
#[derive(PartialEq, Debug)]
pub struct HandleGetAnnotationsErr;

impl_youtui_task_handler!(
    HandleGetAnnotationsOk,
    Vec<(String, String)>,
    LyricsPopup,
    |_, annotations: Vec<(String, String)>| {
        AnnotationsEffect::FetchAnnotationsSuccess(annotations)
    }
);

impl_youtui_task_handler!(
    HandleGetAnnotationsErr,
    anyhow::Error,
    LyricsPopup,
    |_, error: anyhow::Error| {
        AnnotationsEffect::FetchAnnotationsError(error.to_string())
    }
);

// Metadata validation effect handlers

#[derive(Debug, PartialEq)]
pub struct HandleMetadataValidated(pub ListSongID);
#[derive(Debug, PartialEq)]
pub struct HandleMetadataValidationError;

#[derive(Debug, PartialEq)]
pub enum MetadataEffect {
    Validated(ValidatedMetadata, ListSongID),
    ValidationError,
}

/// Apply metadata fields (album, year, artist, track_no, genres, styles) to a song.
/// Returns the original album name before overwriting.
fn apply_metadata_fields<'a>(song: &mut ListSong, data: &'a ValidatedMetadata) -> Option<String> {
    let original_album = song.album.as_ref().map(|a| a.as_ref().name.clone());
    if let Some(ref album) = data.album {
        // Only override album when YTM has none (preserve YTM's album to prevent
        // wrong metadata from overwriting correct data, e.g. Phyllomedusa albums)
        let ytm_empty = song.album.as_ref().map_or(true, |a| {
            a.as_ref().name.is_empty()
                || matches!(&a.as_ref().id, AlbumOrUploadAlbumID::Album(id) if id.get_raw().is_empty())
        });
        if ytm_empty {
            // Preserve original YTM album ID when overwriting album name
            // (e.g. when metadata provider fills in empty YTM album)
            let orig_id = song.album.as_ref()
                .map(|a| a.as_ref().id.clone())
                .unwrap_or(AlbumOrUploadAlbumID::Album(AlbumID::from_raw("")));
            song.album = Some(MaybeRc::Owned(ListSongAlbum {
                name: album.clone(),
                id: orig_id,
            }));
        } else {
            debug!(
                ytm = ?song.album.as_ref().map(|a| a.as_ref().name.as_str()),
                provider = %album,
                "ValidateMetadata: keeping YTM album, skipping provider override"
            );
        }
    }
    if let Some(ref year) = data.year {
        song.year = Some(Rc::new(year.clone()));
    } else if let Some(ref album) = data.album {
        // Fallback: extract year from album name
        if let Some(y) = album.split(|c: char| !c.is_ascii_digit())
            .find(|p| p.len() == 4)
            .and_then(|p| p.parse::<u16>().ok())
            .filter(|y| (1900..2100).contains(y))
        {
            song.year = Some(Rc::new(y.to_string()));
        }
    }
    if let Some(ref artist) = data.artist {
        let normalized = crate::app::structures::normalize_artist_name(artist);
        song.artists = MaybeRc::Owned(vec![
            ListSongArtist {
                name: normalized,
                id: None,
            },
        ]);
    }
    if let Some(tn) = data.track_no {
        song.track_no = Some(tn);
    }
    if !data.genres.is_empty() {
        song.genres = data.genres.clone();
    }
    if !data.styles.is_empty() {
        song.styles = data.styles.clone();
    }
    song.release_mbid = data.musicbrainz_release_group_id.clone();
    original_album
}

/// Handle album split decision: validate duration ratio, tracklist match, insert tracks, fetch album art.
/// Returns an optional chained effect if album split occurs.
/// Guard: should album split be skipped for this track?
/// Returns true when song already has proper YTM track metadata
/// (track_no + album) and is not a channel upload.
pub(crate) fn should_skip_album_split(
    is_album_upload: bool,
    pre_track_no: Option<usize>,
    pre_has_album: bool,
) -> bool {
    !is_album_upload && pre_track_no.is_some() && pre_has_album
}

fn handle_album_split(
    target: &mut Playlist,
    song_id: ListSongID,
    data: &ValidatedMetadata,
    original_album: &Option<String>,
    original_title: &str,
    is_album_upload: bool,
    _song_duration_secs: u64,
    pre_track_no: Option<usize>,
    pre_has_album: bool,
) -> Option<AsyncTask<Playlist, ArcServer, TaskMetadata>> {
    // Layer 2: Internal belt-and-suspenders guard using PRE-mutation state.
    // Even if the caller's needs_split check somehow passes, this catches
    // regular YTM tracks that already have proper track_no and album.
    if should_skip_album_split(is_album_upload, pre_track_no, pre_has_album) {
        info!("Album split rejected (internal guard): song has YTM track metadata");
        target.last_error = Some("Album split rejected: song has YTM track".to_string());
        return None;
    }
    if data.album_tracks.len() < 2 {
        info!("Album split rejected: only {} track(s) from provider", data.album_tracks.len());
        target.last_error = Some("Album split failed: only 1 track in provider data".to_string());
        return None;
    }
    // Track title must appear in album tracklist
    // (skip this check when song is a channel upload album -
    //  the title IS the album name, not a track name)
    if !data.album_tracks.is_empty() && !is_album_upload {
        let title_norm: String = original_title.to_lowercase().chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
        let title_norm = title_norm.trim();
        let track_in_list = data.album_tracks.iter().any(|t| {
            let t_norm: String = t.title.to_lowercase().chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
            let t_norm = t_norm.trim();
            t_norm == title_norm
                || t_norm.contains(title_norm)
                || title_norm.contains(t_norm)
        });
        if !track_in_list {
            info!("Album tracklist rejected: track '{:?}' not found in album tracklist, skipping split to avoid wrong album", original_title);
            target.last_error = Some("Album split failed: track not in provider tracklist".to_string());
            return None;
        }
    }
    // Filter out zero-duration tracks
    let valid_tracks: Vec<_> = data.album_tracks.iter()
        .filter(|t| t.duration_secs > 0.0)
        .cloned()
        .collect();
    if valid_tracks.is_empty() {
        info!("Album tracklist rejected: all {} tracks have zero duration",
            data.album_tracks.len());
        target.last_error = Some("Album split failed: all tracks have zero duration".to_string());
        return None;
    }
    if valid_tracks.len() < data.album_tracks.len() {
        info!("Album tracklist: {} zero-duration tracks filtered out, {} remaining",
            data.album_tracks.len() - valid_tracks.len(), valid_tracks.len());
    }
    target.album_tracks = Some(valid_tracks.clone());
    target.album_current_track = 0;
    let play_effect = target.insert_album_tracks(song_id, &valid_tracks, &data.artist, &data.album, &data.year, original_album);
    info!("Album mode: {} tracks loaded for song {:?}",
        target.album_tracks.as_ref().map_or(0, |t| t.len()), song_id);
    target.last_status = Some(format!("Album split: {} tracks", valid_tracks.len()));

    // Fetch album art from Last.fm
    let api_key = target.scrobbling_config.api_key.clone();
    let mut effect = AsyncTask::new_no_op();
    if !api_key.is_empty() {
        let art_artist = data.artist.clone()
            .or_else(|| target.get_song_from_id(song_id)
                .map(|s| s.artists.iter().map(|a| a.name.as_str())
                    .collect::<Vec<_>>().join(", ")));
        let art_album = data.album.clone()
            .or_else(|| target.get_song_from_id(song_id)
                .and_then(|s| s.album.as_ref().map(|a| a.name.clone())));
        if let (Some(ref aa), Some(ref ab)) = (art_artist, art_album) {
            if !aa.is_empty() && !ab.is_empty() {
                info!("MetadataEffect: triggering FetchAlbumArt for '{}' - '{}'", aa, ab);
                use crate::app::server::FetchAlbumArt;
                let art_task = AsyncTask::new_future_try(
                    FetchAlbumArt(aa.clone(), ab.clone(), api_key.clone(), data.musicbrainz_release_group_id.clone()),
                    HandleFetchAlbumArtOk,
                    HandleFetchAlbumArtErr,
                    None,
                );
                effect = effect.push(art_task);
            } else {
                debug!("MetadataEffect: skip FetchAlbumArt - empty artist or album");
            }
        } else {
            debug!("MetadataEffect: skip FetchAlbumArt - missing artist or album");
        }
    }
    if let Some(e) = play_effect {
        effect = effect.push(e);
    }
    Some(effect)
}

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for MetadataEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            MetadataEffect::Validated(data, song_id) => {
                // Step 1: Apply metadata fields to song (borrows song, not target)
                // Capture pre-mutation track_no and album BEFORE apply_metadata_fields
                // to prevent provider data from defeating the needs_split guard.
                let (original_album, original_title, is_album_upload, song_dur_secs, pre_track_no, pre_has_album) = if let Some(idx) = target.get_index_from_id(song_id) {
                    if let Some(song) = target.list.get_list_iter_mut().nth(idx) {
                        let pre_track_no = song.track_no;
                        let pre_has_album = song.album.is_some();
                        let orig_album = apply_metadata_fields(song, &data);
                        let title = song.title.clone();
                        let upload = song.is_album_upload;
                        let dur = song.actual_duration
                            .map(|d| d.as_secs())
                            .or_else(|| {
                                let parts: Vec<&str> = song.duration_string.split(':').collect();
                                if parts.len() == 2 {
                                    Some(parts[0].parse::<u64>().unwrap_or(0) * 60
                                        + parts[1].parse::<u64>().unwrap_or(0))
                                } else if parts.len() == 3 {
                                    Some(parts[0].parse::<u64>().unwrap_or(0) * 3600
                                        + parts[1].parse::<u64>().unwrap_or(0) * 60
                                        + parts[2].parse::<u64>().unwrap_or(0))
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        info!("Metadata validated for song {:?} (artist={:?}, album={:?}, year={:?}, track={:?}, genres={:?}, styles={:?})",
                            song_id, data.artist, data.album, data.year, data.track_no, data.genres, data.styles);
                        (orig_album, title, upload, dur, pre_track_no, pre_has_album)
                    } else {
                        (None, String::new(), false, 0, None, false)
                    }
                } else {
                    (None, String::new(), false, 0, None, false)
                };
                // Step 2: Album split decision (borrows target, song reference is dropped)
                // Uses PRE-mutation track_no/album to prevent apply_metadata_fields
                // from defeating the guard by overwriting pre-existing YTM data.
                // Duration match: sum(provider_tracks.duration_secs) must approximate
                // song_duration_secs within 20%. Prevents finding wrong album for
                // Liked Songs where track_no=None but provider returns artist's most
                // popular album (e.g., 3min Liked Song matched with 42min 21-track album).
                let duration_ok = !data.album_tracks.is_empty() && song_dur_secs > 0;
                let dur_match = duration_ok && {
                    let provider_sum: u64 = data.album_tracks.iter()
                        .map(|t| t.duration_secs as u64)
                        .sum();
                    let diff = if provider_sum > song_dur_secs {
                        provider_sum - song_dur_secs
                    } else {
                        song_dur_secs - provider_sum
                    };
                    // Allow 20% tolerance: sum within 1.2x of song duration
                    diff <= song_dur_secs / 5 || diff <= 30
                };
                let needs_split = (is_album_upload
                    || pre_track_no.is_none()
                    || !pre_has_album) && dur_match;
                if !data.album_tracks.is_empty() && needs_split {
                    if let Some(effect) = handle_album_split(
                        target, song_id, &data, &original_album, &original_title, is_album_upload, song_dur_secs,
                        pre_track_no, pre_has_album,
                    ) {
                        return effect;
                    }
                }
            }
            MetadataEffect::ValidationError => {
                info!("Metadata validation failed (non-critical)");
                target.last_error = Some("Metadata validation failed (non-critical)".to_string());
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleMetadataValidated,
    ValidatedMetadata,
    Playlist,
    |this: HandleMetadataValidated, metadata: ValidatedMetadata| {
        MetadataEffect::Validated(metadata, this.0)
    }
);

impl_youtui_task_handler!(
    HandleMetadataValidationError,
    anyhow::Error,
    Playlist,
    |_, error: anyhow::Error| {
        error!("Metadata validation error: {}", error);
        MetadataEffect::ValidationError
    }
);

// Album art from Last.fm effect handlers

use crate::app::server::song_thumbnail_downloader::SongThumbnail;
#[derive(Debug, PartialEq)]
pub struct HandleFetchAlbumArtOk;
#[derive(Debug, PartialEq)]
pub struct HandleFetchAlbumArtErr;

#[derive(Debug, PartialEq)]
pub enum FetchAlbumArtEffect {
    Fetched(SongThumbnail, Option<String>),
    FetchError,
}

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for FetchAlbumArtEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
    match self {
            FetchAlbumArtEffect::Fetched(thumbnail, canonical_album) => {
                let thumb_rc = std::rc::Rc::new(thumbnail);
                let album_name = target.album_art_fetching_name.take();
                for song in target.list.get_list_iter_mut() {
                    if matches!(song.album_art, AlbumArtState::None | AlbumArtState::Init)
                        && album_name.as_deref().map_or(true, |name| {
                            song.album.as_ref().map(|a| a.name.as_str()) == Some(name)
                        })
                    {
                        song.album_art = AlbumArtState::Downloaded(thumb_rc.clone());
                    }
                }
                // Store canonical album name for ALL scrobble paths.
                // Last.fm is primary source of truth for album names.
                if let Some(ref canonical) = canonical_album {
                    target.canonical_album_name = Some(canonical.clone());
                    // Also update live scrobble_state if it matches current song.
                    // Compare canonical vs state.album (both cleaned/canonical),
                    // NOT album_name (raw YTM) vs state.album (cleaned).
                    if let Some(ref mut state) = target.scrobble_state {
                        let matches_current = canonical_album.as_deref().zip(state.album.as_deref())
                            .map_or(false, |(c, s)| c == s);
                        if matches_current {
                            info!("FetchAlbumArtEffect: updating scrobble album '{}' -> canonical '{}'",
                                state.album.as_deref().unwrap_or(""), canonical);
                            state.album = Some(canonical.clone());
                        } else {
                            debug!("FetchAlbumArtEffect: skip album update - canonical '{:?}' doesn't match current scrobble_state album '{:?}'",
                                canonical, state.album);
                        }
                    }
                }
                target.album_art_fetching = false;
                info!("FetchAlbumArtEffect: applied art");
            }
            FetchAlbumArtEffect::FetchError => {
                let album_name = target.album_art_fetching_name.take();
                for song in target.list.get_list_iter_mut() {
                    if matches!(song.album_art, AlbumArtState::None | AlbumArtState::Init)
                        && album_name.as_deref().map_or(true, |name| {
                            song.album.as_ref().map(|a| a.name.as_str()) == Some(name)
                        })
                    {
                        song.album_art = AlbumArtState::None;
                    }
                }
                target.album_art_fetching = false;
                error!("FetchAlbumArtEffect: failed to fetch album art");
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleFetchAlbumArtOk,
    (SongThumbnail, Option<String>),
    Playlist,
    |_, (thumbnail, canonical_album): (SongThumbnail, Option<String>)| {
        FetchAlbumArtEffect::Fetched(thumbnail, canonical_album)
    }
);

impl_youtui_task_handler!(
    HandleFetchAlbumArtErr,
    anyhow::Error,
    Playlist,
    |_, _error: anyhow::Error| {
        FetchAlbumArtEffect::FetchError
    }
);

// Song year enrichment effect handlers

#[derive(Debug, PartialEq)]
pub struct HandleEnrichSongYearOk;
#[derive(Debug, PartialEq)]
pub struct HandleEnrichSongYearErr;

#[derive(Debug, PartialEq)]
pub enum EnrichSongYearEffect {
    YearFound(String),
    YearNotFound,
    YearFetchError,
}

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for EnrichSongYearEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            EnrichSongYearEffect::YearFound(year) => {
                if let Some(cur_id) = target.get_cur_playing_id() {
                    if let Some(idx) = target.get_index_from_id(cur_id) {
                        if let Some(song) = target.list.get_list_iter_mut().nth(idx) {
                            song.year = Some(Rc::new(year));
                        }
                    }
                }
                target.year_being_enriched = false;
                info!("EnrichSongYearEffect: applied year to playing song");
            }
            EnrichSongYearEffect::YearNotFound => {
                target.year_being_enriched = false;
                debug!("EnrichSongYearEffect: no year found");
            }
            EnrichSongYearEffect::YearFetchError => {
                target.year_being_enriched = false;
                debug!("EnrichSongYearEffect: year fetch error");
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleEnrichSongYearOk,
    Option<String>,
    Playlist,
    |_, year: Option<String>| {
        match year {
            Some(year) => EnrichSongYearEffect::YearFound(year),
            None => EnrichSongYearEffect::YearNotFound,
        }
    }
);

impl_youtui_task_handler!(
    HandleEnrichSongYearErr,
    anyhow::Error,
    Playlist,
    |_, _error: anyhow::Error| {
        EnrichSongYearEffect::YearFetchError
    }
);

// Queue batch year enrichment handlers
#[derive(Debug, PartialEq)]
pub struct HandleQueueEnrichYearsOk;
#[derive(Debug, PartialEq)]
pub struct HandleQueueEnrichYearsErr;

impl_youtui_task_handler!(
    HandleQueueEnrichYearsOk,
    Vec<(usize, Option<String>, Vec<String>, Vec<String>)>,
    Playlist,
    |_, results: Vec<(usize, Option<String>, Vec<String>, Vec<String>)>| {
        move |this: &mut Playlist| {
            use std::collections::HashMap;
            let year_map: HashMap<usize, Option<String>> = results.into_iter()
                .map(|(idx, year, _, _)| (idx, year))
                .collect();
            let mut applied = 0usize;
            for (i, song) in this.list.get_list_iter_mut().enumerate() {
                if let Some(Some(year)) = year_map.get(&i) {
                    song.year = Some(Rc::new(year.clone()));
                    applied += 1;
                }
            }
            if applied > 0 {
                info!("Queue batch enrichment: applied years to {} songs", applied);
            }
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleQueueEnrichYearsErr,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            warn!("Queue batch year enrichment failed: {}", msg);
            this.last_error = Some(format!("Year enrichment failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

// Playlist load from YouTube Music effect handlers

#[derive(Debug, PartialEq)]
pub struct HandleGetPlaylistTracksOk;
#[derive(Debug, PartialEq)]
pub struct HandleGetPlaylistTracksErr;

#[derive(Debug, PartialEq)]
pub enum LoadPlaylistEffect {
    TracksFetched(Vec<PlaylistSong>),
    TracksAppended(Vec<PlaylistSong>),
    FetchError,
}

fn convert_playlist_songs(songs: Vec<PlaylistSong>) -> Vec<ListSong> {
    let mut list_songs = Vec::with_capacity(songs.len());
    for s in songs {
        let list_artists = MaybeRc::Owned(s.artists.into_iter().map(|a| ListSongArtist {
            name: a.name,
            id: None,
        }).collect());
        let album_name = s.album.name.clone();
        let list_album = Some(MaybeRc::Owned(ListSongAlbum {
            name: album_name.clone(),
            id: AlbumOrUploadAlbumID::Album(s.album.id),
        }));
        let year = album_name.split('(').last().and_then(|s| s.get(..4))
            .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
            .map(|y| y.to_string());
        list_songs.push(ListSong {
            video_id: s.video_id,
            track_no: Some(s.track_no),
            plays: String::new(),
            title: s.title,
            explicit: Some(s.explicit),
            download_status: DownloadStatus::None,
            id: ListSongID(0),
            duration_string: s.duration,
            actual_duration: None,
            start_offset: None,
            year: year.map(Rc::new),
            album_art: AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            artists: list_artists,
            thumbnails: MaybeRc::Owned(s.thumbnails),
            album: list_album,
            like_status: s.like_status,
            is_album_upload: false,
            release_mbid: None,
        });
    }
    list_songs
}

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for LoadPlaylistEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            LoadPlaylistEffect::TracksFetched(songs) => {
                let count = songs.len();
                let list_songs = convert_playlist_songs(songs);
                // Replace playlist
                target.list.clear();
                target.list.next_id = ListSongID(0);
                let first_id = target.list.push_song_list(list_songs);
                target.cur_selected = 0;
                info!("Loaded {} songs from YouTube Music playlist", count);
                // Validate first song for album splitting
                let mut effect = AsyncTask::new_no_op();
                if let Some(song) = target.get_song_from_id(first_id) {
                    let first_id = song.id;
                    let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                    if !artist.is_empty() {
                        let clean_title = Playlist::clean_title_for_metadata(&artist, &song.title);
                        let album = song.album.as_ref().map(|a| a.name.clone());
                        let validation_task = AsyncTask::new_future_try(
                            ValidateMetadata(artist, clean_title, first_id,
                                target.scrobbling_config.api_key.clone(),
                                Some(target.scrobbling_config.discogs_token.clone()).filter(|s| !s.is_empty()),
                                album,
                            ),
                            HandleMetadataValidated(first_id),
                            HandleMetadataValidationError,
                            None,
                        );
                        effect = effect.push(validation_task);
                    }
                }
                return effect;
            }
            LoadPlaylistEffect::TracksAppended(songs) => {
                let count = songs.len();
                let list_songs = convert_playlist_songs(songs);
                // Append to existing queue
                let first_id = target.list.push_song_list(list_songs);
                info!("Appended {} songs to queue from YouTube Music playlist", count);
                // Validate first appended song for album splitting
                let mut effect = AsyncTask::new_no_op();
                if let Some(song) = target.get_song_from_id(first_id) {
                    let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                    if !artist.is_empty() {
                        let clean_title = Playlist::clean_title_for_metadata(&artist, &song.title);
                        let album = song.album.as_ref().map(|a| a.name.clone());
                        let validation_task = AsyncTask::new_future_try(
                            ValidateMetadata(artist, clean_title, first_id,
                                target.scrobbling_config.api_key.clone(),
                                Some(target.scrobbling_config.discogs_token.clone()).filter(|s| !s.is_empty()),
                                album,
                            ),
                            HandleMetadataValidated(first_id),
                            HandleMetadataValidationError,
                            None,
                        );
                        effect = effect.push(validation_task);
                    }
                }
                return effect;
            }
            LoadPlaylistEffect::FetchError => {
                error!("Failed to load playlist tracks from YouTube Music");
                target.last_error = Some("Failed to load YouTube Music playlist - single video added if available".to_string());
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleGetPlaylistTracksOk,
    EnrichedPlaylistTracks,
    Playlist,
    |_, tracks: EnrichedPlaylistTracks| {
        LoadPlaylistEffect::TracksFetched(tracks.songs)
    }
);

impl_youtui_task_handler!(
    HandleGetPlaylistTracksErr,
    anyhow::Error,
    Playlist,
    |_, error: anyhow::Error| {
        error!("GetPlaylistTracks failed: {:?}", error);
        LoadPlaylistEffect::FetchError
    }
);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleGetPlaylistTracksAppendOk;

impl_youtui_task_handler!(
    HandleGetPlaylistTracksAppendOk,
    EnrichedPlaylistTracks,
    Playlist,
    |_, tracks: EnrichedPlaylistTracks| {
        LoadPlaylistEffect::TracksAppended(tracks.songs)
    }
);

// GetRelatedTracks handler - converts WatchPlaylistTrack → ListSong and inserts next
#[derive(Debug, PartialEq)]
pub struct HandleGetRelatedTracksOk;
#[derive(Debug, PartialEq)]
pub struct HandleGetRelatedTracksErr;

impl_youtui_task_handler!(
    HandleGetRelatedTracksOk,
    Vec<WatchPlaylistTrack>,
    Playlist,
    |_, tracks: Vec<WatchPlaylistTrack>| {
        move |this: &mut Playlist| {
            let count = tracks.len();

            // Pre-collect enrichment data before consuming tracks
            let enrich_data: Vec<(String, String, String)> = tracks.iter().map(|t| {
                (t.video_id.get_raw().to_string(), t.author.clone(), t.title.clone())
            }).collect();

            let insert_pos = this.get_cur_playing_index().map(|i| i + 1).unwrap_or(0);

            let songs: Vec<crate::app::structures::ListSong> = tracks.into_iter().map(|t| {
                crate::app::structures::ListSong {
                    video_id: t.video_id,
                    track_no: None,
                    plays: String::new(),
                    title: t.title,
                    explicit: None,
                    download_status: DownloadStatus::None,
                    id: crate::app::structures::ListSongID(0),
                    duration_string: t.duration,
                    actual_duration: None,
                    start_offset: None,
                    year: None,
                    album_art: AlbumArtState::None,
                    genres: Vec::new(),
                    styles: Vec::new(),
                    artists: MaybeRc::Owned(vec![ListSongArtist {
                        name: t.author,
                        id: None,
                    }]),
                    thumbnails: MaybeRc::Owned(t.thumbnails),
                    album: None,
                    like_status: ytmapi_rs::common::LikeStatus::Indifferent,
                    is_album_upload: false,
                    release_mbid: None,
                }
            }).collect();
            let _task = this.insert_next_song_list(songs);
            info!("Inserted {} related tracks", count);

            // Build enrichment data with positions and spawn yt-dlp metadata fetch
            let enrich_with_pos: Vec<(usize, String, String, String)> = enrich_data.into_iter().enumerate().map(|(i, (vid, artist, title))| {
                (insert_pos + i, vid, artist, title)
            }).collect();

            if !enrich_with_pos.is_empty() {
                AsyncTask::new_future_try(
                    EnrichRelatedTracks(enrich_with_pos),
                    HandleEnrichRelatedTracksOk,
                    HandleEnrichRelatedTracksErr,
                    None,
                )
            } else {
                AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
            }
        }
    }
);

impl_youtui_task_handler!(
    HandleGetRelatedTracksErr,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("GetRelatedTracks failed: {}", msg);
            this.last_error = Some(format!("Related tracks failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

// EnrichRelatedTracks handlers - apply yt-dlp metadata (year, album) to related tracks
#[derive(Debug, PartialEq)]
pub struct HandleEnrichRelatedTracksOk;
#[derive(Debug, PartialEq)]
pub struct HandleEnrichRelatedTracksErr;

impl_youtui_task_handler!(
    HandleEnrichRelatedTracksOk,
    Vec<(usize, Option<String>, Option<String>)>,
    Playlist,
    |_, results: Vec<(usize, Option<String>, Option<String>)>| {
        move |this: &mut Playlist| {
            let count = results.len();
            use std::collections::HashMap;
            let result_map: HashMap<usize, (Option<String>, Option<String>)> = results.into_iter().map(|(idx, y, a)| (idx, (y, a))).collect();
            for (i, song) in this.list.get_list_iter_mut().enumerate() {
                if let Some((year, album_name)) = result_map.get(&i) {
                    if year.is_some() || album_name.is_some() {
                        song.year = year.clone().map(Rc::new);
                        if let Some(name) = album_name.clone() {
                            let vid = song.video_id.get_raw().to_string();
                            song.album = Some(MaybeRc::Owned(ListSongAlbum {
                                name,
                                id: AlbumOrUploadAlbumID::Album(AlbumID::from_raw(vid)),
                            }));
                        }
                    }
                }
            }
            info!("Enriched {} related tracks with yt-dlp metadata", count);
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleEnrichRelatedTracksErr,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            warn!("Related tracks enrichment failed: {}", msg);
            this.last_error = Some(format!("Related tracks enrichment failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

#[derive(Debug, PartialEq)]
pub struct HandleSubscribeToArtistOk;
#[derive(Debug, PartialEq)]
pub struct HandleSubscribeToArtistError;

impl_youtui_task_handler!(HandleSubscribeToArtistOk, (), Playlist, |_, _: ()| {
    |_this: &mut Playlist| {
        info!("Subscribed to artist");
        AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
    }
});

playlist_err_handler!(HandleSubscribeToArtistError, "subscribe to artist", "Subscribe failed");

#[derive(Debug, PartialEq)]
pub struct HandleUnsubscribeFromArtistsOk;
#[derive(Debug, PartialEq)]
pub struct HandleUnsubscribeFromArtistsError;

impl_youtui_task_handler!(HandleUnsubscribeFromArtistsOk, (), Playlist, |_, _: ()| {
    |_this: &mut Playlist| {
        info!("Unsubscribed from artist");
        AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
    }
});

playlist_err_handler!(HandleUnsubscribeFromArtistsError, "unsubscribe from artist", "Unsubscribe failed");

#[derive(Debug, PartialEq)]
pub struct HandleAddPlaylistToPlaylistOk;
#[derive(Debug, PartialEq)]
pub struct HandleAddPlaylistToPlaylistError;

playlist_ok_handler!(HandleAddPlaylistToPlaylistOk, "Playlist merged successfully");
playlist_err_handler!(HandleAddPlaylistToPlaylistError, "merge playlist", "Merge failed");

