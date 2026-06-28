use crate::config::{ApiKey, Config};
pub use messages::*;
pub use metadata_provider::{AlbumTrack, MetadataRegistry, ValidatedMetadata};
use rusty_ytdl::reqwest;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
mod messages;
// pub mod providers; // extracted to metadata-provider crate

pub mod api;
pub mod api_error_handler;
pub mod player;
pub mod song_downloader;
pub mod song_thumbnail_downloader;
// pub mod metallum; // TODO: Metal Archives CLI integration (blocked by
// Cloudflare)

const DL_CALLBACK_CHUNK_SIZE: u64 = 100000; // How often song download will pause to execute code.
const MAX_RETRIES: usize = 5;
const AUDIO_QUALITY: rusty_ytdl::VideoQuality = rusty_ytdl::VideoQuality::HighestAudio;

pub type ArcServer = Arc<Server>;

/// Application backend that is capable of spawning concurrent tasks in response
/// to requests. Tasks each receive a handle to respond back to the caller.
pub struct Server {
    pub api: api::Api,
    pub player: player::Player,
    pub song_downloader: song_downloader::SongDownloader,
    pub song_thumbnail_downloader: song_thumbnail_downloader::SongThumbnailDownloader,
    pub api_error_handler: api_error_handler::ApiErrorHandler,
    pub http_client: ::reqwest::Client,
    pub metadata_registry: Arc<MetadataRegistry>,
}

impl Server {
    pub fn new(
        api_key: ApiKey,
        po_token: Option<String>,
        cookie_path: Option<String>,
        config: &Config,
        overrides_path: Option<PathBuf>,
        cache_path: Option<PathBuf>,
    ) -> Server {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Expected reqwest client build to succeed");
        let api = api::Api::new(api_key);
        let player = player::Player::new();
        let song_downloader =
            song_downloader::SongDownloader::new(po_token, client.clone(), cookie_path, config);
        let song_thumbnail_downloader =
            song_thumbnail_downloader::SongThumbnailDownloader::new(client);
        let api_error_handler = api_error_handler::ApiErrorHandler::new();
        let http_client = ::reqwest::Client::builder()
            .user_agent("Youtui/0.1 (music-player)")
            .build()
            .expect("Expected reqwest client build to succeed");
        let metadata_registry = Arc::new(MetadataRegistry::new(
            http_client.clone(),
            Some(config.scrobbling.api_key.clone()).filter(|s| !s.is_empty()),
            Some(config.scrobbling.discogs_token.clone()).filter(|s| !s.is_empty()),
            Some(config.scrobbling.genius_token.clone()).filter(|s| !s.is_empty()),
            overrides_path,
            cache_path,
        ));
        Server {
            api,
            player,
            song_downloader,
            api_error_handler,
            song_thumbnail_downloader,
            http_client,
            metadata_registry,
        }
    }
}
