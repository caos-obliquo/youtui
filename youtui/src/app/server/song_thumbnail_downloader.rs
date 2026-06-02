use crate::app::structures::{AlbumOrUploadAlbumID, ListSong, ListSongAlbum};
use crate::core::create_or_clean_directory;
use crate::get_data_dir;
use anyhow::{Context, anyhow};
use async_cell::sync::AsyncCell;
use futures::FutureExt;
use futures::future::try_join;
use lru::LruCache;
use rusty_ytdl::reqwest;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use ytmapi_rs::common::{AlbumID, UploadAlbumID, VideoID, YoutubeID};

// The directory and prefix are to protect the user - files in this directory
// with this prefix will be monitored by youtui and cleaned up when over a
// certain age.
const ALBUM_ART_DIR_PATH: &str = "album_art";
// "Youtui Album Art" if you were wondering.
const ALBUM_ART_FILENAME_PREFIX: &str = "YAA_";
const ALBUM_ART_IMAGE_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(60 * 60 * 24 * 10); //10 days

fn get_album_art_dir() -> anyhow::Result<PathBuf> {
    get_data_dir().map(|dir| dir.join(ALBUM_ART_DIR_PATH))
}

/// Unique identifier for the thumbnail - dependent on the type of song.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum SongThumbnailID<'a> {
    Album(AlbumID<'a>),
    UploadAlbum(UploadAlbumID<'a>),
    Video(VideoID<'a>),
}
impl<'a> From<&'a ListSong> for SongThumbnailID<'a> {
    fn from(song: &'a ListSong) -> SongThumbnailID<'a> {
        match song.album.as_deref() {
            Some(ListSongAlbum {
                id: AlbumOrUploadAlbumID::Album(a),
                ..
            }) => SongThumbnailID::Album(a.into()),
            Some(ListSongAlbum {
                id: AlbumOrUploadAlbumID::UploadAlbum(a),
                ..
            }) => SongThumbnailID::UploadAlbum(a.into()),
            None => SongThumbnailID::Video((&song.video_id).into()),
        }
    }
}
impl std::fmt::Display for SongThumbnailID<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SongThumbnailID::Album(id) => write!(f, "A_{}", id.get_raw()),
            SongThumbnailID::UploadAlbum(id) => write!(f, "U_{}", id.get_raw()),
            SongThumbnailID::Video(id) => write!(f, "V_{}", id.get_raw()),
        }
    }
}
impl<'a> SongThumbnailID<'a> {
    /// Convert the SongThumbnailID to static lifetime (by cloning the
    /// underlying data).
    pub fn into_owned(self) -> SongThumbnailID<'static> {
        match self {
            SongThumbnailID::Album(id) => {
                let id_string = id.get_raw().to_owned();
                SongThumbnailID::Album(AlbumID::from_raw(id_string))
            }
            SongThumbnailID::UploadAlbum(id) => {
                let id_string = id.get_raw().to_owned();
                SongThumbnailID::UploadAlbum(UploadAlbumID::from_raw(id_string))
            }
            SongThumbnailID::Video(id) => {
                let id_string = id.get_raw().to_owned();
                SongThumbnailID::Video(VideoID::from_raw(id_string))
            }
        }
    }
}

#[derive(PartialEq)]
pub struct SongThumbnail {
    pub in_mem_image: image::DynamicImage,
    pub on_disk_path: std::path::PathBuf,
    pub song_thumbnail_id: SongThumbnailID<'static>,
}

// Custom debug format - otherwise in_mem_image will be displaying array of
// bytes...
impl std::fmt::Debug for SongThumbnail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlbumArt")
            .field("in_mem_image", &"image::DynamicImage")
            .field("on_disk_path", &self.on_disk_path)
            .field("song_thumbnail_id", &self.song_thumbnail_id)
            .finish()
    }
}

