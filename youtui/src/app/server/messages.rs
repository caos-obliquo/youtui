use super::ArcServer;
use ytmapi_rs::parse::SearchResultAlbum;
use ytmapi_rs::parse::{TableListSong, LibraryArtist, LibraryPlaylist};
use super::api::GetArtistSongsProgressUpdate;
use super::player::{DecodedInMemSong, Player};
use super::song_downloader::{DownloadProgressUpdate, InMemSong};
use super::song_thumbnail_downloader::SongThumbnail;
use crate::app::server::api::GetPlaylistSongsProgressUpdate;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use crate::app::AudioQuality;
use crate::app::structures::ListSongID;
use crate::async_rodio_sink::rodio::decoder::DecoderError;
use crate::async_rodio_sink::{
    AllStopped, AutoplayUpdate, PausePlayResponse, Paused, PlayUpdate, ProgressUpdate, QueueUpdate,
    Resumed, SeekDirection, Stopped, VolumeUpdate,
};
use anyhow::{Error, Result};
use async_callback_manager::{BackendStreamingTask, BackendTask, MapFn};
use futures::{Future, Stream};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, SearchSuggestion, VideoID, SetVideoID, YoutubeID, LikeStatus};
use ytmapi_rs::query::playlist::PrivacyStatus;
// NOTE: Fallback lyrics providers disabled in favor of Genius-only.
// Uncomment these if you want to re-enable Musixmatch/bandcamp/lyr CLI:
// use musixmatch_inofficial::Musixmatch;
use ytmapi_rs::parse::{SearchResultArtist, SearchResultPlaylist, SearchResultSong, GetPlaylistDetails};

#[derive(PartialEq, Debug)]
pub enum TaskMetadata {
    PlayingSong,
    PlayPause,
}

#[derive(Debug)]
pub struct HandleApiError {
    pub error: Error,
    pub message: String,
}

#[derive(Debug, PartialEq)]
pub struct GetLyrics(pub String, pub String, pub String);
#[derive(Debug, PartialEq)]
pub struct GetAnnotations(pub String, pub String, pub String);
#[derive(Debug, PartialEq)]
pub struct ValidateMetadata(pub String, pub String, pub crate::app::structures::ListSongID, pub String, pub Option<String>);

