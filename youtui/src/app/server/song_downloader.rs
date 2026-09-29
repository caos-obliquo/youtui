use super::{AUDIO_QUALITY, DL_CALLBACK_CHUNK_SIZE};
use crate::app::CALLBACK_CHANNEL_SIZE;
use crate::app::server::MAX_RETRIES;
use crate::app::structures::{AudioQuality, ListSongID, Percentage};
use crate::config::{Config, DownloaderType};
use crate::core::send_or_error;
use crate::youtube_downloader::native::NativeYoutubeDownloader;
use crate::youtube_downloader::yt_dlp::YtDlpDownloader;
use crate::youtube_downloader::{YoutubeMusicDownload, YoutubeMusicDownloader};
use async_callback_manager::PanickingReceiverStream;
use futures::{Stream, StreamExt};
use rusty_ytdl::reqwest;
use std::future::Future;
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};
use ytmapi_rs::common::{VideoID, YoutubeID};

#[derive(Debug, PartialEq)]
pub struct DownloadProgressUpdate {
    pub kind: DownloadProgressUpdateType,
    pub id: ListSongID,
}

// Maximum number of concurrent yt-dlp downloads.
const MAX_CONCURRENT_DOWNLOADS: usize = 4;
static DOWNLOAD_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Tracks download performance for dynamic concurrency adjustment.
static DOWNLOAD_STATS: OnceLock<std::sync::Mutex<DownloadStats>> = OnceLock::new();

#[derive(Default)]
struct DownloadStats {
    total_downloads: usize,
    total_time_ms: u64,
    recent_times_ms: std::collections::VecDeque<u64>,
}

impl DownloadStats {
    fn record_download(&mut self, time_ms: u64) {
        self.total_downloads += 1;
        self.total_time_ms += time_ms;
        self.recent_times_ms.push_back(time_ms);
        if self.recent_times_ms.len() > 10 {
            self.recent_times_ms.pop_front();
        }
    }

    fn average_time(&self) -> u64 {
        if self.recent_times_ms.is_empty() {
            return 6000; // Default 6s estimate
        }
        self.recent_times_ms.iter().sum::<u64>() / self.recent_times_ms.len() as u64
    }
}

fn get_download_stats() -> &'static std::sync::Mutex<DownloadStats> {
    DOWNLOAD_STATS
        .get_or_init(|| std::sync::Mutex::new(DownloadStats::default()))
}

fn get_download_semaphore() -> Arc<Semaphore> {
    let avg_time = get_download_stats()
        .lock()
        .map(|s| s.average_time())
        .unwrap_or(0);
    
    // Dynamic concurrency: faster downloads = more concurrent, slower = less concurrent
    // When stats are empty (0), use default for initial warm-up period
    let target_permits = if avg_time == 0 || avg_time < 4000 {
        // Very fast downloads or uninitialized - can handle more concurrent
        MAX_CONCURRENT_DOWNLOADS
    } else if avg_time < 7000 {
        // Normal downloads - use slightly reduced concurrency
        3
    } else {
        // Slow downloads - reduce concurrency to avoid network saturation
        1
    };
    
    DOWNLOAD_SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(target_permits)))
        .clone()
}

#[derive(Debug, PartialEq)]
pub enum DownloadProgressUpdateType {
    Started,
    Completed(InMemSong),
    Error(String),
    Retrying { times_retried: usize },
}

/// Representation of a song in memory - an array of bytes.
/// Newtype pattern is used to provide a cleaner Debug display.
#[derive(PartialEq)]
pub struct InMemSong(pub Vec<u8>);
// Custom derive - otherwise will be displaying 3MB array of bytes...
impl std::fmt::Debug for InMemSong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("InMemSong").field(&"Vec<..>").finish()
    }
}

pub enum SongDownloader {
    YtDlp(YtDlpDownloader),
    Native(NativeYoutubeDownloader),
}

