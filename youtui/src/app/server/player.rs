use super::song_downloader::InMemSong;
use crate::app::structures::ListSongID;
use crate::async_rodio_sink::rodio::Decoder;
use crate::async_rodio_sink::rodio::decoder::DecoderError;
use crate::async_rodio_sink::{self, AsyncRodio};
use anyhow::Context;
use futures::Stream;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

pub struct DecodedInMemSong(Decoder<Cursor<ArcInMemSong>>);
struct ArcInMemSong(Arc<InMemSong>);

// Derive to assist with debub printing tasks
impl std::fmt::Debug for DecodedInMemSong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DecodedInMemSong").field(&"..").finish()
    }
}

impl AsRef<[u8]> for ArcInMemSong {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().0.as_ref()
    }
}

pub struct Player {
    rodio_handle: AsyncRodio<Decoder<Cursor<ArcInMemSong>>, ListSongID>,
}

// Consider if this can be managed by Server.
impl Player {
    pub fn new() -> Self {
        let rodio_handle = AsyncRodio::new();
        Self { rodio_handle }
    }
    pub fn autoplay_song(
        &self,
        song: DecodedInMemSong,
        song_id: ListSongID,
    ) -> impl Stream<Item = async_rodio_sink::AutoplayUpdate<ListSongID>> + use<> {
        self.rodio_handle.autoplay_song(song.0, song_id)
    }
    pub fn play_song(
        &self,
        song: DecodedInMemSong,
        song_id: ListSongID,
    ) -> impl Stream<Item = async_rodio_sink::PlayUpdate<ListSongID>> + use<> {
        self.rodio_handle.play_song(song.0, song_id)
    }
    pub fn queue_song(
        &self,
        song: DecodedInMemSong,
        song_id: ListSongID,
    ) -> impl Stream<Item = async_rodio_sink::QueueUpdate<ListSongID>> + use<> {
        self.rodio_handle.queue_song(song.0, song_id)
    }
    pub async fn seek(
        &self,
        duration: Duration,
        direction: async_rodio_sink::SeekDirection,
    ) -> Option<async_rodio_sink::ProgressUpdate<ListSongID>> {
        self.rodio_handle.seek(duration, direction).await
    }
    pub async fn seek_to(
        &self,
        seek_to_pos: Duration,
        id: ListSongID,
    ) -> Option<async_rodio_sink::ProgressUpdate<ListSongID>> {
        self.rodio_handle.seek_to(seek_to_pos, id).await
    }
    pub async fn stop(&self, song_id: ListSongID) -> Option<async_rodio_sink::Stopped<ListSongID>> {
        self.rodio_handle.stop(song_id).await
    }
    pub async fn stop_all(&self) -> Option<async_rodio_sink::AllStopped> {
        self.rodio_handle.stop_all().await
    }
    pub async fn pause_play(
        &self,
        song_id: ListSongID,
    ) -> Option<async_rodio_sink::PausePlayResponse<ListSongID>> {
        self.rodio_handle.pause_play(song_id).await
    }
    pub async fn resume(
        &self,
        song_id: ListSongID,
    ) -> Option<async_rodio_sink::Resumed<ListSongID>> {
        self.rodio_handle.resume(song_id).await
    }
    pub async fn pause(&self, song_id: ListSongID) -> Option<async_rodio_sink::Paused<ListSongID>> {
        self.rodio_handle.pause(song_id).await
    }
    pub async fn increase_volume(&self, vol_inc: i8) -> Option<async_rodio_sink::VolumeUpdate> {
        self.rodio_handle.increase_volume(vol_inc).await
    }
    pub async fn set_volume(&self, new_vol: u8) -> Option<async_rodio_sink::VolumeUpdate> {
        self.rodio_handle.set_volume(new_vol).await
    }
    pub async fn try_decode(
        song: Arc<InMemSong>,
        start_offset: Option<Duration>,
        actual_duration: Option<Duration>,
    ) -> std::result::Result<DecodedInMemSong, DecoderError> {
        tokio::task::spawn_blocking(move || try_decode(song, start_offset, actual_duration))
            .await
            .map_err(|e| {
                tracing::error!("spawn_blocking for decode failed: {e}");
                DecoderError::UnrecognizedFormat
            })?
    }
}

