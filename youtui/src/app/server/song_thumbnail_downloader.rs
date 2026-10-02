use crate::app::structures::{AlbumOrUploadAlbumID, ListSong, ListSongAlbum};
use crate::core::{create_or_clean_directory, get_dir_file_paths, touch_file_with_timestamp};
use crate::get_data_dir;
use anyhow::{Context, anyhow};
use async_cell::sync::AsyncCell;
use futures::FutureExt;
use futures::future::try_join;
use rusty_ytdl::reqwest;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};
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
    /// Keyed on the artwork URL. Compilation imports give every track the same
    /// cover, so keying on the per-track video id would download one image once
    /// per track and store one copy of it per track.
    Url(String),
}
impl<'a> From<&'a ListSong> for SongThumbnailID<'a> {
    fn from(song: &'a ListSong) -> SongThumbnailID<'a> {
        match song.album.as_deref() {
            // URL-added songs (YouTube URL or Bandcamp) carry an empty album id
            // (playlist.rs insert_yt_video_metadata). Keying art on that empty
            // id would collapse every URL-added song onto one cache entry, so
            // fall back to the artwork URL, and only then to the unique video
            // id (the URL itself).
            Some(ListSongAlbum {
                id: AlbumOrUploadAlbumID::Album(a),
                ..
            }) if !a.get_raw().is_empty() => SongThumbnailID::Album(a.into()),
            Some(ListSongAlbum {
                id: AlbumOrUploadAlbumID::UploadAlbum(a),
                ..
            }) if !a.get_raw().is_empty() => SongThumbnailID::UploadAlbum(a.into()),
            _ => match largest_thumbnail_url(song) {
                Some(url) => SongThumbnailID::Url(url),
                None => SongThumbnailID::Video((&song.video_id).into()),
            },
        }
    }
}

fn largest_thumbnail_url(song: &ListSong) -> Option<String> {
    song.thumbnails
        .as_ref()
        .iter()
        .max_by_key(|t| t.height * t.width)
        .map(|t| t.url.clone())
}

/// Filesystem-safe cache key for a thumbnail id. The key becomes part of the
/// on-disk filename (`YAA_{key}.{ext}`), so path separators in ids (e.g. full
/// Bandcamp URLs used as video ids) must be replaced or they would create
/// nested directories.
fn thumbnail_cache_key(id: &SongThumbnailID<'_>) -> String {
    id.to_string().replace(['/', ':'], "_")
}
impl std::fmt::Display for SongThumbnailID<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SongThumbnailID::Album(id) => write!(f, "A_{}", id.get_raw()),
            SongThumbnailID::UploadAlbum(id) => write!(f, "U_{}", id.get_raw()),
            SongThumbnailID::Video(id) => write!(f, "V_{}", id.get_raw()),
            SongThumbnailID::Url(url) => write!(f, "C_{}", url),
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
            SongThumbnailID::Url(url) => SongThumbnailID::Url(url),
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

pub struct SongThumbnailDownloader {
    client: reqwest::Client,
    // For information about why this error is stringly typed, see DynamicApiError
    status: Arc<AsyncCell<Result<(), String>>>,
}

