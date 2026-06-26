use crate::api::{DynamicApiError, DynamicYtMusic};
use crate::app::CALLBACK_CHANNEL_SIZE;
use audio_player::send_or_error;
use crate::config::ApiKey;
use crate::{OAUTH_FILENAME, get_config_dir};
use anyhow::{Error, Result};
use async_callback_manager::PanickingReceiverStream;
use async_cell::sync::AsyncCell;
use futures::stream::FuturesOrdered;
use futures::{Stream, StreamExt};
use std::borrow::Borrow;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{error, info};
use ytmapi_rs::auth::{BrowserToken, OAuthToken};
use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, SearchSuggestion, Thumbnail, VideoID, LikeStatus, YoutubeID};
use ytmapi_rs::parse::{
    AlbumSong, GetAlbum, GetArtistAlbums, ParsedSongAlbum, ParsedSongArtist, PlaylistItem,
    SearchResultAlbum, SearchResultArtist, SearchResultPlaylist, SearchResultSong,
};
use ytmapi_rs::continuations::ParseFromContinuable;
use ytmapi_rs::query::{
    GetAlbumQuery, GetArtistAlbumsQuery, PostQuery,
};
use ytmapi_rs::query::playlist::{
    PrivacyStatus, CreatePlaylistQuery, DuplicateHandlingMode, AddPlaylistItemsQuery,
};

#[derive(Clone)]
/// # Note
/// Since the underlying API is wrapped in an Arc, it's cheap to clone this
/// type.
pub struct Api {
    api: Arc<AsyncCell<Result<ConcurrentApi, DynamicApiError>>>,
}
pub type ConcurrentApi = Arc<RwLock<DynamicYtMusic>>;

impl Api {
    pub fn new(api_key: ApiKey) -> Api {
        let api = AsyncCell::new().into_shared();
        let api_clone = api.clone();
        tokio::spawn(async move {
            let api = DynamicYtMusic::new(api_key)
                .await
                .map(|api| Arc::new(RwLock::new(api)));
            api_clone.set(api)
        });
        Api { api }
    }
    // NOTE: Situation where user has tried to create API from an expired OAuth
    // token is not currently handled.
    pub async fn get_api(&self) -> Result<ConcurrentApi, DynamicApiError> {
        // Note that the error, if it exists, is cloned here.
        self.api.get().await
    }
    pub async fn get_search_suggestions(
        &self,
        text: String,
    ) -> Result<(Vec<SearchSuggestion>, String)> {
        get_search_suggestions(self.get_api().await?, text).await
    }
    pub async fn search_playlists(&self, text: String) -> Result<Vec<SearchResultPlaylist>> {
        search_playlists(self.get_api().await?, text).await
    }
    pub async fn search_albums(&self, text: String) -> Result<Vec<SearchResultAlbum>> {
        search_albums(self.get_api().await?, text).await
    }
    pub async fn search_artists(&self, text: String) -> Result<Vec<SearchResultArtist>> {
        search_artists(self.get_api().await?, text).await
    }
    pub async fn search_songs(&self, text: String) -> Result<Vec<SearchResultSong>> {
        search_songs(self.get_api().await?, text).await
    }
    pub async fn create_playlist_with_videos(
        &self,
        title: String,
        description: Option<String>,
        video_ids: Vec<VideoID<'static>>,
        privacy: Option<ytmapi_rs::query::playlist::PrivacyStatus>,
    ) -> Result<PlaylistID<'static>> {
        create_playlist_with_videos(self.get_api().await?, title, description, video_ids, privacy).await
    }
    pub async fn add_playlist_items(
        &self,
        playlist_id: PlaylistID<'static>,
        video_ids: Vec<VideoID<'static>>,
    ) -> Result<()> {
        add_playlist_items(self.get_api().await?, playlist_id, video_ids).await
    }
    pub async fn rate_song(
        &self,
        video_id: VideoID<'static>,
        rating: LikeStatus,
    ) -> Result<()> {
        let api = self.get_api().await?;
        api.read().await.rate_song(video_id, rating).await
    }
    pub fn get_playlist_songs(
        &self,
        playlist_id: PlaylistID<'static>,
        max_results: usize,
    ) -> impl Stream<Item = GetPlaylistSongsProgressUpdate> + 'static + use<> {
        let api = self.api.clone();
        get_playlist_songs(api, playlist_id, max_results)
    }
    pub fn get_artist_songs(
        &self,
        browse_id: ArtistChannelID<'static>,
    ) -> impl Stream<Item = GetArtistSongsProgressUpdate> + 'static + use<> {
        let api = self.api.clone();
        get_artist_songs(api, browse_id)
    }
}