#[derive(Debug, PartialEq)]
pub struct GetSearchSuggestions(pub String);
#[derive(Debug, PartialEq)]
pub struct SearchArtists(pub String);
#[derive(Debug, PartialEq)]
pub struct SearchSongs(pub String);
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub struct SearchPlaylists(pub String);
#[derive(Debug, PartialEq)]
pub struct SearchAlbums(pub String);
#[derive(Debug, PartialEq)]
pub struct GetArtistSongs(pub ArtistChannelID<'static>);
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub struct GetPlaylistSongs {
    pub playlist_id: PlaylistID<'static>,
    pub max_songs: usize,
}

#[derive(Debug, PartialEq)]
pub struct CreatePlaylistWithVideos {
    pub title: String,
    pub description: Option<String>,
    pub video_ids: Vec<VideoID<'static>>,
    pub privacy: Option<PrivacyStatus>,
}

#[derive(Debug, PartialEq)]
pub struct AddSongsToPlaylist {
    pub playlist_id: PlaylistID<'static>,
    pub video_ids: Vec<VideoID<'static>>,
}

#[derive(Debug, PartialEq)]
pub struct RateSong(pub VideoID<'static>, pub LikeStatus);

impl BackendTask<ArcServer> for RateSong {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            backend.api.rate_song(self.0, self.1).await
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SubscribeToArtist(pub ArtistChannelID<'static>);

impl BackendTask<ArcServer> for SubscribeToArtist {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::SubscribeArtistQuery;
            let api_guard = backend.api.get_api().await?;
            let query = SubscribeArtistQuery::new(self.0);
            api_guard.read().await.query_browser_or_oauth::<_, ()>(query).await?;
            tracing::info!("Subscribed to artist");
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct UnsubscribeFromArtists(pub Vec<ArtistChannelID<'static>>);

impl BackendTask<ArcServer> for UnsubscribeFromArtists {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::UnsubscribeArtistsQuery;
            let api_guard = backend.api.get_api().await?;
            let query = UnsubscribeArtistsQuery::new(self.0.into_iter().map(|id| id));
            api_guard.read().await.query_browser_or_oauth::<_, ()>(query).await
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RenamePlaylist {
    pub playlist_id: PlaylistID<'static>,
    pub new_title: String,
}

#[derive(Debug, PartialEq)]
pub struct RemovePlaylistItems {
    pub playlist_id: PlaylistID<'static>,
    pub video_ids: Vec<VideoID<'static>>,
}

#[derive(Debug, PartialEq)]
pub struct DeletePlaylist(pub PlaylistID<'static>);

#[derive(Debug, PartialEq)]
pub struct EditPlaylistDetails {
    pub playlist_id: PlaylistID<'static>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub privacy: Option<PrivacyStatus>,
}

#[derive(Debug, PartialEq)]
pub struct RatePlaylistMessage(pub PlaylistID<'static>, pub LikeStatus);

#[derive(Debug, PartialEq)]
pub struct GetPlaylistDetailsMessage(pub PlaylistID<'static>);

#[derive(Debug, PartialEq)]
pub struct ReorderPlaylistItem {
    pub playlist_id: PlaylistID<'static>,
    pub video_id: VideoID<'static>,
    pub target_video_id: VideoID<'static>,
}

#[derive(Debug, PartialEq)]
pub struct GetAllLibraryPlaylists;

impl BackendTask<ArcServer> for GetAllLibraryPlaylists {
    type Output = Result<Vec<LibraryPlaylist>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::GetLibraryPlaylistsQuery;
            use crate::app::server::api::stream_api_with_retry_n;

            let api_guard = backend.api.get_api().await?;

            match stream_api_with_retry_n(&api_guard, &GetLibraryPlaylistsQuery, 10).await {
                Ok(pages) => {
                    let playlists: Vec<_> = pages.into_iter().flatten().collect();
                    tracing::info!(count = %playlists.len(), "GetAllLibraryPlaylists done");
                    Ok(playlists)
                }
                Err(e) => {
                    tracing::warn!("GetLibraryPlaylistsQuery failed: {}. Library playlists require browser auth (cookies) or OAuth.", e);
                    Err(anyhow::anyhow!(
                        "Library playlists unavailable. Configure cookies or OAuth in config. Error: {}", e
                    ))
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct GetAllLibrarySongs;

impl BackendTask<ArcServer> for GetAllLibrarySongs {
    type Output = Result<Vec<TableListSong>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::GetLibrarySongsQuery;
            use crate::app::server::api::stream_api_with_retry_n;

            let api_guard = backend.api.get_api().await?;

            match stream_api_with_retry_n(&api_guard, &GetLibrarySongsQuery::default(), 10).await {
                Ok(pages) => {
                    let songs: Vec<_> = pages.into_iter().flatten().collect();
                    tracing::info!(count = %songs.len(), "GetAllLibrarySongs done");
                    Ok(songs)
                }
                Err(e) => {
                    tracing::warn!("GetLibrarySongsQuery failed: {}. Library songs require browser auth (cookies) or OAuth.", e);
                    Err(anyhow::anyhow!(
                        "Library songs unavailable. Configure cookies or OAuth in config. Error: {}", e
                    ))
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct GetAllLibraryArtists;

impl BackendTask<ArcServer> for GetAllLibraryArtists {
    type Output = Result<Vec<LibraryArtist>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::GetLibraryArtistsQuery;
            use crate::app::server::api::stream_api_with_retry_n;

            let api_guard = backend.api.get_api().await?;

            match stream_api_with_retry_n(&api_guard, &GetLibraryArtistsQuery::default(), 10).await {
                Ok(pages) => {
                    let artists: Vec<_> = pages.into_iter().flatten().collect();
                    tracing::info!(count = %artists.len(), "GetAllLibraryArtists done");
                    Ok(artists)
                }
                Err(e) => {
                    tracing::warn!("GetLibraryArtistsQuery failed: {}. Library artists require browser auth (cookies) or OAuth.", e);
                    Err(anyhow::anyhow!(
                        "Library artists unavailable. Configure cookies or OAuth in config. Error: {}", e
                    ))
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct GetAllLibraryAlbums;

impl BackendTask<ArcServer> for GetAllLibraryAlbums {
    type Output = Result<Vec<SearchResultAlbum>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::GetLibraryAlbumsQuery;
            use crate::app::server::api::stream_api_with_retry_n;

            let api_guard = backend.api.get_api().await?;

            match stream_api_with_retry_n(&api_guard, &GetLibraryAlbumsQuery::default(), 10).await {
                Ok(pages) => {
                    let albums: Vec<_> = pages.into_iter().flatten().collect();
                    tracing::info!(count = %albums.len(), "GetAllLibraryAlbums done");
                    Ok(albums)
                }
                Err(e) => {
                    tracing::warn!("GetLibraryAlbumsQuery failed: {}. Library albums require browser auth (cookies) or OAuth.", e);
                    Err(anyhow::anyhow!(
                        "Library albums unavailable. Configure cookies or OAuth in config. Error: {}", e
                    ))
                }
            }
        }
    }
}

use ytmapi_rs::parse::PlaylistSong;

#[derive(Debug, PartialEq)]
pub struct GetPlaylistTracks(pub PlaylistID<'static>);

impl BackendTask<ArcServer> for GetPlaylistTracks {
    type Output = Result<Vec<PlaylistSong>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::GetPlaylistTracksQuery;
            use ytmapi_rs::parse::PlaylistItem;
            use crate::app::server::api::stream_api_with_retry_n;

            let api_guard = backend.api.get_api().await?;
            let query = GetPlaylistTracksQuery::new(self.0);

            match stream_api_with_retry_n(&api_guard, &query, 50).await {
                Ok(pages) => {
                    let items: Vec<PlaylistItem> = pages.into_iter().flatten().collect();
                    tracing::info!(count = %items.len(), "GetPlaylistTracks streaming done");
                    let songs: Vec<PlaylistSong> = items.into_iter().filter_map(|item| {
                        match item {
                            PlaylistItem::Song(s) => Some(s),
                            _ => None,
                        }
                    }).collect();
                    Ok(songs)
                }
                Err(e) => {
                    tracing::warn!("GetPlaylistTracks streaming failed: {}", e);
                    Err(anyhow::anyhow!("GetPlaylistTracks: {}", e))
                }
            }
        }
    }
}

impl BackendTask<ArcServer> for CreatePlaylistWithVideos {
    type Output = Result<PlaylistID<'static>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            let title = self.title;
            let description = self.description;
            let privacy = self.privacy;
            let all_ids = self.video_ids;
            let total = all_ids.len();
            tracing::info!("Creating playlist with {total} videos: {title}");

            // YouTube Music: 5000 songs max per playlist, API accepts all at once
            let max_per_playlist: usize = 5000;
            let mut remaining: Vec<VideoID<'static>> = all_ids;

            let mut first_playlist_id: Option<PlaylistID<'static>> = None;
            let mut playlist_index = 0;

            while !remaining.is_empty() {
                let playlist_songs: Vec<VideoID<'static>> = remaining.drain(..remaining.len().min(max_per_playlist)).collect();

                let playlist_title = if playlist_index == 0 {
                    title.clone()
                } else {
                    format!("{} pt. {}", title, playlist_index + 1)
                };
                playlist_index += 1;

                tracing::info!("Creating playlist #{playlist_index}: {playlist_title} ({} songs)", playlist_songs.len());

                let pid = backend.api.create_playlist_with_videos(
                    playlist_title,
                    description.clone(),
                    playlist_songs,
                    privacy.clone(),
                ).await?;

                if first_playlist_id.is_none() {
                    first_playlist_id = Some(pid);
                }
            }

            Ok(first_playlist_id.expect("at least one playlist should have been created"))
        }
    }
}

impl BackendTask<ArcServer> for AddSongsToPlaylist {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            let playlist_id = self.playlist_id;
            let all_ids = self.video_ids;
            let total = all_ids.len();
            // Deduplicate: YouTube API rejects duplicates in ReturnError mode
            let mut seen = std::collections::HashSet::new();
            let unique_ids: Vec<_> = all_ids.into_iter().filter(|id| seen.insert(id.clone())).collect();
            let deduped = total - unique_ids.len();
            if deduped > 0 {
                tracing::warn!("Removed {deduped} duplicate video IDs before adding to playlist");
            }
            tracing::info!("Adding {} videos to playlist in batches of 100", unique_ids.len());
            for chunk in unique_ids.chunks(100) {
                tracing::info!("Adding batch of {} videos", chunk.len());
                backend.api.add_playlist_items(
                    playlist_id.clone(),
                    chunk.to_vec(),
                ).await?;
            }
            Ok(())
        }
    }
}

impl BackendTask<ArcServer> for RenamePlaylist {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::EditPlaylistQuery;
            use ytmapi_rs::common::ApiOutcome;
            let api_guard = backend.api.get_api().await?;
            let query = EditPlaylistQuery::new_title(self.playlist_id, self.new_title);
            let _: ApiOutcome = api_guard.read().await.query_browser_or_oauth::<_, ApiOutcome>(query).await?;
            tracing::info!("Playlist renamed");
            Ok(())
        }
    }
}

impl BackendTask<ArcServer> for RemovePlaylistItems {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::RemovePlaylistItemsQuery;
            let api_guard = backend.api.get_api().await?;
            let set_ids: Vec<_> = self.video_ids.iter().map(|id| ytmapi_rs::common::SetVideoID::from_raw(id.get_raw().to_string())).collect();
            let query = RemovePlaylistItemsQuery::new(self.playlist_id, set_ids);
            api_guard.read().await.query_browser_or_oauth::<_, ()>(query).await?;
            tracing::info!("Playlist items removed");
            Ok(())
        }
    }
}

impl BackendTask<ArcServer> for DeletePlaylist {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::DeletePlaylistQuery;
            let api_guard = backend.api.get_api().await?;
            let query = DeletePlaylistQuery::new(self.0);
            api_guard.read().await.query_browser_or_oauth::<_, ()>(query).await?;
            tracing::info!("Playlist deleted");
            Ok(())
        }
    }
}

impl BackendTask<ArcServer> for EditPlaylistDetails {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::EditPlaylistQuery;
            use ytmapi_rs::common::ApiOutcome;
            let api_guard = backend.api.get_api().await?;
            // Apply each change sequentially. YTM API supports per-field edits.
            if let Some(title) = &self.title {
                let query = EditPlaylistQuery::new_title(&self.playlist_id, title.as_str());
                let _: ApiOutcome = api_guard.read().await.query_browser_or_oauth::<_, ApiOutcome>(query).await?;
            }
            if let Some(description) = &self.description {
                let query = EditPlaylistQuery::new_description(&self.playlist_id, description.as_str());
                let _: ApiOutcome = api_guard.read().await.query_browser_or_oauth::<_, ApiOutcome>(query).await?;
            }
            if let Some(privacy) = self.privacy {
                let query = EditPlaylistQuery::new_privacy_status(&self.playlist_id, privacy);
                let _: ApiOutcome = api_guard.read().await.query_browser_or_oauth::<_, ApiOutcome>(query).await?;
            }
            tracing::info!("Playlist details updated");
            Ok(())
        }
    }
}

impl BackendTask<ArcServer> for RatePlaylistMessage {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::RatePlaylistQuery;
            let api_guard = backend.api.get_api().await?;
            let query = RatePlaylistQuery::new(self.0, self.1);
            api_guard.read().await.query_browser_or_oauth::<_, ()>(query).await?;
            tracing::info!("Playlist rated");
            Ok(())
        }
    }
}

impl BackendTask<ArcServer> for GetPlaylistDetailsMessage {
    type Output = Result<GetPlaylistDetails>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::GetPlaylistDetailsQuery;
            let api_guard = backend.api.get_api().await?;
            let query = GetPlaylistDetailsQuery::new(self.0);
            api_guard.read().await.query_browser_or_oauth::<_, GetPlaylistDetails>(query).await
        }
    }
}