impl SongThumbnailDownloader {
    pub fn new(client: reqwest::Client) -> Self {
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
        Self { client, status }
    }
    pub async fn download_song_thumbnail(
        &self,
        thumbnail_id: SongThumbnailID<'static>,
        thumbnail_url: String,
    ) -> anyhow::Result<SongThumbnail> {
        // Do not download album art until directory setup and clean has completed.
        self.status.get().await.map_err(|e| anyhow!(e))?;

        // Return early if thumbnail already exists in disk cache.
        if let Some(cached_song_thumbnail) = get_cached_album_art(thumbnail_id.clone()).await {
            if let Err(e) =
                touch_file_with_timestamp(&cached_song_thumbnail.on_disk_path, SystemTime::now())
                    .await
            {
                warn!(
                    "Error <{e} whilst trying to update timestamp on image {}",
                    cached_song_thumbnail.on_disk_path.display()
                )
            }
            return Ok(cached_song_thumbnail);
        }

        // Upgrade YTM thumbnail URL to request larger resolution.
        let url = if let Some(eq_pos) = thumbnail_url.rfind('=') {
            reqwest::Url::parse(&format!("{}w1920-h1200", &thumbnail_url[..=eq_pos]))?
        } else {
            reqwest::Url::parse(&thumbnail_url)?
        };
        let image_bytes = self
            .client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("album art request failed for {url}"))?
            .error_for_status()
            .with_context(|| format!("album art request returned a bad status for {url}"))?
            .bytes()
            .await
            .with_context(|| format!("album art response body unreadable for {url}"))?;
        // `Bytes` is cheap to clone.
        let image_reader = image::ImageReader::new(std::io::Cursor::new(image_bytes.clone()))
            .with_guessed_format()?;
        let image_format = image_reader
            .format()
            .context("Unable to determine album art image format")?;
        let on_disk_path = get_album_art_dir()?
            .join(format!(
                "{}{}",
                ALBUM_ART_FILENAME_PREFIX,
                thumbnail_cache_key(&thumbnail_id)
            ))
            .with_extension(image_format.extensions_str()[0]);
        let image_decoding_task = tokio::task::spawn_blocking(|| image_reader.decode());
        let (in_mem_image, _) = try_join(
            image_decoding_task.map(|res| res.map_err(anyhow::Error::from)),
            tokio::fs::write(&on_disk_path, image_bytes)
                .map(|res| res.map_err(anyhow::Error::from)),
        )
        .await?;
        Ok(SongThumbnail {
            in_mem_image: in_mem_image?,
            on_disk_path,
            song_thumbnail_id: thumbnail_id,
        })
    }
    pub async fn download_song_thumbnail_from_bytes(
        &self,
        thumbnail_id: SongThumbnailID<'static>,
        image_bytes: Vec<u8>,
    ) -> anyhow::Result<SongThumbnail> {
        // Do not download album art until directory setup and clean has completed.
        self.status.get().await.map_err(|e| anyhow!(e))?;

        // Return early if thumbnail already exists in disk cache.
        if let Some(cached_song_thumbnail) = get_cached_album_art(thumbnail_id.clone()).await {
            if let Err(e) =
                touch_file_with_timestamp(&cached_song_thumbnail.on_disk_path, SystemTime::now())
                    .await
            {
                warn!(
                    "Error <{e} whilst trying to update timestamp on image {}",
                    cached_song_thumbnail.on_disk_path.display()
                )
            }
            return Ok(cached_song_thumbnail);
        }

        let image_bytes = bytes::Bytes::from(image_bytes);
        let image_reader = image::ImageReader::new(std::io::Cursor::new(image_bytes.clone()))
            .with_guessed_format()?;
        let image_format = image_reader
            .format()
            .context("Unable to determine album art image format")?;
        let on_disk_path = get_album_art_dir()?
            .join(format!(
                "{}{}",
                ALBUM_ART_FILENAME_PREFIX,
                thumbnail_cache_key(&thumbnail_id)
            ))
            .with_extension(image_format.extensions_str()[0]);
        let image_decoding_task = tokio::task::spawn_blocking(|| image_reader.decode());
        let (in_mem_image, _) = try_join(
            image_decoding_task.map(|res| res.map_err(anyhow::Error::from)),
            tokio::fs::write(&on_disk_path, image_bytes)
                .map(|res| res.map_err(anyhow::Error::from)),
        )
        .await?;
        Ok(SongThumbnail {
            in_mem_image: in_mem_image?,
            on_disk_path,
            song_thumbnail_id: thumbnail_id,
        })
    }
}