/// Update the local oauth token file.
async fn update_oauth_token_file(token: OAuthToken) -> Result<()> {
    let mut file_path = get_config_dir()?;
    file_path.push(OAUTH_FILENAME);
    let mut tmpfile_path = file_path.clone();
    tmpfile_path.set_extension("json.tmp");
    let out = serde_json::to_string_pretty(&token)?;
    info!("Updating oauth token at: {:?}", &file_path);
    let mut file = tokio::fs::File::create_new(&tmpfile_path).await?;
    file.write_all(out.as_bytes()).await?;
    tokio::fs::rename(tmpfile_path, &file_path).await?;
    info!("Updated oauth token at: {:?}", file_path);
    Ok(())
}

/// Run a query. If the oauth token is expired, take the lock and refresh
/// it (single retry only). If another error occurs, try a single retry too.
pub async fn query_api_with_retry<Q, O>(api: &ConcurrentApi, query: impl Borrow<Q>) -> Result<O>
where
    Q: ytmapi_rs::query::Query<BrowserToken, Output = O>,
    Q: ytmapi_rs::query::Query<OAuthToken, Output = O>,
    Q: ytmapi_rs::query::Query<NoAuthToken, Output = O>,
{
    let res = api
        .read()
        .await
        .query::<Q, O>(query.borrow())
        .await;
    match res {
        Ok(r) => Ok(r),
        Err(e) => {
            info!("Got error {e} from api");
            match e.downcast::<ytmapi_rs::Error>().map(|e| e.into_kind()) {
                Ok(ytmapi_rs::error::ErrorKind::OAuthTokenExpired { token_hash }) => {
                    // Take a clone to re-use later.
                    let api_clone = api.to_owned();
                    // First take an exclusive lock - prevent others from doing the same.
                    let api_owned = api_clone.clone();
                    let mut api_locked = api_owned.write_owned().await;
                    // Then check to see if the token_hash hasn't changed since calling the
                    // query. If it hasn't, we were the first one and are responsible for
                    // refreshing. If it has, that means another query must have
                    // already refreshed the token, and we don't need to do
                    // anything.
                    let api_token_hash = api_locked.get_token_hash()?;
                    if api_token_hash == Some(token_hash) {
                        // A task is spawned to refresh the token, to ensure that it still
                        // refreshes even if this task is
                        // cancelled.
                        tokio::spawn(async {
                            info!("Refreshing oauth token");
                            let tok = api_locked.refresh_token().await?
                                .ok_or_else(|| anyhow::anyhow!("refresh_token returned None after OAuthTokenExpired"))?;
                            info!("Oauth token refreshed");
                            if let Err(e) = update_oauth_token_file(tok).await {
                                error!("Error updating locally saved oauth token: <{e}>")
                            }
                            Ok::<_,anyhow::Error>(api_locked)
                        }).await??;
                    }
                    Ok(api_clone
                        .read_owned()
                        .await
                        .query::<Q, O>(query)
                        .await?)
                }
                // Regular retry without token refresh, if token isn't expired.
                Ok(_) => {
                    info!("Retrying once");
                    Ok(api.read().await.query::<Q, O>(query).await?)
                }
                // If the DynamicApi didn't return a ytmapi_rs::Error, the error must be
                // non-retryable.
                Err(e) => Err(e),
            }
        }
    }
}