impl BackendTask<ArcServer> for ReorderPlaylistItem {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::EditPlaylistQuery;
            use ytmapi_rs::common::ApiOutcome;
            let api_guard = backend.api.get_api().await?;
            let set_id = SetVideoID::from_raw(self.video_id.get_raw().to_string());
            let target_set_id = SetVideoID::from_raw(self.target_video_id.get_raw().to_string());
            let query = EditPlaylistQuery::swap_videos_order(self.playlist_id, set_id, target_set_id);
            let _: ApiOutcome = api_guard.read().await.query_browser_or_oauth::<_, ApiOutcome>(query).await?;
            tracing::info!("Playlist item reordered");
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct DownloadSong(pub VideoID<'static>, pub ListSongID, pub Arc<CancellationToken>, pub AudioQuality);

impl PartialEq for DownloadSong {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

// Player Requests documentation:
// NOTE: I considered giving player more control of the playback than playlist,
// and increasing message size. However this seems to be more combinatorially
// difficult without a well defined data structure.

// XXX: This should be programmed to be unkillable.
// Case:
// Cur volume: 5
// Send IncreaseVolume(5)
// Send IncreaseVolume(5), killing previous task
// Volume will now be 10 - should be 15, should not allow caller to cause this.
// New note - 2025:
// SetVolume should be able to kill IncreaseVolume however...
#[derive(PartialEq, Debug)]
pub struct IncreaseVolume(pub i8);
#[derive(Debug, PartialEq)]
pub struct SetVolume(pub u8);
/// Seek forwards or backwards a duration in a song.
#[derive(Debug, PartialEq)]
pub struct Seek {
    pub duration: Duration,
    pub direction: SeekDirection,
}
/// Seek to a target position in a song.
#[derive(Debug, PartialEq)]
pub struct SeekTo {
    pub position: Duration,
    // Unlike seeking forward or back, it would be odd if user was expecting to seek to pos x in
    // song a but due to a race condition seek applied to song b.
    pub id: ListSongID,
}
/// Stop a song if it is still currently playing.
#[derive(Debug, PartialEq)]
pub struct Stop(pub ListSongID);
/// Stop the player, regardless of what song is playing.
#[derive(Debug, PartialEq)]
pub struct StopAll;
#[derive(Debug, PartialEq)]
pub struct PausePlay(pub ListSongID);
#[derive(Debug, PartialEq)]
pub struct Resume(pub ListSongID);
#[derive(Debug, PartialEq)]
pub struct Pause(pub ListSongID);
/// Decode a song into a format that can be played.
#[derive(PartialEq, Debug)]
pub struct DecodeSong(pub Arc<InMemSong>, pub Option<Duration>, pub Option<Duration>);
/// Play a song, starting from the start, regardless what's queued.
#[derive(Debug)]
pub struct PlaySong {
    pub song: DecodedInMemSong,
    pub id: ListSongID,
}
/// Play a song, unless it's already queued.
#[derive(Debug)]
pub struct AutoplaySong {
    pub song: DecodedInMemSong,
    pub id: ListSongID,
}
/// Queue a song to play next.
#[derive(Debug)]
pub struct QueueSong {
    pub song: DecodedInMemSong,
    pub id: ListSongID,
}
#[derive(Debug, PartialEq)]
pub struct GetSongThumbnail {
    pub thumbnail_url: String,
    pub thumbnail_id: SongThumbnailID<'static>,
}

impl BackendTask<ArcServer> for HandleApiError {
    // Infallible - assumption is that even if this task fails, caller won't care.
    type Output = ();
    // TODO: Review if TaskMetadata needs new enum cases.
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let Self { error, message } = self;
        let backend = backend.clone();
        async move {
            backend.api_error_handler.handle_error(error, message).await;
        }
    }
}

impl BackendTask<ArcServer> for GetLyrics {
    type Output = Result<String>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let http_client = backend.http_client.clone();
        async move {
            let artist = self.0;
            let title = self.1;
            // Primary: Genius (slug URL first, then API search)
            let genius = genius_rs::GeniusClient::new(Some(self.2), http_client.clone());
            match genius.find_and_fetch(&artist, &title).await {
                Ok((_hit, lyrics)) => {
                    tracing::info!("Genius lyrics: {} chars, {} lines",
                        lyrics.len(), lyrics.lines().count());
                    if lyrics.len() > 50 && lyrics.lines().count() > 2 {
                        return Ok(lyrics);
                    }
                }
                Err(e) => tracing::warn!("Genius fetch failed: {}", e),
            }

            // Fallback 1: bandcamp-lyrics CLI (great for niche/underground music)
            fn bc_slug(s: &str) -> String {
                s.to_lowercase().chars()
                    .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                    .collect::<String>()
                    .split_whitespace().collect::<Vec<_>>().join("-")
            }
            let bc_artist = bc_slug(&artist);
            let bc_title = bc_slug(&title);
            let bc_variants = [&bc_artist as &str];
            for artist_slug in &bc_variants {
                for suffix in &["", "-2", "-3", "-4", "-5"] {
                    let bc_url = format!("https://{}.bandcamp.com/track/{}{}", artist_slug, bc_title, suffix);
                    if let Ok(out) = tokio::process::Command::new("bandcamp-lyrics")
                        .arg(&bc_url).output().await
                    {
                        if out.status.success() {
                            let l = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            if !l.is_empty() {
                                tracing::info!("bandcamp found lyrics ({} chars)", l.len());
                                return Ok(l);
                            }
                        }
                    }
                }
            }

            // Fallback 2: lyr CLI
            let variants = [
                (&artist as &str, &title as &str),
                (artist.split(',').next().unwrap_or(&artist).trim(), &title),
            ];
            for (artist_v, title_v) in &variants {
                if let Ok(out) = tokio::process::Command::new("lyr")
                    .args(["--artist", artist_v, "--title", title_v])
                    .output().await
                {
                    if out.status.success() {
                        let raw = String::from_utf8_lossy(&out.stdout).to_string();
                        let l = raw.lines().skip(1).collect::<Vec<_>>().join("\n");
                        let l = l.splitn(2, "Lyrics").nth(1).unwrap_or(&l).trim().to_string();
                        if !l.is_empty() {
                            tracing::info!("lyr CLI found lyrics ({} chars)", l.len());
                            return Ok(l);
                        }
                    }
                }
            }

            Err(anyhow::anyhow!("No lyrics found from any provider"))
        }
    }
}

impl BackendTask<ArcServer> for GetSearchSuggestions {
    // TODO: Consider alternative where the text isn't returned back to the caller.
    type Output = Result<(Vec<SearchSuggestion>, String)>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.api.get_search_suggestions(self.0).await }
    }
}
impl BackendTask<ArcServer> for SearchArtists {
    type Output = Result<Vec<SearchResultArtist>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.api.search_artists(self.0).await }
    }
}
impl BackendTask<ArcServer> for SearchSongs {
    type Output = Result<Vec<SearchResultSong>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let query = self.0;
        let backend = backend.clone();
        async move {
            // Try YTMusic first
            match backend.api.search_songs(query.clone()).await {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => tracing::info!("YTMusic no results, trying YouTube fallback for: {}", query),
                Err(e) => tracing::warn!("YTMusic search error: {}, trying YouTube fallback", e),
            }
            // Fallback: yt-dlp YouTube search
            let output = tokio::process::Command::new("yt-dlp")
                .args([
                    "--flat-playlist", "--dump-json", "--no-warnings",
                    &format!("ytsearch10:{}", query),
                ])
                .output()
                .await
                .map_err(|e| anyhow::anyhow!("yt-dlp search failed: {}", e))?;

            fn extract_artist_from_title(title: &str, fallback: &str) -> String {
                // Title is often "Artist - Song Title" — extract artist before " - "
                if let Some(idx) = title.find(" - ") {
                    let candidate = title[..idx].trim();
                    if !candidate.is_empty() && candidate.len() < 80 {
                        return candidate.to_string();
                    }
                }
                fallback.to_string()
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let results: Vec<SearchResultSong> = stdout.lines()
                .filter_map(|line| {
                    let v: serde_json::Value = serde_json::from_str(line).ok()?;
                    let title = v.get("title")?.as_str()?;
                    let uploader = v.get("uploader").and_then(|u| u.as_str()).unwrap_or("Unknown");
                    let artist = extract_artist_from_title(title, uploader);
                    let id = v.get("id")?.as_str()?;
                    let d = v.get("duration").and_then(|s| s.as_f64()).unwrap_or(0.0) as u64;
                    let duration = format!("{}:{:02}", d / 60, d % 60);
                    let vid: VideoID<'static> = VideoID::from_raw(id.to_string());
                    let album_id: AlbumID<'static> = AlbumID::from_raw(id.to_string());
                    let artist_name = artist.clone();
                    Some(ytmapi_rs::parse::SearchResultSong::from_yt_dlp(
                        title.to_string(),
                        artist,
                        vid,
                        Some(ytmapi_rs::parse::ParsedSongAlbum {
                            name: format!("YouTube: {}", artist_name),
                            id: album_id,
                        }),
                        duration,
                    ))
                })
                .collect();
            Ok(results)
        }
    }
}
impl BackendTask<ArcServer> for GetAnnotations {
    type Output = Result<Vec<(String, String)>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = backend.http_client.clone();
        async move {
            let artist = self.0;
            let title = self.1;
            let token = self.2;

            // Use genius-rs with API-first annotations when token available
            let genius = genius_rs::GeniusClient::new(Some(token), client.clone());
            let hit = genius.find_song(&artist, &title).await
                .map_err(|e| anyhow::anyhow!("Genius search error: {}", e))?
                .ok_or_else(|| anyhow::anyhow!("No Genius results"))?;

            let annotations = genius.fetch_annotations_with_token(&hit.path, hit.id).await
                .map_err(|e| anyhow::anyhow!("Genius annotation fetch error: {}", e))?;

            let pairs: Vec<(String, String)> = annotations
                .into_iter()
                .map(|a| (a.fragment, a.body))
                .collect();

            tracing::info!("Fetched {} annotations for song {} via API+scrape", pairs.len(), hit.id);
            Ok(pairs)
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ValidatedMetadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub track_no: Option<usize>,
    pub album_tracks: Vec<AlbumTrack>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumTrack {
    pub title: String,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchAlbumArt(pub String, pub String, pub String);

impl BackendTask<ArcServer> for FetchAlbumArt {
    type Output = anyhow::Result<SongThumbnail>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let artist = self.0;
        let album = self.1;
        let api_key = self.2;
        let backend = backend.clone();
        let client = backend.http_client.clone();
        async move {
            // Query Last.fm album.getInfo for image URL
            let info_url = format!(
                "https://ws.audioscrobbler.com/2.0/?method=album.getInfo&api_key={}&artist={}&album={}&format=json",
                api_key, urlencoding(&artist), urlencoding(&album)
            );
            let image_url = if api_key.is_empty() {
                None
            } else if let Ok(resp) = client.get(&info_url).send().await {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    data.get("album")
                        .and_then(|a| a.get("image"))
                        .and_then(|imgs| imgs.as_array())
                        .and_then(|arr| {
                            arr.iter()
                                .max_by_key(|img| {
                                    img.get("size")
                                        .and_then(|s| s.as_str())
                                        .map(|s| match s {
                                            "small" => 0usize,
                                            "medium" => 1,
                                            "large" => 2,
                                            "extralarge" => 3,
                                            "mega" => 4,
                                            _ => 0,
                                        })
                                        .unwrap_or(0)
                                })
                                .and_then(|img| img.get("#text").and_then(|t| t.as_str()))
                                .map(|s| s.to_string())
                        })
                } else {
                    None
                }
            } else {
                None
            };

            match image_url {
                Some(url) if !url.is_empty() => {
                    let thumb_id = SongThumbnailID::Album(ytmapi_rs::common::AlbumID::from_raw(
                        format!("lastfm:{}:{}", artist, album),
                    ));
                    backend
                        .song_thumbnail_downloader
                        .download_song_thumbnail(thumb_id, url)
                        .await
                }
                _ => Err(anyhow::anyhow!("No album art URL found on Last.fm")),
            }
        }
    }
}

impl BackendTask<ArcServer> for ValidateMetadata {
    type Output = Result<ValidatedMetadata>;
    type MetadataType = TaskMetadata;
    fn into_future(self, backend: &ArcServer) -> impl Future<Output = Self::Output> + Send + 'static {
        let registry = backend.metadata_registry.clone();
        async move {
            let artist = self.0;
            let title = self.1;
            let _song_id = self.2;
            registry.resolve(&artist, &title).await
        }
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            ' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", c as u8));
            }
        }
    }
    out
}

