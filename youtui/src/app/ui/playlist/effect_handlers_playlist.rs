use crate::app::component::actionhandler::ComponentEffect;
use crate::app::server::ValidatedMetadata;

use crate::app::server::{ArcServer, TaskMetadata};
use crate::app::structures::{AlbumOrUploadAlbumID, ListSongID, ListSongArtist, MaybeRc, ListSongAlbum};
use crate::app::structures::{AlbumArtState, DownloadStatus};
use crate::app::ui::playlist::Playlist;
use crate::app::ui::playlist::lyrics_popup::LyricsPopup;
use crate::app::ui::playlist::playlist_update_popup::{PlaylistUpdatePopup, PlaylistUpdatePopupState};
use crate::app::ui::playlist::playlist_details_popup::PlaylistDetailsPopup;
use async_callback_manager::{AsyncTask, FrontendEffect};
use std::rc::Rc;
use tracing::{error, info};
use ytmapi_rs::common::{PlaylistID, YoutubeID};
use ytmapi_rs::parse::LibraryPlaylist;
use ytmapi_rs::parse::PlaylistSong;
use ytmapi_rs::parse::WatchPlaylistTrack;

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
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub struct HandleReorderPlaylistItemOk;
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub struct HandleReorderPlaylistItemError;
#[derive(Debug, PartialEq)]
pub struct HandleRenamePlaylistOk;
#[derive(Debug, PartialEq)]
pub struct HandleRenamePlaylistError;
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub struct HandleRemovePlaylistItemsOk;
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub struct HandleRemovePlaylistItemsError;

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