/// Like `query_api_with_retry` but for Browser+OAuth-only queries via stream.
pub async fn stream_api_with_retry_n<Q, O>(
    api: &ConcurrentApi,
    query: &Q,
    max_pages: usize,
) -> Result<Vec<O>>
where
    Q: ytmapi_rs::query::Query<BrowserToken, Output = O>,
    Q: ytmapi_rs::query::Query<OAuthToken, Output = O>,
    O: ParseFromContinuable<Q>,
    Q: PostQuery,
{
    let res = api
        .read()
        .await
        .stream_browser_or_oauth(query, max_pages)
        .await;
    match res {
        Ok(r) => Ok(r),
        Err(e) => {
            info!("Got error {e} from api stream");
            match e.downcast::<ytmapi_rs::Error>().map(|e| e.into_kind()) {
                Ok(ytmapi_rs::error::ErrorKind::OAuthTokenExpired { token_hash }) => {
                    let api_clone = api.to_owned();
                    let api_owned = api_clone.clone();
                    let mut api_locked = api_owned.write_owned().await;
                    let api_token_hash = api_locked.get_token_hash()?;
                    if api_token_hash == Some(token_hash) {
                        tokio::spawn(async {
                            info!("Refreshing oauth token");
                            let tok = api_locked.refresh_token().await?
                                .ok_or_else(|| anyhow::anyhow!("refresh_token returned None after OAuthTokenExpired"))?;
                            info!("Oauth token refreshed");
                            if let Err(e) = update_oauth_token_file(tok).await {
                                error!("Error updating locally saved oauth token: <{e}>")
                            }
                            Ok::<_, anyhow::Error>(api_locked)
                        }).await??;
                    }
                    Ok(api_clone
                        .read_owned()
                        .await
                        .stream_browser_or_oauth(query, max_pages)
                        .await?)
                }
                Ok(_) => {
                    info!("Retrying once");
                    Ok(api.read().await.stream_browser_or_oauth(query, max_pages).await?)
                }
                Err(e) => Err(e),
            }
        }
    }
}

async fn search_playlists(api: ConcurrentApi, text: String) -> Result<Vec<SearchResultPlaylist>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    tracing::info!("Searching playlists for {text}");
    let query = ytmapi_rs::query::SearchQuery::new_filtered(
        text,
        ytmapi_rs::query::search::PlaylistsFilter,
    )
    .with_spelling_mode(ytmapi_rs::query::search::SpellingMode::ExactMatch);
    query_api_with_retry(&api, query).await
}

async fn search_albums(api: ConcurrentApi, text: String) -> Result<Vec<SearchResultAlbum>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    tracing::info!("Searching albums for {text}");
    let query = ytmapi_rs::query::SearchQuery::new_filtered(
        text,
        ytmapi_rs::query::search::AlbumsFilter,
    )
    .with_spelling_mode(ytmapi_rs::query::search::SpellingMode::ExactMatch);
    query_api_with_retry(&api, query).await
}

async fn search_artists(api: ConcurrentApi, text: String) -> Result<Vec<SearchResultArtist>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    tracing::info!("Searching artists for {text}");
    let query = ytmapi_rs::query::SearchQuery::new_filtered(
        text,
        ytmapi_rs::query::search::ArtistsFilter,
    )
    .with_spelling_mode(ytmapi_rs::query::search::SpellingMode::ExactMatch);
    query_api_with_retry(&api, query).await
}

async fn search_songs(api: ConcurrentApi, text: String) -> Result<Vec<SearchResultSong>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    tracing::info!("Searching songs for {text}");
    let query = ytmapi_rs::query::SearchQuery::new_filtered(
        text,
        ytmapi_rs::query::search::SongsFilter,
    )
    .with_spelling_mode(ytmapi_rs::query::search::SpellingMode::ExactMatch);
    query_api_with_retry(&api, query).await
}

async fn create_playlist_with_videos(
    api: ConcurrentApi,
    title: String,
    description: Option<String>,
    video_ids: Vec<VideoID<'static>>,
    privacy: Option<ytmapi_rs::query::playlist::PrivacyStatus>,
) -> Result<PlaylistID<'static>> {
    tracing::info!("Creating playlist with {} videos: {}", video_ids.len(), title);
    let privacy = privacy.unwrap_or(PrivacyStatus::Unlisted);
    let query = CreatePlaylistQuery::new(&title, description.as_deref(), privacy)
        .with_video_ids(video_ids);
    query_api_with_retry(&api, query).await
}

async fn add_playlist_items(
    api: ConcurrentApi,
    playlist_id: PlaylistID<'static>,
    video_ids: Vec<VideoID<'static>>,
) -> Result<()> {
    tracing::info!("Adding {} videos to playlist", video_ids.len());
    // Strip VL prefix — browse format != API format for add endpoint
    let raw = playlist_id.get_raw();
    let clean_id = if let Some(stripped) = raw.strip_prefix("VL") {
        PlaylistID::from_raw(stripped.to_string())
    } else {
        playlist_id
    };
    let query =
        AddPlaylistItemsQuery::new_from_videos(clean_id, video_ids, DuplicateHandlingMode::Unhandled);
    query_api_with_retry(&api, query).await.map(|_: Vec<ytmapi_rs::parse::AddPlaylistItem>| ())
}

