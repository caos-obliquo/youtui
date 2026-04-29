use crate::config::{ApiKey, Config};
pub use messages::*;
use rusty_ytdl::reqwest;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::VecDeque;
mod messages;

pub mod api;
pub mod api_error_handler;
pub mod player;
pub mod song_downloader;
pub mod song_thumbnail_downloader;

const DL_CALLBACK_CHUNK_SIZE: u64 = 100000; // How often song download will pause to execute code.
const MAX_RETRIES: usize = 5;
const AUDIO_QUALITY: rusty_ytdl::VideoQuality = rusty_ytdl::VideoQuality::HighestAudio;

pub type ArcServer = Arc<Server>;

const MAX_RECENT_DOWNLOADS: usize = 20;

pub struct ServerMetrics {
    pub start_time: Instant,
    pub total_downloads: usize,
    pub total_download_time_ms: u64,
    pub recent_download_times_ms: VecDeque<u64>,
    pub active_downloads: usize,
}

impl ServerMetrics {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_downloads: 0,
            total_download_time_ms: 0,
            recent_download_times_ms: VecDeque::new(),
            active_downloads: 0,
        }
    }
    
    pub fn record_download(&mut self, time_ms: u64) {
        self.total_downloads += 1;
        self.total_download_time_ms += time_ms;
        self.recent_download_times_ms.push_back(time_ms);
        if self.recent_download_times_ms.len() > MAX_RECENT_DOWNLOADS {
            self.recent_download_times_ms.pop_front();
        }
    }
    
    pub fn average_download_time_ms(&self) -> u64 {
        if self.recent_download_times_ms.is_empty() {
            return 0;
        }
        self.recent_download_times_ms.iter().sum::<u64>() / self.recent_download_times_ms.len() as u64
    }
    
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Application backend that is capable of spawning concurrent tasks in response
/// to requests. Tasks each receive a handle to respond back to the caller.
pub struct Server {
    pub api: api::Api,
    pub player: player::Player,
    pub song_downloader: song_downloader::SongDownloader,
    pub song_thumbnail_downloader: song_thumbnail_downloader::SongThumbnailDownloader,
    pub api_error_handler: api_error_handler::ApiErrorHandler,
    pub metrics: ServerMetrics,
}

impl Server {
    pub fn new(api_key: ApiKey, po_token: Option<String>, config: &Config) -> Server {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Expected reqwest client build to succeed");
        let api = api::Api::new(api_key, client.clone());
        let player = player::Player::new();
        let song_downloader =
            song_downloader::SongDownloader::new(po_token, client.clone(), config);
        let song_thumbnail_downloader =
            song_thumbnail_downloader::SongThumbnailDownloader::new(client);
        let api_error_handler = api_error_handler::ApiErrorHandler::new();
        let metrics = ServerMetrics::default();
        Server {
            api,
            player,
            song_downloader,
            api_error_handler,
            song_thumbnail_downloader,
            metrics,
        }
    }
}