impl SongDownloader {
    pub fn new(po_token: Option<String>, client: reqwest::Client, cookie_path: Option<String>, config: &Config) -> Self {
        match config.downloader_type {
            DownloaderType::Native => {
                info!(
                    "Initiating native downloader. Has po_token: {}",
                    po_token.is_some()
                );
                SongDownloader::Native(NativeYoutubeDownloader::new(
                    DL_CALLBACK_CHUNK_SIZE,
                    AUDIO_QUALITY,
                    po_token,
                    client,
                ))
            }
            DownloaderType::YtDlp => {
                info!(
                    "Initiating yt-dlp downloader using yt-dlp path `{}`",
                    config.yt_dlp_command
                );
                let downloader = YtDlpDownloader::new(config.yt_dlp_command.clone(), po_token.clone(), cookie_path.clone(), config.cookie_browser.clone());
                let downloader_clone = YtDlpDownloader::new(config.yt_dlp_command.clone(), po_token.clone(), cookie_path.clone(), config.cookie_browser.clone());
                tokio::task::spawn(async {
                    let output = downloader_clone.get_version().await;
                    match output {
                        Ok(output) => {
                            info!("yt-dlp version is: {:?}", output.trim_end());
                        }
                        Err(e) => error!("Unable to determine yt-dlp version, error: <{e}>"),
                    }
                });
                SongDownloader::YtDlp(downloader)
            }
        }
    }
    pub fn download_song(
        &self,
        song_video_id: VideoID<'static>,
        song_playlist_id: ListSongID,
        cancel_token: Option<Arc<tokio_util::sync::CancellationToken>>,
        quality: AudioQuality,
    ) -> impl Stream<Item = DownloadProgressUpdate> + use<> {
        match self {
            SongDownloader::YtDlp(yt_dlp_downloader) => {
                futures::future::Either::Left(download_song_using_downloader(
                    yt_dlp_downloader.clone(),
                    song_video_id,
                    song_playlist_id,
                    cancel_token,
                    quality,
                ))
            }
            SongDownloader::Native(native_youtube_downloader) => {
                futures::future::Either::Right(download_song_using_downloader(
                    native_youtube_downloader.clone(),
                    song_video_id,
                    song_playlist_id,
                    cancel_token,
                    quality,
                ))
            }
        }
    }
}

fn download_song_using_downloader<T>(
    downloader: T,
    song_video_id: VideoID<'static>,
    song_playlist_id: ListSongID,
    cancel_token: Option<Arc<tokio_util::sync::CancellationToken>>,
    quality: AudioQuality,
) -> impl Stream<Item = DownloadProgressUpdate>
where
    T: YoutubeMusicDownloader + Send + Sync + 'static,
    T::Error: std::fmt::Display + Send,
{
    let (tx, rx) = tokio::sync::mpsc::channel(CALLBACK_CHANNEL_SIZE);
    let handle = tokio::spawn(async move {
        let semaphore = get_download_semaphore();
        let _permit = match semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                error!("Download semaphore closed");
                return;
            }
        };
        // Check if already cancelled before starting
        if let Some(ref token) = cancel_token
            && token.is_cancelled() {
            info!("Download cancelled before starting for song {:?}", song_playlist_id);
            return;
        }
        
        info!("Running download");
        send_or_error(
            &tx.clone(),
            DownloadProgressUpdate {
                kind: DownloadProgressUpdateType::Started,
                id: song_playlist_id,
            },
        )
        .await;
        
        let song_download = || {
            let _tx = tx.clone();
            // No progress callback - icons handle the status entirely
            download_song_with_progress_update_callback(
                &downloader,
                song_video_id.clone(),
                quality,
                move |_| async move { /* No-op - status shown via icons */ },
            )
        };
        let song = run_future_with_retries_and_retry_callback(
            song_download,
            |times_retried| {
                let tx = tx.clone();
                warn!("Retrying - {} tries left", MAX_RETRIES - times_retried);
                send_or_error(
                    tx,
                    DownloadProgressUpdate {
                        kind: DownloadProgressUpdateType::Retrying { times_retried },
                        id: song_playlist_id,
                    },
                )
            },
            MAX_RETRIES,
        )
        .await;

        match song {
            Some(song) => {
                if song.0.is_empty() {
                    warn!("Download produced 0 bytes, marking as failed");
                    send_or_error(
                        &tx,
                        DownloadProgressUpdate {
                            kind: DownloadProgressUpdateType::Error("Download produced 0 bytes".to_string()),
                            id: song_playlist_id,
                        },
                    )
                    .await;
                } else {
                    info!("Song downloaded ({} bytes)", song.0.len());
                    send_or_error(
                        &tx,
                        DownloadProgressUpdate {
                            kind: DownloadProgressUpdateType::Completed(song),
                            id: song_playlist_id,
                        },
                    )
                    .await;
                }
            }
            None => {
                error!("Max retries exceeded");
                send_or_error(
                    &tx,
                    DownloadProgressUpdate {
                        kind: DownloadProgressUpdateType::Error("Max retries exceeded".to_string()),
                        id: song_playlist_id,
                    },
                )
                .await;
            }
        };
    });
    PanickingReceiverStream::new(rx, handle)
}

