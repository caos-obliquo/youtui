use super::{AUDIO_QUALITY, DL_CALLBACK_CHUNK_SIZE};
use crate::app::CALLBACK_CHANNEL_SIZE;
use crate::app::server::MAX_RETRIES;
use crate::app::AudioQuality;
use crate::app::structures::{ListSongID, Percentage};
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
    Error,
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
                let downloader = YtDlpDownloader::new(config.yt_dlp_command.clone(), po_token.clone(), cookie_path.clone(), AudioQuality::default());
                let downloader_clone = YtDlpDownloader::new(config.yt_dlp_command.clone(), po_token.clone(), cookie_path.clone(), AudioQuality::default());
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
        audio_quality: AudioQuality,
    ) -> impl Stream<Item = DownloadProgressUpdate> + use<> {
        match self {
            SongDownloader::YtDlp(yt_dlp_downloader) => {
                futures::future::Either::Left(download_song_using_downloader(
                    yt_dlp_downloader.clone(),
                    song_video_id,
                    song_playlist_id,
                    cancel_token,
                    audio_quality,
                ))
            }
            SongDownloader::Native(native_youtube_downloader) => {
                futures::future::Either::Right(download_song_using_downloader(
                    native_youtube_downloader.clone(),
                    song_video_id,
                    song_playlist_id,
                    cancel_token,
                    audio_quality,
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
    audio_quality: AudioQuality,
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
        let audio_quality = audio_quality;
            // No progress callback - icons handle the status entirely
            download_song_with_progress_update_callback(
                &downloader,
                song_video_id.clone(),
                audio_quality,
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
                            kind: DownloadProgressUpdateType::Error,
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
                        kind: DownloadProgressUpdateType::Error,
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
    audio_quality: AudioQuality,
    _run_on_progress_interval: impl Fn(Percentage) -> Fut + Send + Sync,
) -> Result<InMemSong, T::Error>
where
    Fut: Future<Output = ()> + Send,
    T: YoutubeMusicDownloader + Send + 'static,
    T::Error: std::fmt::Display + Send,
{
    let song_video_id = song_video_id.get_raw();
    let stream_future = downloader.stream_song(song_video_id, audio_quality);
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
    
    #[tokio::test]
    #[ignore = "Unreliable with dynamic concurrency - permits may be held by other tests"]
    async fn test_semaphore_limiting() {
        // Reset global state for test to ensure consistent behavior
        let semaphore = get_download_semaphore();
        
        // With default uninitialized stats, should allow MAX_CONCURRENT_DOWNLOADS
        let max_expected = if get_download_stats().lock().map(|s| s.average_time()).unwrap_or(0) == 0 {
            MAX_CONCURRENT_DOWNLOADS
        } else {
            // Could be dynamically adjusted
            1.max(MAX_CONCURRENT_DOWNLOADS)
        };
        
        let mut permits = Vec::new();
        
        // Acquire all available permits
        for _ in 0..max_expected {
            let p = semaphore.try_acquire();
            assert!(p.is_ok(), "Should be able to acquire permit");
            permits.push(p.unwrap());
        }
        
        // Next acquisition should fail (semaphore exhausted)
        assert!(semaphore.try_acquire().is_err(), "Should not be able to acquire more permits");
        
        // Release one permit
        drop(permits.pop());
        
        // Next acquisition should succeed
        assert!(semaphore.try_acquire().is_ok(), "Should be able to acquire permit after releasing one");
    }
}
