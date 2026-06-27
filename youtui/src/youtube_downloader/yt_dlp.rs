use crate::app::AudioQuality;
use crate::youtube_downloader::{YoutubeMusicDownload, YoutubeMusicDownloader};
use bytes::Bytes;
use futures::Stream;
use std::ffi::OsString;
use std::ops::Deref;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

#[derive(Clone)]
#[allow(dead_code)] // fields po_token, audio_quality passed at construction, read by yt-dlp subprocess
/// # Note
/// Cheap to clone due to use of Arc to store internals.
pub struct YtDlpDownloader {
    yt_dlp_command: Arc<OsString>,
    po_token: Option<String>,
    cookie_path: Option<String>,
    cookie_browser: String,
    audio_quality: AudioQuality,
}

#[derive(Debug)]
#[allow(dead_code)] // variants NoOutput/FormatNotAvailable/AuthenticationError: yt-dlp error patterns kept for future handling
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
    pub fn new(yt_dlp_command: String, po_token: Option<String>, cookie_path: Option<String>, cookie_browser: String, audio_quality: AudioQuality) -> Self {
        Self {
            yt_dlp_command: Arc::new(yt_dlp_command.into()),
            po_token,
            cookie_path,
            cookie_browser,
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
        _audio_quality: AudioQuality,
    ) -> Result<
        YoutubeMusicDownload<impl Stream<Item = Result<Bytes, Self::Error>> + Send>,
        Self::Error,
    > {
        let command = self.yt_dlp_command.clone();
        async move {
            let video_id = song_video_id.as_ref().to_string();
            info!(%video_id, "Starting yt-dlp download");
            
            // Write to temp file so yt-dlp applies FixupM4a/container post-processing.
            // Stdout pipe (-o -) skips post-processing, producing corrupted data on yt-dlp 2026+.
            // Use .m4a suffix so yt-dlp matches the format container.
            // --force-overwrites needed: yt-dlp's resume feature treats pre-existing 0-byte
            // files as "already complete" and writes nothing.
            let tmpfile = tempfile::Builder::new()
                .suffix(".m4a")
                .tempfile()
                .map_err(|e| {
                    YtDlpDownloaderError::IoError {
                        message: format!("Failed to create temp file: {e}"),
                    }
                })?;
            let output_path = tmpfile.path().to_owned();
            
            let format_string = "bestaudio[ext=m4a][abr>=256]/bestaudio[ext=m4a]/bestaudio/best".to_string();
            
            // web_creator extractor needs cookies - only use it when configured
            // Default extractor works without auth for most videos
            let use_web_creator = self.cookie_path.is_some();
            
            let mut stream_args = vec![
                "--no-simulate",
                "--force-overwrites",
                "-q",
                "--no-warnings",
                "-f",
                format_string.as_str(),
                "-o",
                output_path.to_str().unwrap(),
            ];
            if use_web_creator {
                stream_args.push("--extractor-args");
                stream_args.push("youtube:player_client=web_creator");
                stream_args.push("--cookies-from-browser");
                stream_args.push(&self.cookie_browser);
            }
            stream_args.push(song_video_id.as_ref());
            
            debug!(%video_id, ?stream_args, "yt-dlp args");
            
            let mut proc = tokio::process::Command::new(command.deref())
                .args(&stream_args)
                .stderr(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()
                .map_err(|e| {
                    error!(%video_id, error = %e, "Failed to spawn yt-dlp process");
                    YtDlpDownloaderError::IoError {
                        message: format!("{e}"),
                    }
                })?;
            
            // Take stderr before spawn to avoid partial move
            let stderr = proc.stderr.take().unwrap();
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
            
            // Wait for yt-dlp to complete (applies FixupM4a, decryption, etc.)
            // 5-minute timeout prevents hung processes from blocking downloads
            let status = timeout(Duration::from_secs(300), proc.wait()).await
                .map_err(|_| {
                    error!(%video_id, "yt-dlp download timed out after 300s");
                    YtDlpDownloaderError::IoError {
                        message: "yt-dlp download timed out".to_string(),
                    }
                })?  // timeout -> Result<ExitStatus, IoError>
                .map_err(|e| {
                    error!(%video_id, error = %e, "Failed to wait for yt-dlp");
                    YtDlpDownloaderError::IoError {
                        message: format!("{e}"),
                    }
                })?;  // wait -> ExitStatus
            
            if !status.success() {
                error!(%video_id, exit_code = %status, "yt-dlp failed");
                return Err(YtDlpDownloaderError::IoError {
                    message: format!("yt-dlp exited with {status}"),
                });
            }
            
            // Read completed file into memory
            let file_bytes = tokio::fs::read(&output_path).await.map_err(|e| {
                error!(%video_id, error = %e, "Failed to read yt-dlp output");
                YtDlpDownloaderError::IoError {
                    message: format!("Failed to read output file: {e}"),
                }
            })?;
            
            // Temp file cleaned up on drop
            drop(tmpfile);
            
            let total_size_bytes = file_bytes.len();
            
            // Detect and log container format
            let format_name = if file_bytes.len() >= 12 && file_bytes[4..8] == *b"ftyp" {
                let brand = &file_bytes[8..12];
                if brand == b"isom" { "MP4 (isom)" }
                else if brand == b"M4A " { "M4A" }
                else { "MP4" }
            } else if file_bytes.starts_with(b"\x1a\x45\xdf\xa3") { "WebM" }
            else if file_bytes.starts_with(b"RIFF") { "WAV" }
            else if file_bytes.starts_with(b"OggS") { "Ogg" }
            else { "unknown" };
            
            // Validate the file has a recognizable audio container header
            // Guards against corrupted output (pipe bug), empty files (resume bug),
            // and unexpected format changes from yt-dlp updates
            let is_valid = total_size_bytes > 100
                && format_name != "unknown";
            
            if !is_valid {
                let info = if file_bytes.is_empty() {
                    "empty file".to_string()
                } else if total_size_bytes < 100 {
                    format!("too small ({} bytes)", total_size_bytes)
                } else {
                    format!("invalid header: {:02x?}", &file_bytes[..16.min(total_size_bytes)])
                };
                error!(%video_id, %info, "yt-dlp download validation failed");
                return Err(YtDlpDownloaderError::IoError {
                    message: format!("Downloaded data has no valid audio container ({info})"),
                });
            }
            
            info!(%video_id, %total_size_bytes, %format_name, "yt-dlp download completed");
            
            // Return as one-shot stream (consumer already collects all chunks)
            let song = futures::stream::once(async move { Ok(Bytes::from(file_bytes)) });
            
            Ok(YoutubeMusicDownload {
                total_size_bytes,
                song,
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
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), Some("test_po_token".to_string()), None, "chromium".to_string(), AudioQuality::default());
        assert!(downloader.po_token.is_some());
        assert_eq!(downloader.po_token.unwrap(), "test_po_token");
    }

    #[tokio::test]
    async fn test_yt_dlp_downloader_without_po_token() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), None, None, "chromium".to_string(), AudioQuality::default());
        assert!(downloader.po_token.is_none());
    }

    #[tokio::test]
    #[ignore = "Network and yt-dlp required"]
    async fn test_downloading_a_song_with_ytdlp() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), None, None, "chromium".to_string(), AudioQuality::default());
        let YoutubeMusicDownload { song: stream, .. } =
            downloader.stream_song("lYBUbBu4W08", AudioQuality::default()).await.unwrap();
        stream
            .map(|item| item.unwrap())
            .collect::<Vec<Bytes>>()
            .await;
    }
}