pub async fn get_search_suggestions(
    api: ConcurrentApi,
    text: String,
) -> Result<(Vec<SearchSuggestion>, String)> {
    tracing::info!("Getting search suggestions for {text}");
    let query = ytmapi_rs::query::GetSearchSuggestionsQuery::new(&text);
    let results = query_api_with_retry(&api, query).await?;
    Ok((results, text))
}

pub enum GetArtistSongsProgressUpdate {
    Loading,
    // Caller should know the ArtistChannelID already, as they provided it.
    // Stream closes here.
    GetArtistAlbumsError(Error),
    // Stream doesn't close here - maybe some of the other albums were succesfully able to send
    // songs.
    GetAlbumsSongsError {
        album_id: AlbumID<'static>,
        error: Error,
    },
    SongsFound,
    Songs {
        song_list: Vec<AlbumSong>,
        album: ParsedSongAlbum,
        year: String,
        artists: Vec<ParsedSongArtist>,
        thumbnails: Vec<Thumbnail>,
    },
    // Stream closes here.
    AllSongsSent,
    // Stream closes here.
    NoSongsFound,
}

fn get_artist_songs(
    api: Arc<AsyncCell<Result<ConcurrentApi, DynamicApiError>>>,
    browse_id: ArtistChannelID<'static>,
) -> impl Stream<Item = GetArtistSongsProgressUpdate> + 'static {
    let (tx, rx) = tokio::sync::mpsc::channel(CALLBACK_CHANNEL_SIZE);
    let handle = tokio::spawn(async move {
        tracing::info!("Running songs query");
        send_or_error(&tx, GetArtistSongsProgressUpdate::Loading).await;
        let api = match api.get().await {
            Err(e) => {
                error!("Error getting API");
                send_or_error(
                    tx,
                    GetArtistSongsProgressUpdate::GetArtistAlbumsError(e.into()),
                )
                .await;
                return;
            }
            Ok(api) => api,
        };
        let query = ytmapi_rs::query::GetArtistQuery::new(&browse_id);
        let artist = query_api_with_retry(&api, query).await;
        let artist = match artist {
            Ok(a) => a,
            Err(e) => {
                error!("Error with GetArtistQuery");
                send_or_error(tx, GetArtistSongsProgressUpdate::GetArtistAlbumsError(e)).await;
                return;
            }
        };
        // Process both albums and singles/EPs sections from the artist page
        async fn process_section(
            api: &ConcurrentApi,
            tx: &tokio::sync::mpsc::Sender<GetArtistSongsProgressUpdate>,
            section: Option<GetArtistAlbums>,
            section_type: Option<&str>,
        ) -> Option<Vec<(AlbumID<'static>, Option<String>)>> {
            let GetArtistAlbums {
                browse_id: section_browse_id,
                params: section_params,
                results: section_results,
                ..
            } = section?;
            let category_fallback = || section_type.map(|s| s.to_string());
            if section_browse_id.is_none()
                && section_params.is_none()
                && !section_results.is_empty()
            {
                return Some(section_results.into_iter().map(|r| {
                    let cat = r.album_type.map(|t| format!("{t:?}")).or_else(category_fallback);
                    (r.album_id, cat)
                }).collect());
            }
            if section_params.is_none() || section_browse_id.is_none() {
                return None;
            }
            let temp_browse_id = section_browse_id?;
            let temp_params = section_params?;
            let query = GetArtistAlbumsQuery::new(temp_browse_id, temp_params);
            match query_api_with_retry(&api, query).await {
                Ok(albums) => Some(albums.into_iter().map(|a| (a.browse_id, a.category.or_else(category_fallback))).collect()),
                Err(e) => {
                    error!("Received error on get_artist_albums query \"{}\"", e);
                    send_or_error(tx, GetArtistSongsProgressUpdate::GetArtistAlbumsError(e)).await;
                    None
                }
            }
        }

        let mut browse_id_list: Vec<(AlbumID<'static>, Option<String>)> = Vec::new();
        if let Some(albums) = process_section(&api, &tx, artist.top_releases.albums, Some("Album")).await {
            tracing::info!("get_artist_albums: found {} albums", albums.len());
            browse_id_list.extend(albums);
        }
        if let Some(singles) = process_section(&api, &tx, artist.top_releases.singles, Some("Single")).await {
            tracing::info!("get_artist_albums: found {} singles", singles.len());
            browse_id_list.extend(singles);
        }
        tracing::info!("get_artist_albums: total {} albums+singles", browse_id_list.len());
        if browse_id_list.is_empty() {
            tracing::info!("Telling caller no songs found (no albums or singles)");
            send_or_error(&tx, GetArtistSongsProgressUpdate::NoSongsFound).await;
            return;
        }
        send_or_error(&tx, GetArtistSongsProgressUpdate::SongsFound).await;
        // Request all albums, concurrently but retaining order.
        // Future improvement: instead of using a FuturesOrdered, we could send
        // willy-nilly but with an index, so the caller can insert songs in place.
        let mut stream = browse_id_list
            .into_iter()
            .inspect(|(a_id, _)| {
                tracing::info!("Spawning request for caller tracks for album ID {:?}", a_id,)
            })
            .map(|(a_id, category)| {
                let api = api.clone();
                async move {
                    let query = GetAlbumQuery::new(&a_id);
                    (query_api_with_retry(&api, query).await, a_id, category)
                }
            })
            .collect::<FuturesOrdered<_>>();
        while let Some((maybe_album, album_id, category)) = stream.next().await {
            let album = match maybe_album {
                Ok(album) => album,
                Err(e) => {
                    error!("Error with GetAlbumQuery, album {:?}", album_id);
                    send_or_error(
                        &tx,
                        GetArtistSongsProgressUpdate::GetAlbumsSongsError { album_id, error: e },
                    )
                    .await;
                    continue;
                }
            };
            let GetAlbum {
                title,
                artists,
                year,
                tracks,
                thumbnails,
                ..
            } = album;
            let display_name = if let Some(ref cat) = category {
                format!("{cat}: {title}")
            } else {
                title
            };
            tracing::info!("Sending caller tracks for artist {:?}", browse_id);
            send_or_error(
                &tx,
                GetArtistSongsProgressUpdate::Songs {
                    song_list: tracks,
                    album: ParsedSongAlbum {
                        name: display_name,
                        id: album_id,
                    },
                    year,
                    artists,
                    thumbnails,
                },
            )
            .await;
        }
        send_or_error(tx, GetArtistSongsProgressUpdate::AllSongsSent).await;
    });
    PanickingReceiverStream::new(rx, handle)
}

