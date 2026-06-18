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
            .expect("Try decode should not panic")
    }
}

/// Decode audio bytes into a rodio Source.
/// When both start_offset AND actual_duration are Some: uses ffmpeg to extract
/// the exact track section (`-ss offset -t duration`) before decoding.
/// This ensures each album track plays the correct audio for its exact length,
/// enabling gapless QueueDecodedSong transitions.
fn try_decode(song: Arc<InMemSong>, start_offset: Option<Duration>, actual_duration: Option<Duration>) -> std::result::Result<DecodedInMemSong, DecoderError> {
    let (data, len) = if let (Some(offset), Some(dur)) = (start_offset, actual_duration) {
        // Extract section from offset with exact track duration (-ss + -t)
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

    let wrapper = ArcInMemSong(data);
    let cur = std::io::Cursor::new(wrapper);
    Ok(DecodedInMemSong(
        async_rodio_sink::rodio::Decoder::builder()
            .with_data(cur)
            .with_gapless(true)
            .with_byte_len(
                len.try_into()
                    .expect("Expected usize to be smaller than or equal to u64"),
            )
            .with_seekable(true)
            .build()?,
    ))
}

/// Extract a section of audio via ffmpeg.
/// Writes the full audio to a temp file, runs `ffmpeg -ss OFFSET [-t DURATION] -c copy`,
/// reads the output back as InMemSong. Used by try_decode when start_offset is present.
/// Falls back gracefully on ffmpeg failure (plays full audio from beginning).
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

    let dur_fmt = duration.map(|dur| {
        let dur_secs = dur.as_secs_f64();
        let dh = dur_secs as u64 / 3600;
        let dm = (dur_secs as u64 % 3600) / 60;
        let ds = dur_secs as u64 % 60;
        format!("{}:{:02}:{:02}", dh, dm, ds)
    });
    let mut args: Vec<&str> = vec![
        "-loglevel", "error",
        "-ss", &start_fmt,
        "-i", &in_file,
        "-c", "copy",
    ];
    if let Some(ref df) = dur_fmt {
        args.push("-t");
        args.push(df);
    }
    args.push(&out_file);

    let output = std::process::Command::new("ffmpeg")
        .args(&args)
        .output()
        .context("spawn ffmpeg")?;

    let _ = std::fs::remove_file(&in_file);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&out_file);
        anyhow::bail!("ffmpeg: {}", stderr.trim());
    }

    let data = std::fs::read(&out_file).context("read ffmpeg output")?;
    let _ = std::fs::remove_file(&out_file);
    Ok(InMemSong(data))
}