impl BackendTask<ArcServer> for SearchPlaylists {
    type Output = Result<Vec<SearchResultPlaylist>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        let query = self.0;
        async move {
            match backend.api.search_playlists(query.clone()).await {
                Ok(playlists) => Ok(playlists),
                Err(e) => {
                    tracing::warn!("Playlist search failed (YTM API): {}. Returning empty.", e);
                    Ok(Vec::new()) // return empty instead of error
                }
            }
        }
    }
}
impl BackendTask<ArcServer> for SearchAlbums {
    type Output = Result<Vec<SearchResultAlbum>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        let query = self.0;
        async move {
            match backend.api.search_albums(query.clone()).await {
                Ok(albums) => Ok(albums),
                Err(e) => {
                    tracing::warn!("Album search failed (YTM API): {}. Returning empty.", e);
                    Ok(Vec::new())
                }
            }
        }
    }
}
impl BackendStreamingTask<ArcServer> for GetArtistSongs {
    type Output = GetArtistSongsProgressUpdate;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl futures::Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.api.get_artist_songs(self.0)
    }
}
impl BackendStreamingTask<ArcServer> for GetPlaylistSongs {
    type Output = GetPlaylistSongsProgressUpdate;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl futures::Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend
            .api
            .get_playlist_songs(self.playlist_id, self.max_songs)
    }
}

