use futures::Stream;
use std::future::Future;

use crate::app::AudioQuality;

pub mod native;
pub mod yt_dlp;

pub struct YoutubeMusicDownload<S> {
    pub total_size_bytes: usize,
    pub song: S,
}

pub trait YoutubeMusicDownloader {
    type Error;
    fn stream_song(
        &self,
        song_video_id: impl AsRef<str> + Send,
        quality: AudioQuality,
    ) -> impl Future<
        Output = Result<
            YoutubeMusicDownload<impl Stream<Item = Result<bytes::Bytes, Self::Error>> + Send>,
            Self::Error,
        >,
    > + Send;
}
