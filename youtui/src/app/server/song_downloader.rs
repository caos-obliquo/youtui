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
use futures::{Stream, StreamExt, TryStreamExt};
use rusty_ytdl::reqwest;
use std::future::Future;
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};
use ytmapi_rs::common::{VideoID, YoutubeID};

// Minimum tick in song progress that is reported to UI - prevents frequent UI
// updates.
const MIN_SONG_PROGRESS_INTERVAL: usize = 3;

#[derive(Debug, PartialEq)]
pub struct DownloadProgressUpdate {
    pub kind: DownloadProgressUpdateType,
    pub id: ListSongID,
}

// Maximum number of concurrent yt-dlp downloads.
const MAX_CONCURRENT_DOWNLOADS: usize = 4;
static DOWNLOAD_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn get_download_semaphore() -> Arc<Semaphore> {
    DOWNLOAD_SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS)))
        .clone()
}

#[derive(Debug, PartialEq)]
pub enum DownloadProgressUpdateType {
    Started,
    Downloading(Percentage),
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
    pub fn new(po_token: Option<String>, client: reqwest::Client, config: &Config) -> Self {
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
                let downloader = YtDlpDownloader::new(config.yt_dlp_command.clone(), po_token.clone(), AudioQuality::default());
                let downloader_clone = YtDlpDownloader::new(config.yt_dlp_command.clone(), po_token.clone(), AudioQuality::default());
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
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                info!("Download cancelled before starting for song {:?}", song_playlist_id);
                return;
            }
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
            let tx = tx.clone();
            let audio_quality = audio_quality.clone();
            // No progress callback - icons handle the status entirely
            download_song_with_progress_update_callback(
                &downloader,
                song_video_id.clone(),
                0, // Disabled
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
                info!("Song downloaded");
                send_or_error(
                    &tx,
                    DownloadProgressUpdate {
                        kind: DownloadProgressUpdateType::Completed(song),
                        id: song_playlist_id,
                    },
                )
                .await;
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
    _min_song_progress_interval: usize,
    audio_quality: AudioQuality,
    run_on_progress_interval: impl Fn(Percentage) -> Fut + Send + Sync,
) -> Result<InMemSong, T::Error>
where
    Fut: Future<Output = ()> + Send,
    T: YoutubeMusicDownloader + Send + 'static,
    T::Error: std::fmt::Display + Send,
{
    let song_video_id = song_video_id.get_raw();
    let stream_future = downloader.stream_song(song_video_id, audio_quality);
    let callback = run_on_progress_interval;
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
                return Err(e.into());
            }
        }
    }
    
    let song = song_data;
    info!(
        "download_complete: song_id={}, actual_size={}, download_ms={}",
        song_video_id,
        song.len(),
        start_time.elapsed().as_millis()
    );
    Ok(InMemSong(song))
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_semaphore_limiting() {
        let semaphore = get_download_semaphore();
        let mut permits = Vec::new();
        
        // Acquire all permits
        for _ in 0..MAX_CONCURRENT_DOWNLOADS {
            let p = semaphore.try_acquire();
            assert!(p.is_ok(), "Should be able to acquire permit");
            permits.push(p.unwrap());
        }
        
        // Next acquisition should fail
        assert!(semaphore.try_acquire().is_err(), "Should not be able to acquire more than MAX_CONCURRENT_DOWNLOADS permits");
        
        // Release one permit
        drop(permits.pop());
        
        // Next acquisition should succeed
        assert!(semaphore.try_acquire().is_ok(), "Should be able to acquire permit after releasing one");
    }
}
