use super::*;
use crate::app::server::{CreatePlaylist, AddSongsToPlaylist};
use ytmapi_rs::common::{PlaylistID, VideoID};

pub struct HandleCreatePlaylistOk;
pub struct HandleCreatePlaylistError;
pub struct HandleAddSongsOk;
pub struct HandleAddSongsError;

impl EffectHandler<Playlist> for HandleCreatePlaylistOk {
    fn handle_effect(
        component: &mut Playlist,
        message: Result<PlaylistID<'static>>,
    ) -> ComponentEffect<Playlist> {
        match message {
            Ok(playlist_id) => {
                info!("Playlist created: {:?}", playlist_id);
                // Now add all songs from queue
                let video_ids: Vec<VideoID> = component
                    .list
                    .get_list_iter()
                    .map(|song| song.video_id.clone())
                    .collect();
                
                if video_ids.is_empty() {
                    return AsyncTask::new_no_op();
                }

                AsyncTask::new_future_try(
                    AddSongsToPlaylist {
                        playlist_id,
                        video_ids,
                    },
                    HandleAddSongsOk,
                    HandleAddSongsError,
                    None,
                )
            }
            Err(e) => {
                error!("Failed to create playlist: {}", e);
                AsyncTask::new_no_op()
            }
        }
    }
}

impl EffectHandler<Playlist> for HandleCreatePlaylistError {
    fn handle_effect(
        _component: &mut Playlist,
        error: anyhow::Error,
    ) -> ComponentEffect<Playlist> {
        error!("Error creating playlist: {}", error);
        AsyncTask::new_no_op()
    }
}

impl EffectHandler<Playlist> for HandleAddSongsOk {
    fn handle_effect(
        _component: &mut Playlist,
        message: Result<()>,
    ) -> ComponentEffect<Playlist> {
        match message {
            Ok(_) => {
                info!("Successfully added songs to playlist!");
                AsyncTask::new_no_op()
            }
            Err(e) => {
                error!("Failed to add songs: {}", e);
                AsyncTask::new_no_op()
            }
        }
    }
}

impl EffectHandler<Playlist> for HandleAddSongsError {
    fn handle_effect(
        _component: &mut Playlist,
        error: anyhow::Error,
    ) -> ComponentEffect<Playlist> {
        error!("Error adding songs to playlist: {}", error);
        AsyncTask::new_no_op()
    }
}

impl EffectHandler<Playlist> for HandleSaveQueueOk {
    fn handle_effect(
        _component: &mut Playlist,
        result: Result<PlaylistID<'static>>,
    ) -> ComponentEffect<Playlist> {
        match result {
            Ok(playlist_id) => {
                info!("Queue saved to playlist: {:?}", playlist_id);
            }
            Err(e) => {
                error!("Failed to save queue: {}", e);
            }
        }
        AsyncTask::new_no_op()
    }
}

impl EffectHandler<Playlist> for HandleSaveQueueError {
    fn handle_effect(
        _component: &mut Playlist,
        error: Error,
    ) -> ComponentEffect<Playlist> {
        error!("Error saving queue to playlist: {}", error);
        AsyncTask::new_no_op()
    }
}