impl Clone for SongThumbnail {
    fn clone(&self) -> Self {
        Self {
            in_mem_image: self.in_mem_image.clone(),
            on_disk_path: self.on_disk_path.clone(),
            song_thumbnail_id: self.song_thumbnail_id.clone(),
        }
    }
}

#[derive(Clone)]
pub struct SongThumbnailDownloader {
    client: reqwest::Client,
    // In-memory LRU cache for thumbnails to avoid re-downloading.
    cache: Arc<Mutex<LruCache<SongThumbnailID<'static>, SongThumbnail>>>,
    // For information about why this error is stringly typed, see DynamicApiError
    status: Arc<AsyncCell<Result<(), String>>>,
}

const THUMBNAIL_CACHE_SIZE: usize = 100;

impl SongThumbnailDownloader {
    pub fn new(client: reqwest::Client) -> Self {
        let cache = Arc::new(Mutex::new(LruCache::new(
            std::num::NonZeroUsize::new(THUMBNAIL_CACHE_SIZE).expect("THUMBNAIL_CACHE_SIZE must be greater than 0"),
        )));
        let cache_clone = cache.clone();
        let status = AsyncCell::new().into_shared();
        let status_clone = status.clone();
        tokio::spawn(async move {
            info!("Setting up and cleaning album art directory");
            let Ok(album_art_dir) = get_album_art_dir() else {
                status_clone.set(Err("Error getting album art dir".to_string()));
                return;
            };
            match create_or_clean_directory(
                &album_art_dir,
                ALBUM_ART_FILENAME_PREFIX,
                ALBUM_ART_IMAGE_MAX_AGE,
            )
            .await
            {
                Ok(n) => {
                    info!("Cleaned up {n} old album art files");
                    status_clone.set(Ok(()));
                }
                Err(e) => {
                    error!("Error {e} setting up and cleaning album art directory");
                    status_clone.set(Err(format!("{e}")))
                }
            }
        });
        Self {
            client,
            cache: cache_clone,
            status,
        }
    }
    pub async fn download_song_thumbnail(
        &self,
        thumbnail_id: SongThumbnailID<'static>,
        thumbnail_url: String,
    ) -> anyhow::Result<SongThumbnail> {
        // Check in-memory cache first.
        let thumbnail_id_owned = thumbnail_id.clone().into_owned();
        {
            let mut cache = self.cache.lock().await;
            if let Some(cached) = cache.get(&thumbnail_id_owned) {
                info!("Thumbnail cache hit for {}", thumbnail_id);
                return Ok(cached.clone());
            }
        }
        // Do not download album art until directory setup and clean has completed.
        self.status.get().await.map_err(|e| anyhow!(e))?;
        let url = reqwest::Url::parse(&thumbnail_url)?;
        let image_bytes = self.client.get(url).send().await?.bytes().await?;
        // `Bytes` is cheap to clone.
        let image_reader = image::ImageReader::new(std::io::Cursor::new(image_bytes.clone()))
            .with_guessed_format()?;
        let image_format = image_reader
            .format()
            .context("Unable to determine album art image format")?;
        let on_disk_path = get_album_art_dir()?
            .join(format!("{}{}", ALBUM_ART_FILENAME_PREFIX, thumbnail_id))
            .with_extension(image_format.extensions_str()[0]);
        let image_decoding_task = tokio::task::spawn_blocking(|| image_reader.decode());
        let (in_mem_image, _) = try_join(
            image_decoding_task.map(|res| res.map_err(anyhow::Error::from)),
            tokio::fs::write(&on_disk_path, image_bytes)
                .map(|res| res.map_err(anyhow::Error::from)),
        )
        .await?;
        let thumbnail = SongThumbnail {
            in_mem_image: in_mem_image?,
            on_disk_path,
            song_thumbnail_id: thumbnail_id.clone().into_owned(),
        };
        // Cache the result.
        let mut cache = self.cache.lock().await;
        cache.push(thumbnail_id_owned, thumbnail.clone());
        Ok(thumbnail)
    }
}
