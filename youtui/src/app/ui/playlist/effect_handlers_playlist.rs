use super::playlist_main::Playlist;
use crate::app::component::actionhandler::ComponentEffect;
use crate::app::server::{AddSongsToPlaylist, ArcServer, TaskMetadata};
use async_callback_manager::{AsyncTask, FrontendEffect};
use tracing::{error, info};
use ytmapi_rs::common::{PlaylistID, VideoID};
#[allow(unused_imports)]
use ytmapi_rs::error::Error;

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
enum PlaylistSaveEffect {
    CreatePlaylistSuccess(PlaylistID<'static>),
    CreatePlaylistError,
    AddSongsSuccess,
    AddSongsError,
    SaveQueueSuccess(PlaylistID<'static>),
    SaveQueueError,
}

// Handler for successful playlist creation - needs to add songs next
impl_youtui_task_handler!(
    HandleCreatePlaylistOk,
    PlaylistID<'static>,
    Playlist,
    |_, playlist_id| {
        info!("Playlist created: {:?}", playlist_id);
        PlaylistSaveEffect::CreatePlaylistSuccess(playlist_id)
    }
);

// Handler for playlist creation error
impl_youtui_task_handler!(
    HandleCreatePlaylistError,
    anyhow::Error,
    Playlist,
    |_, error| {
        error!("Failed to create playlist: {}", error);
        PlaylistSaveEffect::CreatePlaylistError
    }
);

// Handler for successful song addition
impl_youtui_task_handler!(
    HandleAddSongsOk,
    (),
    Playlist,
    |_, _| {
        info!("Successfully added songs to playlist!");
        PlaylistSaveEffect::AddSongsSuccess
    }
);

// Handler for song addition error
impl_youtui_task_handler!(
    HandleAddSongsError,
    anyhow::Error,
    Playlist,
    |_, error| {
        error!("Error adding songs to playlist: {}", error);
        PlaylistSaveEffect::AddSongsError
    }
);

// Handler for save queue success
impl_youtui_task_handler!(
    HandleSaveQueueOk,
    PlaylistID<'static>,
    Playlist,
    |_, playlist_id| {
        info!("Queue saved to playlist: {:?}", playlist_id);
        PlaylistSaveEffect::SaveQueueSuccess(playlist_id)
    }
);

// Handler for save queue error
impl_youtui_task_handler!(
    HandleSaveQueueError,
    anyhow::Error,
    Playlist,
    |_, error| {
        error!("Error saving queue to playlist: {}", error);
        PlaylistSaveEffect::SaveQueueError
    }
);

impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for PlaylistSaveEffect {
    fn apply(self, target: &mut Playlist) -> impl Into<ComponentEffect<Playlist>> {
        match self {
            PlaylistSaveEffect::CreatePlaylistSuccess(playlist_id) => {
                // Playlist already created with videos - no need to add them again
                info!("Playlist created successfully: {:?}", playlist_id);
            }
            PlaylistSaveEffect::CreatePlaylistError => {},
            PlaylistSaveEffect::AddSongsSuccess => {},
            PlaylistSaveEffect::AddSongsError => {},
            PlaylistSaveEffect::SaveQueueSuccess(_) => {},
            PlaylistSaveEffect::SaveQueueError => {},
        }
        AsyncTask::new_no_op()
    }
}