/// Get the first matching thumbnail in the cache directory matching
/// thumbnail_id if there is one with the correct name and format.
async fn get_cached_album_art(thumbnail_id: SongThumbnailID<'_>) -> Option<SongThumbnail> {
    let album_art_dir = get_album_art_dir()
        .inspect_err(|e| {
            warn!("Error <{e}> getting list of files in album art dir, falling back to network",)
        })
        .ok()?;

    let dir_file_paths = get_dir_file_paths(&album_art_dir)
        .await
        .inspect_err(|e| {
            warn!(
                "Error <{e}> iterating through files in album art dir {}, falling back to network",
                album_art_dir.display()
            )
        })
        .ok()?
        .filter_map(|maybe_path| match maybe_path {
            Ok(path) => Some(path),
            Err(e) => {
                warn!(
                    "Error <{e}> iterating through files in album art dir {}, ignoring this entry",
                    album_art_dir.display()
                );
                None
            }
        });
    let thumbnail_id_clone = thumbnail_id.clone();
    let matching_album_art = futures::stream::StreamExt::filter_map(dir_file_paths, async |path| {
        if path
            .file_prefix()
            .and_then(|dir_file_prefix| dir_file_prefix.to_str())
            // Youtui album art is valid unicode - ie YAA_{STRING}
            // Therefore, we can ignore all invalid unicode files in this directory as they
            // are not from Youtui.
            .is_none_or(|dir_file_prefix| {
                dir_file_prefix
                    != format!(
                        "{}{}",
                        ALBUM_ART_FILENAME_PREFIX,
                        thumbnail_cache_key(&thumbnail_id_clone)
                    )
                    .as_str()
            })
        {
            // Not this song's art, and not a problem: the directory holds every
            // cached album, so a non-matching name is the normal case. This
            // used to warn, which fired once per cached file on every lookup
            // and produced 23691 identical warnings in a single session.
            return None;
        }
        // Youtui will always write a file extension.
        let Some(file_ext) = path.extension() else {
            warn!(
                "Detected a file in youtui album art directory with no extension {:?}",
                path.file_name()
            );
            return None;
        };
        // ...and it will be a valid image format extension.
        let Some(image_format) = image::ImageFormat::from_extension(file_ext) else {
            warn!(
                "Detected a file in youtui album art directory with invalid extension {:?}",
                path.file_name()
            );
            return None;
        };
        let image_bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                info!("Unable to read image {path:?}, with error <{e}> ignoring");
                return None;
            }
        };
        let image_reader =
            image::ImageReader::with_format(std::io::Cursor::new(image_bytes), image_format);
        let image_decoded = match tokio::task::spawn_blocking(|| image_reader.decode()).await {
            Ok(Ok(img)) => img,
            Ok(Err(e)) => {
                warn!(
                    "Decoding image {:?} errored with error <{e}>, ignoring",
                    path.file_name()
                );
                return None;
            }
            Err(e) => {
                error!(
                    "Decoding image {:?} panicked with error <{e}>, ignoring",
                    path.file_name()
                );
                return None;
            }
        };
        Some((image_decoded, path.as_path().to_owned()))
    });
    let mut matching_album_art = std::pin::pin!(matching_album_art);
    if let Some((in_mem_image, on_disk_path)) = matching_album_art.next().await {
        debug!(
            "Loaded thumbnail id {thumbnail_id:?} from disk path {}",
            on_disk_path.display()
        );
        return Some(SongThumbnail {
            in_mem_image,
            on_disk_path,
            song_thumbnail_id: thumbnail_id.into_owned(),
        });
    };
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::structures::{ListSongArtist, MaybeRc};
    use ytmapi_rs::common::{AlbumID, Thumbnail, VideoID};

    #[test]
    fn cache_key_sanitizes_bandcamp_url_video_ids() {
        let id = SongThumbnailID::Video(VideoID::from_raw(
            "https://domnoise.bandcamp.com/track/do-suor-do-teu-rosto",
        ));
        let key = thumbnail_cache_key(&id);
        assert!(!key.contains('/'), "cache key must not contain path separators: {key}");
        assert_eq!(key, "V_https___domnoise.bandcamp.com_track_do-suor-do-teu-rosto");
    }

    #[test]
    fn cache_key_leaves_safe_ids_unchanged() {
        let id = SongThumbnailID::Album(AlbumID::from_raw("MPREb_abc123"));
        assert_eq!(thumbnail_cache_key(&id), "A_MPREb_abc123");
    }

    #[test]
    fn empty_album_id_falls_back_to_video_id() {
        let song = ListSong {
            video_id: VideoID::from_raw("https://domnoise.bandcamp.com/track/one"),
            track_no: None,
            plays: String::new(),
            title: "One".into(),
            explicit: None,
            download_status: crate::app::structures::DownloadStatus::None,
            id: crate::app::structures::ListSongID(0),
            duration_string: "3:00".into(),
            actual_duration: None,
            start_offset: None,
            year: None,
            album_art: crate::app::structures::AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            artists: MaybeRc::Owned(vec![ListSongArtist { name: "Artist".into(), id: None }]),
            thumbnails: MaybeRc::Owned(Vec::new()),
            album: Some(MaybeRc::Owned(ListSongAlbum {
                name: "Some Album".into(),
                id: AlbumOrUploadAlbumID::Album(AlbumID::from_raw("")),
            })),
            like_status: ytmapi_rs::common::LikeStatus::Indifferent,
            is_album_upload: false,
            release_mbid: None,
            artists_string: std::sync::OnceLock::new(),
        };
        let id = SongThumbnailID::from(&song);
        match id {
            SongThumbnailID::Video(v) => {
                assert_eq!(v.get_raw(), "https://domnoise.bandcamp.com/track/one");
            }
            other => panic!("expected Video variant for empty album id, got {other:?}"),
        }
    }

    #[test]
    fn real_album_id_stays_album_variant() {
        let song = ListSong {
            video_id: VideoID::from_raw("dQw4w9WgXcQ"),
            track_no: None,
            plays: String::new(),
            title: "Song".into(),
            explicit: None,
            download_status: crate::app::structures::DownloadStatus::None,
            id: crate::app::structures::ListSongID(0),
            duration_string: "3:00".into(),
            actual_duration: None,
            start_offset: None,
            year: None,
            album_art: crate::app::structures::AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            artists: MaybeRc::Owned(vec![ListSongArtist { name: "Artist".into(), id: None }]),
            thumbnails: MaybeRc::Owned(Vec::new()),
            album: Some(MaybeRc::Owned(ListSongAlbum {
                name: "Some Album".into(),
                id: AlbumOrUploadAlbumID::Album(AlbumID::from_raw("MPREb_abc123")),
            })),
            like_status: ytmapi_rs::common::LikeStatus::Indifferent,
            is_album_upload: false,
            release_mbid: None,
            artists_string: std::sync::OnceLock::new(),
        };
        let id = SongThumbnailID::from(&song);
        match id {
            SongThumbnailID::Album(a) => assert_eq!(a.get_raw(), "MPREb_abc123"),
            other => panic!("expected Album variant for real album id, got {other:?}"),
        }
    }

    fn bc_song(track_url: &str, covers: &[(&str, u64)]) -> ListSong {
        ListSong {
            video_id: VideoID::from_raw(track_url.to_string()),
            track_no: None,
            plays: String::new(),
            title: "Track".into(),
            explicit: None,
            download_status: crate::app::structures::DownloadStatus::None,
            id: crate::app::structures::ListSongID(0),
            duration_string: "3:00".into(),
            actual_duration: None,
            start_offset: None,
            year: None,
            album_art: crate::app::structures::AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            artists: MaybeRc::Owned(vec![ListSongArtist { name: "Artist".into(), id: None }]),
            thumbnails: MaybeRc::Owned(
                covers
                    .iter()
                    .map(|(url, size)| Thumbnail {
                        width: *size,
                        height: *size,
                        url: (*url).to_string(),
                    })
                    .collect(),
            ),
            album: Some(MaybeRc::Owned(ListSongAlbum {
                name: "Compilation".into(),
                id: AlbumOrUploadAlbumID::Album(AlbumID::from_raw("")),
            })),
            like_status: ytmapi_rs::common::LikeStatus::Indifferent,
            is_album_upload: false,
            release_mbid: None,
            artists_string: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn compilation_tracks_sharing_a_cover_collapse_to_one_cache_entry() {
        let one = bc_song(
            "https://dramarecorder.bandcamp.com/track/neutralize",
            &[("https://f4.bcbits.com/img/a0489092809_5.jpg", 1200)],
        );
        let two = bc_song(
            "https://dramarecorder.bandcamp.com/track/temper-wrecked",
            &[("https://f4.bcbits.com/img/a0489092809_5.jpg", 1200)],
        );
        let a = SongThumbnailID::from(&one).into_owned();
        let b = SongThumbnailID::from(&two).into_owned();
        assert_eq!(a, b, "same cover must not download once per track");
        assert!(matches!(a, SongThumbnailID::Url(_)), "expected Url variant, got {a:?}");
    }

    #[test]
    fn differing_covers_do_not_collapse() {
        let one = bc_song("https://x.bandcamp.com/track/a", &[("https://f4.bcbits.com/img/a1_5.jpg", 1200)]);
        let two = bc_song("https://x.bandcamp.com/track/b", &[("https://f4.bcbits.com/img/a2_5.jpg", 1200)]);
        assert_ne!(
            SongThumbnailID::from(&one).into_owned(),
            SongThumbnailID::from(&two).into_owned()
        );
    }

    #[test]
    fn largest_thumbnail_is_the_one_keyed_on() {
        let song = bc_song(
            "https://x.bandcamp.com/track/a",
            &[
                ("https://f4.bcbits.com/img/small_5.jpg", 60),
                ("https://f4.bcbits.com/img/big_10.jpg", 1200),
            ],
        );
        match SongThumbnailID::from(&song) {
            SongThumbnailID::Url(url) => assert_eq!(url, "https://f4.bcbits.com/img/big_10.jpg"),
            other => panic!("expected Url variant, got {other:?}"),
        }
    }

    #[test]
    fn real_album_id_outranks_the_cover_url() {
        let mut song = bc_song("dQw4w9WgXcQ", &[("https://f4.bcbits.com/img/a1_5.jpg", 1200)]);
        song.album = Some(MaybeRc::Owned(ListSongAlbum {
            name: "Some Album".into(),
            id: AlbumOrUploadAlbumID::Album(AlbumID::from_raw("MPREb_abc123")),
        }));
        match SongThumbnailID::from(&song) {
            SongThumbnailID::Album(a) => assert_eq!(a.get_raw(), "MPREb_abc123"),
            other => panic!("expected Album to win over cover url, got {other:?}"),
        }
    }

    #[test]
    fn url_variant_cache_key_has_no_path_separators() {
        let song = bc_song("https://x.bandcamp.com/track/a", &[("https://f4.bcbits.com/img/a1_5.jpg", 1200)]);
        let key = thumbnail_cache_key(&SongThumbnailID::from(&song));
        assert!(!key.contains('/') && !key.contains(':'), "cache key must be filesystem safe: {key}");
        assert_eq!(key, "C_https___f4.bcbits.com_img_a1_5.jpg");
    }

    #[test]
    fn no_cover_and_empty_album_id_still_falls_back_to_video_id() {
        let song = bc_song("https://x.bandcamp.com/track/a", &[]);
        match SongThumbnailID::from(&song) {
            SongThumbnailID::Video(v) => assert_eq!(v.get_raw(), "https://x.bandcamp.com/track/a"),
            other => panic!("expected Video fallback, got {other:?}"),
        }
    }
}