/// Decode audio bytes into a rodio Source.
/// When both start_offset AND actual_duration are Some: uses ffmpeg to extract
/// the exact track section (`-ss offset -t duration`) before decoding.
/// This ensures each album track plays the correct audio for its exact length,
/// enabling gapless QueueDecodedSong transitions.
fn try_decode(song: Arc<InMemSong>, start_offset: Option<Duration>, actual_duration: Option<Duration>) -> std::result::Result<DecodedInMemSong, DecoderError> {
    let (data, len) = if let (Some(offset), Some(dur)) = (start_offset, actual_duration) {
        match extract_section(&song, offset, Some(dur)) {
            Ok(extracted) => {
                let len = extracted.0.len();
                info!("Extracted section offset={:?} dur={:?}: {} bytes", offset, dur, len);
                (Arc::new(extracted), len)
            }
            Err(e) => {
                info!("ffmpeg extract failed: {}, using full audio", e);
                let len = song.as_ref().0.len();
                (song, len)
            }
        }
    } else if let Some(offset) = start_offset {
        if !offset.is_zero() {
            match extract_section(&song, offset, None) {
                Ok(extracted) => {
                    let len = extracted.0.len();
                    info!("Extracted section at {:?}: {} bytes", offset, len);
                    (Arc::new(extracted), len)
                }
                Err(e) => {
                    info!("ffmpeg extract at {:?} failed: {}", offset, e);
                    let len = song.as_ref().0.len();
                    (song, len)
                }
            }
        } else {
            let len = song.as_ref().0.len();
            (song, len)
        }
    } else {
        let len = song.as_ref().0.len();
        (song, len)
    };

    // DEBUG: log audio format before decode
    let header: Vec<u8> = data.as_ref().0.iter().take(16).copied().collect();
    tracing::info!(
        "try_decode: {} bytes, header={:02x?}, offset={:?}, dur={:?}",
        len, header, start_offset, actual_duration
    );

    // Try direct decode first (fast path)
    let wrapper = ArcInMemSong(data.clone());
    let cur = std::io::Cursor::new(wrapper);
    let direct_result = async_rodio_sink::rodio::Decoder::builder()
        .with_data(cur)
        .with_gapless(true)
        .with_byte_len(
            len.try_into()
                .expect("Expected usize to be smaller than or equal to u64"),
        )
        .with_seekable(true)
        .build();

    match direct_result {
        Ok(decoder) => Ok(DecodedInMemSong(decoder)),
        Err(_) => {
            tracing::info!("direct rodio decode failed, trying WAV conversion");
            // Fallback: convert to WAV via ffmpeg and decode that
            let raw: &[u8] = &data.as_ref().0.as_slice()[..len];
            let wav = match convert_to_wav_fallback(raw) {
                Some(w) => w,
                None => return Err(DecoderError::UnrecognizedFormat),
            };
            let wav_len = wav.len();
            let wav_arc = Arc::new(InMemSong(wav));
            let wrapper = ArcInMemSong(wav_arc);
            let cur = std::io::Cursor::new(wrapper);
            Ok(DecodedInMemSong(
                async_rodio_sink::rodio::Decoder::builder()
                    .with_data(cur)
                    .with_gapless(true)
                    .with_byte_len(
                        wav_len.try_into()
                            .expect("Expected usize to be smaller than or equal to u64"),
                    )
                    .with_seekable(true)
                    .build()?,
            ))
        }
    }
}

/// Fallback: convert any audio bytes to WAV via ffmpeg. Returns None on failure.
fn convert_to_wav_fallback(data: &[u8]) -> Option<Vec<u8>> {
    let pid = std::process::id();
    let in_file = format!("/tmp/youtui_wav_{}.bin", pid);
    let out_file = format!("/tmp/youtui_wav_{}.wav", pid);

    let _ = std::fs::write(&in_file, data);
    let status = std::process::Command::new("ffmpeg")
        .args(&["-loglevel", "error", "-y", "-i", &in_file, "-vn", "-f", "wav", &out_file])
        .status()
        .ok()?;
    if !status.success() {
        let _ = std::fs::remove_file(&in_file);
        let _ = std::fs::remove_file(&out_file);
        return None;
    }
    let wav = std::fs::read(&out_file).ok()?;
    let _ = std::fs::remove_file(&in_file);
    let _ = std::fs::remove_file(&out_file);
    if wav.is_empty() { None } else { Some(wav) }
}

/// Remux fragmented MP4 (YouTube's moov atom at end) to a seekable file.
fn remux_moov(in_file: &str) -> bool {
    let tmp = format!("{}_remuxed", in_file);
    let ok = std::process::Command::new("ffmpeg")
        .args(&["-loglevel", "error", "-i", in_file, "-c", "copy", "-movflags", "+faststart", &tmp])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        let _ = std::fs::rename(&tmp, in_file);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
    ok
}