impl_youtui_task_handler!(
    HandleRateSongOk,
    (),
    Playlist,
    |_, _: ()| {
        |_this: &mut Playlist| {
            info!("Song rated successfully");
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleRateSongErr,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |_this: &mut Playlist| {
            error!("Failed to rate song: {}", msg);
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

impl_youtui_task_handler!(
    HandleDeletePlaylistOk,
    (),
    Playlist,
    |_, _: ()| {
        |this: &mut Playlist| {
            info!("Playlist deleted successfully");
            this.library_playlist_mutated = true;
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleDeletePlaylistError,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("Failed to delete playlist: {}", msg);
            this.last_error = Some(format!("Delete failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleEditPlaylistDetailsOk,
    (),
    Playlist,
    |_, _: ()| {
        |this: &mut Playlist| {
            info!("Playlist details updated");
            this.library_playlist_mutated = true;
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleEditPlaylistDetailsError,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("Failed to update playlist details: {}", msg);
            this.last_error = Some(format!("Edit failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleRatePlaylistOk,
    (),
    Playlist,
    |_, _: ()| {
        |this: &mut Playlist| {
            info!("Playlist rated successfully");
            this.library_playlist_mutated = true;
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleRatePlaylistError,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("Failed to rate playlist: {}", msg);
            this.last_error = Some(format!("Rate failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);


impl_youtui_task_handler!(
    HandleReorderPlaylistItemOk,
    (),
    Playlist,
    |_, _: ()| {
        |_this: &mut Playlist| {
            info!("Playlist item reordered");
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleReorderPlaylistItemError,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("Failed to reorder playlist item: {}", msg);
            this.last_error = Some(format!("Reorder failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleRenamePlaylistOk,
    (),
    Playlist,
    |_, _: ()| {
        |this: &mut Playlist| {
            info!("Playlist renamed successfully");
            this.library_playlist_mutated = true;
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleRenamePlaylistError,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("Failed to rename playlist: {}", msg);
            this.last_error = Some(format!("Rename failed: {}", msg));
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleRemovePlaylistItemsOk,
    (),
    Playlist,
    |_, _: ()| {
        |_this: &mut Playlist| {
            info!("Playlist items removed successfully");
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
        }
    }
);

impl_youtui_task_handler!(
    HandleRemovePlaylistItemsError,
    anyhow::Error,
    Playlist,
    |_, err: anyhow::Error| {
        let msg = err.to_string();
        move |this: &mut Playlist| {
            error!("Failed to remove playlist items: {}", msg);
            this.last_error = Some(format!("Remove failed: {}", msg));
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

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for MetadataEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            // Album metadata found: update song fields, split into tracks, spawn per-track validation + album art
            MetadataEffect::Validated(data, song_id) => {
                if let Some(idx) = target.get_index_from_id(song_id) {
                    if let Some(song) = target.list.get_list_iter_mut().nth(idx) {
                        // Save original album before metadata overwrites it
                        let original_album = song.album.as_ref().map(|a| a.as_ref().name.clone());
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
                            song.artists = crate::app::structures::MaybeRc::Owned(vec![
                                crate::app::structures::ListSongArtist {
                                    name: normalized,
                                    id: None,
                                },
                            ]);
                        }
                        if let Some(tn) = data.track_no {
                            song.track_no = Some(tn);
                        }
                        info!("Metadata validated for song {:?} (artist={:?}, album={:?}, year={:?}, track={:?})",
                            song_id, data.artist, data.album, data.year, data.track_no);
                        if !data.album_tracks.is_empty() && target.album_tracks.is_none() {
                            // Quality guard: verify tracklist before splitting
                            let valid_tracks = data.album_tracks.iter().all(|t| t.duration_secs > 0.0);
                            let total_dur: f64 = data.album_tracks.iter().map(|t| t.duration_secs).sum();
                            let video_dur = song.actual_duration.map(|d| d.as_secs_f64()).unwrap_or(0.0);
                            let duration_ok = video_dur == 0.0 || total_dur >= video_dur * 0.5;
                            if !valid_tracks || !duration_ok {
                                target.last_error = Some(format!(
                                    "Album tracklist rejected: {} tracks, {:.0}s total vs video {:.0}s {}",
                                    data.album_tracks.len(), total_dur, video_dur,
                                    if !valid_tracks { "(zero-duration tracks)" } else { "" }
                                ));
                                return AsyncTask::new_no_op();
                            }
                            target.album_tracks = Some(data.album_tracks.clone());
                            target.album_current_track = 0;
                            let play_effect = target.insert_album_tracks(song_id, &data.album_tracks, &data.artist, &data.album, &data.year, &original_album);
                            info!("Album mode: {} tracks loaded for song {:?}",
                                target.album_tracks.as_ref().map_or(0, |t| t.len()), song_id);

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
                                        use crate::app::server::FetchAlbumArt;
                                        let art_task = AsyncTask::new_future_try(
                                            FetchAlbumArt(aa.clone(), ab.clone(), api_key.clone()),
                                            HandleFetchAlbumArtOk,
                                            HandleFetchAlbumArtErr,
                                            None,
                                        );
                                        effect = effect.push(art_task);
                                    }
                                }
                            }

                            if let Some(e) = play_effect {
                                effect = effect.push(e);
                            }
                            return effect;
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

// Album art from Last.fm effect handlers

use crate::app::server::song_thumbnail_downloader::SongThumbnail;
#[derive(Debug, PartialEq)]
pub struct HandleFetchAlbumArtOk;
#[derive(Debug, PartialEq)]
pub struct HandleFetchAlbumArtErr;

#[derive(Debug, PartialEq)]
pub enum FetchAlbumArtEffect {
    Fetched(SongThumbnail),
    FetchError,
}

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for FetchAlbumArtEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
    match self {
            FetchAlbumArtEffect::Fetched(thumbnail) => {
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
                target.album_art_fetching = false;
                info!("FetchAlbumArtEffect: applied art");
            }
            FetchAlbumArtEffect::FetchError => {
                target.album_art_fetching_name.take();
                target.album_art_fetching = false;
                error!("FetchAlbumArtEffect: failed to fetch album art");
            }
        }
        AsyncTask::new_no_op()
    }
}

impl_youtui_task_handler!(
    HandleFetchAlbumArtOk,
    SongThumbnail,
    Playlist,
    |_, thumbnail: SongThumbnail| {
        FetchAlbumArtEffect::Fetched(thumbnail)
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

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for LoadPlaylistEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            LoadPlaylistEffect::TracksFetched(songs) => {
                let count = songs.len();
                let mut list_songs: Vec<crate::app::structures::ListSong> = Vec::new();
                for s in songs {
                    use ytmapi_rs::common::YoutubeID;
                    let list_artists = MaybeRc::Owned(s.artists.into_iter().map(|a| ListSongArtist {
                        name: a.name,
                        id: None,
                    }).collect());
                    let list_album = Some(MaybeRc::Owned(ListSongAlbum {
                        name: s.album.name.clone(),
                        id: AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
                    }));
                    // Extract year from album name as fallback
                    let year = s.year.clone().or_else(|| {
                        let name = &s.album.name;
                        name.split('(').last().and_then(|s| s.get(..4))
                            .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
                            .map(|y| y.to_string())
                    });
                    list_songs.push(crate::app::structures::ListSong {
                        video_id: s.video_id,
                        track_no: None,
                        plays: String::new(),
                        title: s.title,
                        explicit: Some(s.explicit),
                        download_status: DownloadStatus::None,
                        id: crate::app::structures::ListSongID(0),
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
                    });
                }
                // Replace playlist
                target.list.clear();
                target.list.next_id = ListSongID(0);
                target.list.push_song_list(list_songs);
                target.cur_selected = 0;
                info!("Loaded {} songs from YouTube Music playlist", count);
            }
            LoadPlaylistEffect::TracksAppended(songs) => {
                let count = songs.len();
                let mut list_songs: Vec<crate::app::structures::ListSong> = Vec::new();
                for s in songs {
                    use ytmapi_rs::common::YoutubeID;
                    let list_artists = MaybeRc::Owned(s.artists.into_iter().map(|a| ListSongArtist {
                        name: a.name,
                        id: None,
                    }).collect());
                    let album_name = s.album.name.clone();
                    let list_album = Some(MaybeRc::Owned(ListSongAlbum {
                        name: album_name.clone(),
                        id: AlbumOrUploadAlbumID::Album(ytmapi_rs::common::AlbumID::from_raw("")),
                    }));
                    // Extract year from album name as fallback
                    let year = s.year.clone().or_else(|| {
                        let name = &album_name;
                        name.split('(').last().and_then(|s| s.get(..4))
                            .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
                            .map(|y| y.to_string())
                    });
                    list_songs.push(crate::app::structures::ListSong {
                        video_id: s.video_id,
                        track_no: None,
                        plays: String::new(),
                        title: s.title,
                        explicit: Some(s.explicit),
                        download_status: DownloadStatus::None,
                        id: crate::app::structures::ListSongID(0),
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
                    });
                }
                // Append to existing queue
                target.list.push_song_list(list_songs);
                info!("Appended {} songs to queue from YouTube Music playlist", count);
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
    Vec<PlaylistSong>,
    Playlist,
    |_, songs: Vec<PlaylistSong>| {
        LoadPlaylistEffect::TracksFetched(songs)
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
    Vec<PlaylistSong>,
    Playlist,
    |_, songs: Vec<PlaylistSong>| {
        LoadPlaylistEffect::TracksAppended(songs)
    }
);

// GetRelatedTracks handler — converts WatchPlaylistTrack → ListSong and inserts next
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
                }
            }).collect();
            let _task = this.insert_next_song_list(songs);
            info!("Inserted {} related tracks", count);
            AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
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

impl_youtui_task_handler!(HandleSubscribeToArtistError, anyhow::Error, Playlist, |_, err: anyhow::Error| {
    let msg = err.to_string();
    move |this: &mut Playlist| {
        error!("Failed to subscribe to artist: {}", msg);
        this.last_error = Some(format!("Subscribe failed: {}", msg));
        AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
    }
});

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

impl_youtui_task_handler!(HandleUnsubscribeFromArtistsError, anyhow::Error, Playlist, |_, err: anyhow::Error| {
    let msg = err.to_string();
    move |this: &mut Playlist| {
        error!("Failed to unsubscribe from artist: {}", msg);
        this.last_error = Some(format!("Unsubscribe failed: {}", msg));
        AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
    }
});



#[derive(Debug, PartialEq)]
pub struct HandleAddPlaylistToPlaylistOk;
#[derive(Debug, PartialEq)]
pub struct HandleAddPlaylistToPlaylistError;

impl_youtui_task_handler!(HandleAddPlaylistToPlaylistOk, (), Playlist, |_, _: ()| {
    |this: &mut Playlist| {
        info!("Playlist merged successfully");
        this.library_playlist_mutated = true;
        AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
    }
});

impl_youtui_task_handler!(HandleAddPlaylistToPlaylistError, anyhow::Error, Playlist, |_, err: anyhow::Error| {
    let msg = err.to_string();
    move |this: &mut Playlist| {
        error!("Failed to merge playlist: {}", msg);
        this.last_error = Some(format!("Merge failed: {}", msg));
        AsyncTask::<Playlist, ArcServer, TaskMetadata>::new_no_op()
    }
});