/// Parameter for run_on_retry callback is "times retried"
async fn run_future_with_retries_and_retry_callback<Fut1, Fut2, T, E>(
    future_generator: impl Fn() -> Fut1 + Send,
    run_on_retry: impl Fn(usize) -> Fut2 + Send,
    max_retries: usize,
) -> Option<T>
where
    Fut1: Future<Output = Result<T, E>> + Send,
    Fut2: Future<Output = ()> + Send,
    E: Send,
    T: Send,
{
    let mut retries = 0;
    while retries <= max_retries {
        let output = future_generator().await;
        if let Ok(output) = output {
            return Some(output);
        }
        retries += 1;
        if retries <= max_retries {
            run_on_retry(retries).await;
        }
    }
    None
}

async fn download_song_with_progress_update_callback<T, Fut>(
    downloader: &T,
    song_video_id: VideoID<'static>,
    quality: AudioQuality,
    _run_on_progress_interval: impl Fn(Percentage) -> Fut + Send + Sync,
) -> Result<InMemSong, T::Error>
where
    Fut: Future<Output = ()> + Send,
    T: YoutubeMusicDownloader + Send + 'static,
    T::Error: std::fmt::Display + Send,
{
    let song_video_id = song_video_id.get_raw();
    let stream_future = downloader.stream_song(song_video_id, quality);
    let YoutubeMusicDownload {
        total_size_bytes,
        song: stream,
    } = match stream_future.await {
        Err(e) => {
            error!("Error received finding song: <{e}>");
            return Err(e);
        }
        Ok(x) => x,
    };
    info!("Commencing streaming song {song_video_id}, expected size bytes: {total_size_bytes}");
    // No progress reporting - UI uses icons only (↓ downloading, ✓ downloaded)
    // Just stream the audio data directly without callback overhead
    let start_time = std::time::Instant::now();
    
    // Collect all chunks
    let mut song_data = Vec::new();
    let mut stream = Box::pin(stream);
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => song_data.extend_from_slice(&chunk),
            Err(e) => {
                error!("Error receiving song data: <{e}>");
                return Err(e);
            }
        }
    }
    
    let song = song_data;
    let download_time = start_time.elapsed().as_millis();
    info!(
        "download_complete: song_id={}, actual_size={}, download_ms={}",
        song_video_id,
        song.len(),
        download_time
    );
    
    // Record download statistics for dynamic concurrency adjustment
    if let Ok(mut stats) = get_download_stats().lock() {
        stats.record_download(download_time as u64);
    }
    
    Ok(InMemSong(song))
}