pub enum GetPlaylistSongsProgressUpdate {
    Loading,
    Songs(Vec<PlaylistItem>),
    // PlaylistID is returned to allow caller to reuse allocation if required.
    // May occur before or after sending some songs, ie api could fail straight away or stream
    // some songs then fail. Stream closes here.
    GetPlaylistSongsError {
        playlist_id: PlaylistID<'static>,
        error: Error,
    },
    // Stream closes here.
    AllSongsSent,
}

fn get_playlist_songs(
    api: Arc<AsyncCell<Result<ConcurrentApi, DynamicApiError>>>,
    playlist_id: PlaylistID<'static>,
    _max_results: usize,
) -> impl Stream<Item = GetPlaylistSongsProgressUpdate> + 'static {
    let (tx, rx) = tokio::sync::mpsc::channel(CALLBACK_CHANNEL_SIZE);
    let handle = tokio::spawn(async move {
        tracing::info!("Running songs query");
        send_or_error(&tx, GetPlaylistSongsProgressUpdate::Loading).await;
        let api = match api.get().await {
            Err(e) => {
                error!("Error getting API");
                send_or_error(
                    tx,
                    GetPlaylistSongsProgressUpdate::GetPlaylistSongsError {
                        playlist_id,
                        error: e.into(),
                    },
                )
                .await;
                return;
            }
            Ok(api) => api,
        };
        let query = ytmapi_rs::query::GetPlaylistTracksQuery::new((&playlist_id).into());
        // TODO: Streaming
        let first_tracks = query_api_with_retry(&api, query).await;
        match first_tracks {
            Ok(t) => {
                info!("Sending caller tracks for {:?}", playlist_id);
                send_or_error(&tx, GetPlaylistSongsProgressUpdate::Songs(t)).await;
            }
            Err(error) => {
                error!("Error with GetPlaylistTracksQuery");
                send_or_error(
                    &tx,
                    GetPlaylistSongsProgressUpdate::GetPlaylistSongsError { playlist_id, error },
                )
                .await;
                return;
            }
        }
        send_or_error(tx, GetPlaylistSongsProgressUpdate::AllSongsSent).await;
    });
    PanickingReceiverStream::new(rx, handle)
}
