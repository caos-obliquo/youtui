use crate::app::AudioQuality;
use crate::youtube_downloader::{YoutubeMusicDownload, YoutubeMusicDownloader};
use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use std::ffi::OsString;
use std::ops::Deref;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tracing::{debug, error, info, warn};

const ESTIMATED_AUDIO_SIZE_BYTES: usize = 4 * 1024 * 1024; // 4MB estimate

#[derive(Clone)]
#[allow(dead_code)]
/// # Note
/// Cheap to clone due to use of Arc to store internals.
pub struct YtDlpDownloader {
    yt_dlp_command: Arc<OsString>,
    po_token: Option<String>,
    audio_quality: AudioQuality,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum YtDlpDownloaderError {
    IoError { message: String },
    NoOutput,
    InvalidFilesizeOutput { output: String },
    FormatNotAvailable { video_id: String },
    AuthenticationError { video_id: String, message: String },
}

impl std::fmt::Display for YtDlpDownloaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YtDlpDownloaderError::IoError { message } => {
                write!(f, "Error running yt-dlp - <{message}>")
            }
            YtDlpDownloaderError::NoOutput => {
                write!(
                    f,
                    "Error running yt-dlp - no output when output was expected"
                )
            }
            YtDlpDownloaderError::FormatNotAvailable { video_id } => {
                write!(f, "Error running yt-dlp - format not available for video {}", video_id)
            },
            YtDlpDownloaderError::AuthenticationError { video_id, message } => {
                write!(f, "Error running yt-dlp - authentication failed for video {}: {}", video_id, message)
            },
            YtDlpDownloaderError::InvalidFilesizeOutput { output } => {
                write!(f, "Error parsing filesize output: {}", output)
            }
        }
    }
}

impl YtDlpDownloader {
    pub fn new(yt_dlp_command: String, po_token: Option<String>, audio_quality: AudioQuality) -> Self {
        Self {
            yt_dlp_command: Arc::new(yt_dlp_command.into()),
            po_token,
            audio_quality,
        }
    }
    pub async fn get_version(self) -> Result<String, YtDlpDownloaderError> {
        let output = tokio::process::Command::new(self.yt_dlp_command.deref())
            .arg("--version")
            .output()
            .await
            .map_err(|e| YtDlpDownloaderError::IoError {
                message: format!("{e}"),
            })?;
        String::from_utf8(output.stdout).map_err(|e| YtDlpDownloaderError::InvalidFilesizeOutput {
            output: e.to_string(),
        })
    }
}

impl YoutubeMusicDownloader for YtDlpDownloader {
    type Error = YtDlpDownloaderError;

    async fn stream_song(
        &self,
        song_video_id: impl AsRef<str> + Send,
        audio_quality: AudioQuality,
    ) -> Result<
        YoutubeMusicDownload<impl Stream<Item = Result<Bytes, Self::Error>> + Send>,
        Self::Error,
    > {
        let command = self.yt_dlp_command.clone();
        async move {
            // Skip extractor args - they add API overhead without benefit
            let video_id = song_video_id.as_ref().to_string();
            info!(%video_id, "Starting yt-dlp stream_song");
            
            // Fixed size for progress tracking (not accurate but sufficient)
            let total_size_bytes = ESTIMATED_AUDIO_SIZE_BYTES;
            let stream_start = Instant::now();
            info!(%video_id, %total_size_bytes, "Using estimated size for progress");
            
            // Use format based on audio quality setting
            // All use m4a for rodio compatibility
            // Different quality levels - let yt-dlp pick within range
            let format_string = match audio_quality {
                AudioQuality::Best => "bestaudio[ext=m4a]/bestaudio/best".to_string(),
                AudioQuality::High => "bestaudio[ext=m4a]/bestaudio/best".to_string(),
                AudioQuality::Medium => "bestaudio[ext=m4a]/bestaudio/best".to_string(),
                AudioQuality::Low => "bestaudio[ext=m4a]/bestaudio".to_string(),  // Smaller m4a, still compatible
            };
            
            info!("Using format {} for quality {:?}", format_string, audio_quality);
            
            let format_ref = format_string.as_str();
            let stream_args = vec![
                "--no-simulate",
                "-q",
                "--no-warnings",
                "-f",
                format_ref,
                "-o",
                "-",
                song_video_id.as_ref(),
            ];
            
            debug!(%video_id, ?stream_args, "yt-dlp stream args");
            
            let proc = tokio::process::Command::new(command.deref())
                .args(&stream_args)
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    error!(%video_id, error = %e, "Failed to spawn yt-dlp stream process");
                    YtDlpDownloaderError::IoError {
                        message: format!("{e}"),
                    }
                })?;
            
            let Child {
                stderr: Some(stderr),
                stdout: Some(stdout),
                ..
            } = proc
            else {
                error!(%video_id, "yt-dlp stream process missing stdout or stderr");
                return Err(YtDlpDownloaderError::NoOutput);
            };
            
            // Read stderr in background for error reporting
            let video_id_clone = video_id.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.contains("ERROR") || line.contains("WARNING") {
                        warn!(video_id = %video_id_clone, %line, "yt-dlp stderr");
                    }
                }
            });
            
            let video_id_for_stream = video_id.clone();
            let stream = tokio_util::io::ReaderStream::new(stdout).map_err(move |e| {
                error!(%video_id_for_stream, error = %e, "Stream error");
                YtDlpDownloaderError::IoError {
                    message: format!("{e}"),
                }
            });
            
            info!(%video_id, %total_size_bytes, stream_start_ms = %stream_start.elapsed().as_millis(), "yt-dlp stream started");
            Ok(YoutubeMusicDownload {
                total_size_bytes,
                song: stream,
            })
        }
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::youtube_downloader::yt_dlp::YtDlpDownloader;
    use crate::youtube_downloader::{YoutubeMusicDownload, YoutubeMusicDownloader};
use crate::app::AudioQuality;
    use bytes::Bytes;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_yt_dlp_downloader_with_po_token() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), Some("test_po_token".to_string()), AudioQuality::default());
        assert!(downloader.po_token.is_some());
        assert_eq!(downloader.po_token.unwrap(), "test_po_token");
    }

    #[tokio::test]
    async fn test_yt_dlp_downloader_without_po_token() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), None, AudioQuality::default());
        assert!(downloader.po_token.is_none());
    }

    #[tokio::test]
    #[ignore = "Network and yt-dlp required"]
    async fn test_downloading_a_song_with_ytdlp() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), None, AudioQuality::default());
        let YoutubeMusicDownload { song: stream, .. } =
            downloader.stream_song("lYBUbBu4W08", AudioQuality::default()).await.unwrap();
        stream
            .map(|item| item.unwrap())
            .collect::<Vec<Bytes>>()
            .await;
    }
}