/// Extract a section of audio via ffmpeg.
/// Writes full audio to temp file, runs ffmpeg with fast copy-based seek first.
/// Falls back to accurate decode-based seek if fast path produces garbage.
fn extract_section(song: &Arc<InMemSong>, offset: Duration, duration: Option<Duration>) -> anyhow::Result<InMemSong> {
    let pid = std::process::id();
    let in_file = format!("/tmp/youtui_seek_{}.m4a", pid);
    let out_file = format!("/tmp/youtui_seek_{}_out.m4a", pid);

    std::fs::write(&in_file, &song.as_ref().0).context("write temp input")?;

    let total_secs = offset.as_secs_f64();
    let h = total_secs as u64 / 3600;
    let m = (total_secs as u64 % 3600) / 60;
    let s = total_secs as u64 % 60;
    let start_fmt = format!("{}:{:02}:{:02}", h, m, s);

    let (dur_fmt, expected_secs) = duration.map(|dur| {
        let dur_secs = dur.as_secs_f64();
        let dh = dur_secs as u64 / 3600;
        let dm = (dur_secs as u64 % 3600) / 60;
        let ds = dur_secs as u64 % 60;
        (format!("{}:{:02}:{:02}", dh, dm, ds), dur_secs)
    }).unwrap_or_else(|| (String::new(), 0.0));

    // Fast path: keyframe-seeking with stream copy
    let run_ffmpeg = |args: &[&str]| -> anyhow::Result<Vec<u8>> {
        let output = std::process::Command::new("ffmpeg")
            .args(args)
            .output()
            .context("spawn ffmpeg")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ffmpeg: {}", stderr.trim());
        }
        let data = std::fs::read(&out_file).context("read ffmpeg output")?;
        Ok(data)
    };

    let fast_args = if !dur_fmt.is_empty() {
        vec!["-loglevel", "error", "-ss", &start_fmt, "-i", &in_file,
             "-c", "copy", "-avoid_negative_ts", "1",
             "-t", &dur_fmt, &out_file]
    } else {
        vec!["-loglevel", "error", "-ss", &start_fmt, "-i", &in_file,
             "-c", "copy", "-avoid_negative_ts", "1", &out_file]
    };

    let try_both = |in_file: &str| -> anyhow::Result<Vec<u8>> {
        match extract_accurate(in_file, &out_file, &start_fmt, &dur_fmt) {
            Ok(data) => Ok(data),
            Err(e) => {
                let msg = format!("{}", e);
                if msg.contains("moov") || msg.contains("Invalid data") {
                    info!("ffmpeg: moov issue detected, remuxing file");
                    let _ = std::fs::remove_file(&out_file);
                    if remux_moov(in_file) {
                        extract_accurate(in_file, &out_file, &start_fmt, &dur_fmt)
                    } else {
                        Err(e)
                    }
                } else {
                    Err(e)
                }
            }
        }
    };

    let data = match run_ffmpeg(&fast_args) {
        Ok(data) => {
            let min_expected = if expected_secs > 0.0 {
                (expected_secs * 2000.0) as usize
            } else {
                1024
            };
            if data.len() >= min_expected.min(1024) {
                let _ = std::fs::remove_file(&in_file);
                let _ = std::fs::remove_file(&out_file);
                return Ok(InMemSong(data));
            }
            info!("ffmpeg fast path: small output ({}b, expected >{}b), retrying accurate", data.len(), min_expected);
            let _ = std::fs::remove_file(&out_file);
            try_both(&in_file)?
        }
        Err(e) => {
            info!("ffmpeg fast path failed: {}, trying accurate", e);
            let _ = std::fs::remove_file(&out_file);
            try_both(&in_file)?
        }
    };

    let _ = std::fs::remove_file(&in_file);
    let _ = std::fs::remove_file(&out_file);
    Ok(InMemSong(data))
}

/// Accurate seek fallback: decode from start, cut at exact offset
fn extract_accurate(in_file: &str, out_file: &str, start_fmt: &str, dur_fmt: &str) -> anyhow::Result<Vec<u8>> {
    let mut args: Vec<&str> = vec![
        "-loglevel", "error",
        "-i", in_file,
        "-ss", start_fmt,
        "-c:a", "aac",
        "-b:a", "192k",
        "-avoid_negative_ts", "1",
    ];
    if !dur_fmt.is_empty() {
        args.push("-t");
        args.push(dur_fmt);
    }
    args.push(out_file);

    let output = std::process::Command::new("ffmpeg")
        .args(&args)
        .output()
        .context("spawn ffmpeg")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg accurate: {}", stderr.trim());
    }
    let data = std::fs::read(out_file).context("read ffmpeg accurate output")?;
    Ok(data)
}
