use crate::app::AudioQuality;
use crate::youtube_downloader::{YoutubeMusicDownload, YoutubeMusicDownloader};
use bytes::Bytes;
use futures::Stream;
use std::ffi::OsString;
use std::ops::Deref;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

#[derive(Clone)]
#[allow(dead_code)] // field po_token passed at construction, read by yt-dlp subprocess
/// # Note
/// Cheap to clone due to use of Arc to store internals.
pub struct YtDlpDownloader {
    yt_dlp_command: Arc<OsString>,
    po_token: Option<String>,
    cookie_path: Option<String>,
    cookie_browser: String,
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
    pub fn new(yt_dlp_command: String, po_token: Option<String>, cookie_path: Option<String>, cookie_browser: String) -> Self {
        Self {
            yt_dlp_command: Arc::new(yt_dlp_command.into()),
            po_token,
            cookie_path,
            cookie_browser,
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
        quality: AudioQuality,
    ) -> Result<
        YoutubeMusicDownload<impl Stream<Item = Result<Bytes, Self::Error>> + Send>,
        Self::Error,
    > {
        let command = self.yt_dlp_command.clone();
        async move {
            let video_id = song_video_id.as_ref().to_string();
            let format_string = quality.format_string().to_string();
            info!(%video_id, quality = ?quality, format = %format_string, "Starting yt-dlp download");

            // Temp dir with an %(ext)s template: Best can pick non-m4a audio
            // (opus in webm), so the output extension is decided by yt-dlp.
            let tmpdir = tempfile::tempdir().map_err(|e| {
                YtDlpDownloaderError::IoError {
                    message: format!("Failed to create temp dir: {e}"),
                }
            })?;
            let template = tmpdir.path().join("audio.%(ext)s");
            let output_template = template.to_str().unwrap().to_owned();
            
            // web_creator extractor needs cookies - only use it when configured
            // Default extractor works without auth for most videos
            let use_web_creator = self.cookie_path.is_some();
            
            let mut stream_args = vec![
                "--no-simulate",
                "--force-overwrites",
                "--no-warnings",
                "--no-progress",
                "--print",
                "after_move:YTDLP_META abr=%(abr)s ext=%(ext)s format=%(format_id)s",
                "-f",
                format_string.as_str(),
                "-o",
                output_template.as_str(),
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
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    error!(%video_id, error = %e, "Failed to spawn yt-dlp process");
                    YtDlpDownloaderError::IoError {
                        message: format!("{e}"),
                    }
                })?;
            
            // Take stderr before spawn to avoid partial move
            let stderr = proc.stderr.take().unwrap();
            let mut stdout = proc.stdout.take().unwrap();
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

            // --print output is one short line; process already exited so this returns at once.
            let mut print_out = String::new();
            let _ = stdout.read_to_string(&mut print_out).await;
            let (dl_abr, dl_ext, dl_format) = parse_print_meta(&print_out);

            // Find the downloaded file (extension decided by yt-dlp via %(ext)s).
            let mut best_path: Option<(u64, std::path::PathBuf)> = None;
            for entry in std::fs::read_dir(tmpdir.path()).map_err(|e| {
                YtDlpDownloaderError::IoError {
                    message: format!("Failed to list temp dir: {e}"),
                }
            })? {
                let entry = entry.map_err(|e| YtDlpDownloaderError::IoError {
                    message: format!("Failed to read temp dir entry: {e}"),
                })?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if path.extension().is_some_and(|e| e == "part") {
                    continue;
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if best_path.as_ref().is_none_or(|(s, _)| size > *s) {
                    best_path = Some((size, path));
                }
            }
            let (_, output_path) = best_path.ok_or_else(|| {
                error!(%video_id, "yt-dlp produced no output file");
                YtDlpDownloaderError::IoError {
                    message: "yt-dlp produced no output file".to_string(),
                }
            })?;

            // Read completed file into memory
            let file_bytes = tokio::fs::read(&output_path).await.map_err(|e| {
                error!(%video_id, error = %e, "Failed to read yt-dlp output");
                YtDlpDownloaderError::IoError {
                    message: format!("Failed to read output file: {e}"),
                }
            })?;

            // Temp dir cleaned up on drop
            drop(tmpdir);
            
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
            
            info!(%video_id, quality = ?quality, format = %format_string, ext = %dl_ext, abr = %dl_abr, yt_format = %dl_format, bytes = %total_size_bytes, container = %format_name, "yt-dlp download completed");
            
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

fn parse_print_meta(output: &str) -> (String, String, String) {
    let mut abr = "unknown".to_string();
    let mut ext = "unknown".to_string();
    let mut format = "unknown".to_string();
    for line in output.lines() {
        let Some((_, rest)) = line.split_once("YTDLP_META ") else {
            continue;
        };
        for token in rest.split_whitespace() {
            let Some((k, v)) = token.split_once('=') else {
                continue;
            };
            match k {
                "abr" => abr = v.to_string(),
                "ext" => ext = v.to_string(),
                "format" => format = v.to_string(),
                _ => {}
            }
        }
    }
    (abr, ext, format)
}

#[cfg(test)]
mod tests {
    use crate::youtube_downloader::yt_dlp::YtDlpDownloader;
    use crate::youtube_downloader::{YoutubeMusicDownload, YoutubeMusicDownloader};
    use bytes::Bytes;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_yt_dlp_downloader_with_po_token() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), Some("test_po_token".to_string()), None, "chromium".to_string());
        assert!(downloader.po_token.is_some());
        assert_eq!(downloader.po_token.unwrap(), "test_po_token");
    }

    #[tokio::test]
    async fn test_yt_dlp_downloader_without_po_token() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), None, None, "chromium".to_string());
        assert!(downloader.po_token.is_none());
    }

    #[tokio::test]
    async fn test_error_display_messages() {
        use crate::youtube_downloader::yt_dlp::YtDlpDownloaderError;
        let cases = [
            (
                YtDlpDownloaderError::IoError {
                    message: "boom".to_string(),
                },
                "Error running yt-dlp - <boom>",
            ),
            (
                YtDlpDownloaderError::NoOutput,
                "Error running yt-dlp - no output when output was expected",
            ),
            (
                YtDlpDownloaderError::InvalidFilesizeOutput {
                    output: "xyz".to_string(),
                },
                "Error parsing filesize output: xyz",
            ),
            (
                YtDlpDownloaderError::FormatNotAvailable {
                    video_id: "abc".to_string(),
                },
                "Error running yt-dlp - format not available for video abc",
            ),
            (
                YtDlpDownloaderError::AuthenticationError {
                    video_id: "abc".to_string(),
                    message: "bad".to_string(),
                },
                "Error running yt-dlp - authentication failed for video abc: bad",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(format!("{err}"), expected);
        }
    }

    #[tokio::test]
    #[ignore = "needs real YouTube access - blocked from CI sandboxes, run locally"]
    async fn test_downloading_a_song_with_ytdlp() {
        let downloader = YtDlpDownloader::new("yt-dlp".to_string(), None, None, "chromium".to_string());
        let YoutubeMusicDownload { song: stream, .. } =
            downloader.stream_song("lYBUbBu4W08", crate::app::AudioQuality::Best).await.unwrap();
        stream
            .map(|item| item.unwrap())
            .collect::<Vec<Bytes>>()
            .await;
    }

    #[test]
    fn test_parse_print_meta() {
        let (abr, ext, format) =
            super::parse_print_meta("YTDLP_META abr=160 ext=webm format=251\n");
        assert_eq!(abr, "160");
        assert_eq!(ext, "webm");
        assert_eq!(format, "251");
    }

    #[test]
    fn test_parse_print_meta_missing() {
        let (abr, ext, format) = super::parse_print_meta("some other output\n");
        assert_eq!(abr, "unknown");
        assert_eq!(ext, "unknown");
        assert_eq!(format, "unknown");
    }
}
