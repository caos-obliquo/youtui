use crate::app::component::actionhandler::ComponentEffect;
use crate::app::server::{ArcServer, TaskMetadata};
use crate::app::ui::playlist::{Playlist, PlaylistUpdatePopup};
use async_callback_manager::{AsyncTask, FrontendEffect};
use tracing::{error, info};
use ytmapi_rs::common::PlaylistID;
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
pub struct HandleSaveQueueOk;
#[derive(Debug, PartialEq)]
pub struct HandleSaveQueueError;
#[derive(Debug, PartialEq)]
pub struct HandleGetAllLibraryPlaylistsOk;
#[derive(Debug, PartialEq)]
pub struct HandleGetAllLibraryPlaylistsError;

// Effect enum for Playlist operations
#[derive(Debug, PartialEq)]
pub enum PlaylistEffect {
    CreatePlaylistSuccess(PlaylistID<'static>),
    CreatePlaylistError,
    AddSongsSuccess,
    AddSongsError,
    SaveQueueSuccess(PlaylistID<'static>),
    SaveQueueError,
}

// Effect enum for PlaylistUpdatePopup
#[derive(Debug, PartialEq)]
pub enum PlaylistUpdateEffect {
    FetchPlaylistsSuccess(Vec<LibraryPlaylist>),
    FetchPlaylistsError(String),
}

// Playlist effect handlers
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

impl_youtui_task_handler!(
    HandleSaveQueueOk,
    PlaylistID<'static>,
    Playlist,
    |_, playlist_id: PlaylistID<'static>| {
        info!("Queue saved to playlist: {:?}", playlist_id);
        PlaylistEffect::SaveQueueSuccess(playlist_id)
    }
);

impl_youtui_task_handler!(
    HandleSaveQueueError,
    anyhow::Error,
    Playlist,
    |_, error: anyhow::Error| {
        error!("Error saving queue to playlist: {}", error);
        PlaylistEffect::SaveQueueError
    }
);

// PlaylistUpdatePopup effect handlers
impl_youtui_task_handler!(
    HandleGetAllLibraryPlaylistsOk,
    Vec<LibraryPlaylist>,
    PlaylistUpdatePopup,
    |_, playlists: Vec<LibraryPlaylist>| {
        info!("Successfully fetched {} library playlists", playlists.len());
        PlaylistUpdateEffect::FetchPlaylistsSuccess(playlists)
    }
);

impl_youtui_task_handler!(
    HandleGetAllLibraryPlaylistsError,
    anyhow::Error,
    PlaylistUpdatePopup,
    |_, error: anyhow::Error| {
        error!("Failed to fetch library playlists: {}", error);
        PlaylistUpdateEffect::FetchPlaylistsError(error.to_string())
    }
);

// FrontendEffect implementations
impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for PlaylistEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
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
            PlaylistEffect::SaveQueueSuccess(playlist_id) => {
                info!("Queue saved to playlist: {:?}", playlist_id);
            }
            PlaylistEffect::SaveQueueError => {
                error!("Error saving queue to playlist");
            }
        }
        AsyncTask::new_no_op()
    }
}

impl FrontendEffect<PlaylistUpdatePopup, ArcServer, TaskMetadata> for PlaylistUpdateEffect {
    fn apply(self, target: &mut PlaylistUpdatePopup) -> impl Into<ComponentEffect<PlaylistUpdatePopup>> {
        match self {
            PlaylistUpdateEffect::FetchPlaylistsSuccess(playlists) => {
                target.set_playlists(playlists);
            }
            PlaylistUpdateEffect::FetchPlaylistsError(error) => {
                target.set_error(error);
            }
        }
        AsyncTask::new_no_op()
    }
}
