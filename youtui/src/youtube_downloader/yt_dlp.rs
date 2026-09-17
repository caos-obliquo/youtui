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
            
            // The cookie file holds a single consistent session. Gate on file
            // existence at download time: cookie_path is always Some (see
            // main.rs), so Option::is_some alone cannot tell a real session
            // from a missing file.
            let cookie_file = effective_cookie_file(self.cookie_path.as_deref());
            let browser_fallback =
                if cookie_file.is_none() && self.cookie_path.is_some() {
                    Some(self.cookie_browser.as_str())
                } else {
                    None
                };
            let stream_args = build_stream_args(
                format_string.as_str(),
                output_template.as_str(),
                song_video_id.as_ref(),
                cookie_file,
                browser_fallback,
            );
            
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
            if is_progressive_fallback(&dl_format) {
                error!(%video_id, format = %dl_format, abr = %dl_abr, ext = %dl_ext, "progressive fallback format picked, audio is ~96k - check cookie/auth");
            }

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

fn effective_cookie_file(cookie_path: Option<&str>) -> Option<&str> {
    cookie_path.filter(|p| std::path::Path::new(p).exists())
}

fn build_stream_args<'a>(
    format_string: &'a str,
    output_template: &'a str,
    video_id: &'a str,
    cookie_file: Option<&'a str>,
    cookie_browser: Option<&'a str>,
) -> Vec<&'a str> {
    let mut args = vec![
        "--no-simulate",
        "--force-overwrites",
        "--no-warnings",
        "--no-progress",
        "--print",
        "after_move:YTDLP_META abr=%(abr)s ext=%(ext)s format=%(format_id)s",
        "-f",
        format_string,
        "-o",
        output_template,
    ];
    if let Some(path) = cookie_file {
        // Single consistent session from the exported file. The live browser
        // profile can merge cookies from multiple sessions which YouTube
        // rejects, so it is dropped here.
        args.push("--extractor-args");
        args.push("youtube:player_client=web_creator");
        args.push("--cookies");
        args.push(path);
    } else if let Some(browser) = cookie_browser {
        args.push("--extractor-args");
        args.push("youtube:player_client=web_creator");
        args.push("--cookies-from-browser");
        args.push(browser);
    }
    // End-of-options separator: video ids can start with a dash
    // (e.g. -nIkN6le_wY) and yt-dlp would parse them as flags.
    args.push("--");
    args.push(video_id);
    args
}

fn is_progressive_fallback(format_id: &str) -> bool {
    format_id == "18" || format_id == "22"
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
    fn test_build_stream_args_with_cookie_file() {
        let args = super::build_stream_args(
            "bestaudio/best",
            "/tmp/audio.%(ext)s",
            "videoid123",
            Some("/home/user/.config/youtui/cookie.txt"),
            None,
        );
        let cookies_pos = args.iter().position(|a| *a == "--cookies").unwrap();
        assert_eq!(args[cookies_pos + 1], "/home/user/.config/youtui/cookie.txt");
        assert!(args.contains(&"youtube:player_client=web_creator"));
        assert!(!args.iter().any(|a| *a == "--cookies-from-browser"));
        assert_eq!(args.last().unwrap(), &"videoid123");
    }

    #[test]
    fn test_build_stream_args_browser_fallback() {
        let args = super::build_stream_args(
            "bestaudio/best",
            "/tmp/audio.%(ext)s",
            "videoid123",
            None,
            Some("chromium"),
        );
        let pos = args.iter().position(|a| *a == "--cookies-from-browser").unwrap();
        assert_eq!(args[pos + 1], "chromium");
        assert!(!args.iter().any(|a| *a == "--cookies"));
    }

    #[test]
    fn test_build_stream_args_no_auth() {
        let args =
            super::build_stream_args("bestaudio/best", "/tmp/audio.%(ext)s", "videoid123", None, None);
        assert!(!args.iter().any(|a| *a == "--cookies"));
        assert!(!args.iter().any(|a| *a == "--cookies-from-browser"));
        assert!(!args.iter().any(|a| *a == "--extractor-args"));
    }

    #[test]
    fn test_build_stream_args_end_of_options_before_id() {
        for id in ["videoid123", "-nIkN6le_wY"] {
            let args = super::build_stream_args(
                "bestaudio/best",
                "/tmp/audio.%(ext)s",
                id,
                None,
                None,
            );
            let id_pos = args.iter().position(|a| *a == id).unwrap();
            assert_eq!(args[id_pos - 1], "--");
            assert_eq!(args.last().unwrap(), &id);
        }
    }

    #[test]
    fn test_effective_cookie_file_missing() {
        assert!(super::effective_cookie_file(None).is_none());
        assert!(super::effective_cookie_file(Some("/nonexistent/path/cookie.txt")).is_none());
    }

    #[test]
    fn test_effective_cookie_file_present() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookie.txt");
        std::fs::write(&path, "# cookie data").unwrap();
        let path_str = path.to_string_lossy().to_string();
        assert_eq!(super::effective_cookie_file(Some(&path_str)), Some(path_str.as_str()));
    }

    #[test]
    fn test_is_progressive_fallback() {
        assert!(super::is_progressive_fallback("18"));
        assert!(super::is_progressive_fallback("22"));
        assert!(!super::is_progressive_fallback("251"));
        assert!(!super::is_progressive_fallback("140"));
        assert!(!super::is_progressive_fallback("unknown"));
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