#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone)]
    struct MockError(String);

    impl std::fmt::Display for MockError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "mock error: {}", self.0)
        }
    }

    #[derive(Clone)]
    struct MockDownloader {
        fail_setup: bool,
        chunks: Vec<Result<Bytes, MockError>>,
    }

    impl MockDownloader {
        fn success(data: &[&[u8]]) -> Self {
            Self {
                fail_setup: false,
                chunks: data
                    .iter()
                    .map(|c| Ok(Bytes::copy_from_slice(c)))
                    .collect(),
            }
        }

        fn setup_failure() -> Self {
            Self {
                fail_setup: true,
                chunks: Vec::new(),
            }
        }
    }

    impl YoutubeMusicDownloader for MockDownloader {
        type Error = MockError;

        async fn stream_song(
            &self,
            _song_video_id: impl AsRef<str> + Send,
            _quality: AudioQuality,
        ) -> Result<
            YoutubeMusicDownload<impl Stream<Item = Result<Bytes, Self::Error>> + Send>,
            Self::Error,
        > {
            if self.fail_setup {
                return Err(MockError("setup failed".to_string()));
            }
            let total_size_bytes: usize = self
                .chunks
                .iter()
                .filter_map(|c| c.as_ref().ok().map(|b| b.len()))
                .sum();
            let stream = futures::stream::iter(self.chunks.clone());
            Ok(YoutubeMusicDownload {
                total_size_bytes,
                song: stream,
            })
        }
    }

    fn test_video_id() -> VideoID<'static> {
        VideoID::from_raw("test123")
    }

    #[tokio::test]
    async fn test_retry_success_first_try() {
        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_clone = callback_count.clone();
        let result = run_future_with_retries_and_retry_callback(
            || async { Ok::<_, String>("ok") },
            move |_| {
                let counter = callback_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            },
            3,
        )
        .await;
        assert_eq!(result, Some("ok"));
        assert_eq!(callback_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts.clone();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_clone = callback_count.clone();
        let result = run_future_with_retries_and_retry_callback(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    let n = attempts.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        Err::<String, String>("fail".to_string())
                    } else {
                        Ok("recovered".to_string())
                    }
                }
            },
            move |_| {
                let counter = callback_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            },
            3,
        )
        .await;
        assert_eq!(result, Some("recovered".to_string()));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(callback_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_always_fails_returns_none() {
        let max_retries = 3;
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts.clone();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_clone = callback_count.clone();
        let result = run_future_with_retries_and_retry_callback(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err::<String, String>("always fails".to_string())
                }
            },
            move |_| {
                let counter = callback_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            },
            max_retries,
        )
        .await;
        assert_eq!(result, None);
        // One initial attempt + max_retries retries.
        assert_eq!(attempts.load(Ordering::SeqCst), max_retries + 1);
        assert_eq!(callback_count.load(Ordering::SeqCst), max_retries);
    }

    #[tokio::test]
    async fn test_retry_callback_receives_retry_number() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_clone = seen.clone();
        let result = run_future_with_retries_and_retry_callback(
            || async { Err::<String, String>("fail".to_string()) },
            move |times_retried| {
                let seen = seen_clone.clone();
                async move {
                    seen.lock().unwrap().push(times_retried);
                }
            },
            3,
        )
        .await;
        assert_eq!(result, None);
        assert_eq!(*seen.lock().unwrap(), vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_retry_zero_max_retries_no_callback() {
        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_clone = callback_count.clone();
        let result = run_future_with_retries_and_retry_callback(
            || async { Err::<String, String>("fail".to_string()) },
            move |_| {
                let counter = callback_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            },
            0,
        )
        .await;
        assert_eq!(result, None);
        assert_eq!(callback_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_download_success_collects_chunks() {
        let downloader = MockDownloader::success(&[b"hello ", b"world"]);
        let result = download_song_with_progress_update_callback(
            &downloader,
            test_video_id(),
            AudioQuality::Best,
            |_| async {},
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, b"hello world".to_vec());
    }

    #[tokio::test]
    async fn test_download_setup_error_returns_err() {
        let downloader = MockDownloader::setup_failure();
        let result = download_song_with_progress_update_callback(
            &downloader,
            test_video_id(),
            AudioQuality::Best,
            |_| async {},
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_chunk_error_returns_err() {
        let downloader = MockDownloader {
            fail_setup: false,
            chunks: vec![
                Ok(Bytes::from_static(b"partial")),
                Err(MockError("chunk failed".to_string())),
            ],
        };
        let result = download_song_with_progress_update_callback(
            &downloader,
            test_video_id(),
            AudioQuality::Best,
            |_| async {},
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_empty_stream_returns_ok_empty() {
        // The 0-byte rejection lives in download_song_using_downloader;
        // this function passes empty data through as Ok.
        let downloader = MockDownloader::success(&[]);
        let result = download_song_with_progress_update_callback(
            &downloader,
            test_video_id(),
            AudioQuality::Best,
            |_| async {},
        )
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().0.is_empty());
    }

    #[tokio::test]
    async fn test_in_mem_song_debug_hides_bytes() {
        let song = InMemSong(vec![1, 2, 3]);
        assert_eq!(format!("{song:?}"), "InMemSong(\"Vec<..>\")");
    }
    
#[tokio::test]
async fn test_semaphore_limiting() {
    let semaphore = get_download_semaphore();
    let stats = get_download_stats().lock().unwrap();
    let avg_time = stats.average_time();
    drop(stats);
    
    let target_permits = if avg_time == 0 || avg_time < 4000 {
        MAX_CONCURRENT_DOWNLOADS
    } else if avg_time < 7000 {
        3
    } else {
        1
    };
    
    let mut permits = Vec::new();
    
    for _ in 0..target_permits {
        let p = semaphore.try_acquire();
        assert!(p.is_ok(), "Should be able to acquire permit");
        permits.push(p.unwrap());
    }
    
    assert!(semaphore.try_acquire().is_err(), "Should not be able to acquire more permits");
    
    drop(permits.pop());
    
    assert!(semaphore.try_acquire().is_ok(), "Should be able to acquire permit after releasing one");
}
}