impl BackendStreamingTask<ArcServer> for DownloadSong {
    type Output = DownloadProgressUpdate;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl futures::Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.song_downloader.download_song(self.0, self.1, Some(self.2), self.3)
    }
}
impl BackendTask<ArcServer> for Seek {
    type Output = Option<ProgressUpdate<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.seek(self.duration, self.direction).await }
    }
}
impl BackendTask<ArcServer> for SeekTo {
    type Output = Option<ProgressUpdate<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.seek_to(self.position, self.id).await }
    }
}
impl BackendTask<ArcServer> for DecodeSong {
    type Output = std::result::Result<DecodedInMemSong, DecoderError>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        _backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        Player::try_decode(self.0, self.1, self.2)
    }
}
impl BackendTask<ArcServer> for IncreaseVolume {
    type Output = Option<VolumeUpdate>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.increase_volume(self.0).await }
    }
}
impl BackendTask<ArcServer> for SetVolume {
    type Output = Option<VolumeUpdate>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.set_volume(self.0).await }
    }
}
impl BackendTask<ArcServer> for Stop {
    type Output = Option<Stopped<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.stop(self.0).await }
    }
}
impl BackendTask<ArcServer> for StopAll {
    type Output = Option<AllStopped>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.stop_all().await }
    }
}
impl BackendTask<ArcServer> for PausePlay {
    type Output = Option<PausePlayResponse<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.pause_play(self.0).await }
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayPause]
    }
}
impl BackendTask<ArcServer> for Resume {
    type Output = Option<Resumed<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.resume(self.0).await }
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayPause]
    }
}
impl BackendTask<ArcServer> for Pause {
    type Output = Option<Paused<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.pause(self.0).await }
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayPause]
    }
}

