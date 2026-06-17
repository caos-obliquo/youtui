use crate::app::component::actionhandler::ComponentEffect;
use crate::app::server::ValidatedMetadata;
use crate::app::server::{ArcServer, TaskMetadata, AlbumTrack};
use crate::app::structures::{AlbumOrUploadAlbumID, ListSongID};
use crate::app::ui::playlist::Playlist;
use crate::app::ui::playlist::lyrics_popup::LyricsPopup;
use crate::app::ui::playlist::playlist_update_popup::{PlaylistUpdatePopup, PlaylistUpdatePopupState};
use async_callback_manager::{AsyncTask, FrontendEffect};
use std::rc::Rc;
use tracing::{error, info};
use ytmapi_rs::common::{PlaylistID, YoutubeID};
use ytmapi_rs::parse::LibraryPlaylist;

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

impl_youtui_task_handler!(
    HandleAddSongsError,
    anyhow::Error,
    Playlist,
    |_, error: anyhow::Error| {
        error!("Error adding songs to playlist: {}", error);
        PlaylistEffect::AddSongsError
    }
);

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for PlaylistEffect {
    fn apply(self, _target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            PlaylistEffect::CreatePlaylistSuccess(playlist_id) => {
                info!("Playlist created: {:?}", playlist_id);
            }
            PlaylistEffect::CreatePlaylistError => {
                error!("Failed to create playlist");
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

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for MetadataEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            MetadataEffect::Validated(data, song_id) => {
                if let Some(idx) = target.get_index_from_id(song_id) {
                    if let Some(song) = target.list.get_list_iter_mut().nth(idx) {
                        if let Some(ref album) = data.album {
                            song.album = Some(crate::app::structures::MaybeRc::Owned(
                                crate::app::structures::ListSongAlbum {
                                    name: album.clone(),
                                    id: AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
                                },
                            ));
                        }
                        if let Some(ref year) = data.year {
                            song.year = Some(Rc::new(year.clone()));
                        }
                        if let Some(ref artist) = data.artist {
                            song.artists = crate::app::structures::MaybeRc::Owned(vec![
                                crate::app::structures::ListSongArtist {
                                    name: artist.clone(),
                                    id: None,
                                },
                            ]);
                        }
                        if let Some(tn) = data.track_no {
                            song.track_no = Some(tn);
                        }
                        info!("Metadata validated for song {:?} (artist={:?}, album={:?}, year={:?}, track={:?})",
                            song_id, data.artist, data.album, data.year, data.track_no);
                        if !data.album_tracks.is_empty() {
                            target.album_tracks = Some(data.album_tracks.clone());
                            target.album_current_track = 0;
                            info!("Album mode: {} tracks loaded for song {:?}",
                                target.album_tracks.as_ref().map_or(0, |t| t.len()), song_id);
                        }
                    }
                }
            }
            MetadataEffect::ValidationError => {
                info!("Metadata validation failed (non-critical)");
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

// Album tracks (full album video splitting) effect handlers

#[derive(Debug, PartialEq)]
pub struct HandleAlbumTracksOk(pub ListSongID);
#[derive(Debug, PartialEq)]
pub struct HandleAlbumTracksError;

#[derive(Debug, PartialEq)]
pub enum AlbumTracksEffect {
    TracksFetched(Vec<AlbumTrack>, ListSongID),
    TrackFetchError,
}

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for AlbumTracksEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            AlbumTracksEffect::TracksFetched(tracks, song_id) => {
                if tracks.len() >= 2 {
                    info!("AlbumTracksEffect: {} tracks for song {:?}", tracks.len(), song_id);
                    target.album_tracks = Some(tracks);
                    target.album_current_track = 0;
                }
            }
            AlbumTracksEffect::TrackFetchError => {
                info!("AlbumTracksEffect: failed to fetch tracks");
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleAlbumTracksOk,
    Vec<AlbumTrack>,
    Playlist,
    |this: HandleAlbumTracksOk, tracks: Vec<AlbumTrack>| {
        AlbumTracksEffect::TracksFetched(tracks, this.0)
    }
);

impl_youtui_task_handler!(
    HandleAlbumTracksError,
    anyhow::Error,
    Playlist,
    |_, _error: anyhow::Error| {
        AlbumTracksEffect::TrackFetchError
    }
);