impl BackendStreamingTask<ArcServer> for PlaySong {
    type Output = PlayUpdate<ListSongID>;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.player.play_song(self.song, self.id)
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayingSong]
    }
}
impl BackendStreamingTask<ArcServer> for AutoplaySong {
    type Output = AutoplayUpdate<ListSongID>;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.player.autoplay_song(self.song, self.id)
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayingSong]
    }
}
impl BackendStreamingTask<ArcServer> for QueueSong {
    type Output = QueueUpdate<ListSongID>;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.player.queue_song(self.song, self.id)
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayingSong]
    }
}
impl BackendTask<ArcServer> for GetSongThumbnail {
    type Output = anyhow::Result<SongThumbnail>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            backend
                .song_thumbnail_downloader
                .download_song_thumbnail(self.thumbnail_id, self.thumbnail_url)
                .await
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct PlayDecodedSong(pub ListSongID);
impl MapFn<DecodedInMemSong> for PlayDecodedSong {
    type Output = PlaySong;
    fn apply(self, input: DecodedInMemSong) -> Self::Output {
        tracing::info!("Song decoded succesfully. {:?}", self.0);
        PlaySong {
            song: input,
            id: self.0,
        }
    }
}
#[derive(PartialEq, Debug)]
pub struct AutoplayDecodedSong(pub ListSongID);
impl MapFn<DecodedInMemSong> for AutoplayDecodedSong {
    type Output = AutoplaySong;
    fn apply(self, input: DecodedInMemSong) -> Self::Output {
        tracing::info!("Song decoded succesfully. {:?}", self.0);
        AutoplaySong {
            song: input,
            id: self.0,
        }
    }
}
#[derive(PartialEq, Debug)]
pub struct QueueDecodedSong(pub ListSongID);
impl MapFn<DecodedInMemSong> for QueueDecodedSong {
    type Output = QueueSong;
    fn apply(self, input: DecodedInMemSong) -> Self::Output {
        tracing::info!("Song decoded succesfully. {:?}", self.0);
        QueueSong {
            song: input,
            id: self.0,
        }
    }
}

/// It's not possible to compare some of these Tasks type due to the underlying
/// type, but because tests and some ci run with async_callback_manager's
/// "task-equality" enabled, a PartialEq impl is required. It's acceptable to
/// panic as running .eq() on these types is a logic error AND should only occur
/// during testing.
#[cfg(any(test, clippy))]
#[allow(unexpected_cfgs)]
mod test_config {
    use crate::app::server::{AutoplaySong, HandleApiError, PlaySong, QueueSong};

    impl PartialEq for HandleApiError {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare HandleApiError");
        }
    }
    impl PartialEq for PlaySong {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare PlaySong");
        }
    }
    impl PartialEq for AutoplaySong {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare AutoplaySong");
        }
    }
    impl PartialEq for QueueSong {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare QueueSong");
        }
    }
}
