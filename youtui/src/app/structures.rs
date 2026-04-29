use serde::{Serialize, Deserialize};
use super::server::song_downloader::InMemSong;
use super::server::song_thumbnail_downloader::SongThumbnail;
use super::view::SortDirection;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use itertools::Itertools;
use std::borrow::Cow;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use ytmapi_rs::common::{
    AlbumID, ArtistChannelID, Explicit, UploadAlbumID, UploadArtistID, VideoID, YoutubeID,
};
pub use ytmapi_rs::common::Thumbnail;
use ytmapi_rs::parse::{
    AlbumSong, ParsedSongAlbum, ParsedSongArtist, ParsedUploadArtist, ParsedUploadSongAlbum,
    PlaylistEpisode, PlaylistItem, PlaylistSong, PlaylistUploadSong, PlaylistVideo,
    SearchResultSong,
};

pub trait SongListComponent {
    fn get_song_from_idx(&self, idx: usize) -> Option<&ListSong>;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MaybeRc<T> {
    Rc(Rc<T>),
    Owned(T),
}
impl<T> Deref for MaybeRc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match self {
            MaybeRc::Rc(rc) => rc.deref(),
            MaybeRc::Owned(t) => t,
        }
    }
}
impl<T> AsRef<T> for MaybeRc<T> {
    fn as_ref(&self) -> &T {
        match self {
            MaybeRc::Rc(rc) => rc,
            MaybeRc::Owned(t) => t,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrowserSongsList {
    pub state: ListStatus,
    list: Vec<ListSong>,
    pub next_id: ListSongID,
}

// As this is a simple wrapper type we implement Copy for ease of handling
#[derive(Clone, PartialEq, Copy, Debug, PartialOrd, Hash, Eq, Serialize, Deserialize)]
pub struct ListSongID(#[cfg(test)] pub usize, #[cfg(not(test))] usize);

// As this is a simple wrapper type we implement Copy for ease of handling
#[derive(Clone, PartialEq, Copy, Debug, Default, PartialOrd, Serialize, Deserialize)]
pub struct Percentage(pub u8);

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum AlbumArtState {
    #[default]
    Init,
    #[serde(skip)]
    Downloaded(Rc<SongThumbnail>),
    None,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListSong {
    pub video_id: VideoID<'static>,
    pub track_no: Option<usize>,
    pub plays: String,
    pub title: String,
    pub explicit: Option<Explicit>,
    pub download_status: DownloadStatus,
    pub id: ListSongID,
    pub duration_string: String,
    pub actual_duration: Option<Duration>,
    pub year: Option<Rc<String>>,
    pub album_art: AlbumArtState,
    pub artists: MaybeRc<Vec<ListSongArtist>>,
    pub thumbnails: MaybeRc<Vec<Thumbnail>>,
    pub album: Option<MaybeRc<ListSongAlbum>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListSongArtist {
    pub name: String,
    pub id: Option<ArtistOrUploadArtistID>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListSongAlbum {
    pub name: String,
    pub id: AlbumOrUploadAlbumID,
}

impl From<ParsedSongArtist> for ListSongArtist {
    fn from(value: ParsedSongArtist) -> Self {
        let ParsedSongArtist { name, id } = value;
        Self {
            name,
            id: id.map(ArtistOrUploadArtistID::Artist),
        }
    }
}

impl From<ParsedUploadArtist> for ListSongArtist {
    fn from(value: ParsedUploadArtist) -> Self {
        let ParsedUploadArtist { name, id } = value;
        Self {
            name,
            id: id.map(ArtistOrUploadArtistID::UploadArtist),
        }
    }
}

impl From<ParsedSongAlbum> for ListSongAlbum {
    fn from(value: ParsedSongAlbum) -> Self {
        let ParsedSongAlbum { name, id } = value;
        Self {
            name,
            id: AlbumOrUploadAlbumID::Album(id),
        }
    }
}

impl From<ParsedUploadSongAlbum> for ListSongAlbum {
    fn from(value: ParsedUploadSongAlbum) -> Self {
        let ParsedUploadSongAlbum { name, id } = value;
        Self {
            name,
            id: AlbumOrUploadAlbumID::UploadAlbum(id),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ArtistOrUploadArtistID {
    Artist(ArtistChannelID<'static>),
    UploadArtist(UploadArtistID<'static>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AlbumOrUploadAlbumID {
    Album(AlbumID<'static>),
    UploadAlbum(UploadAlbumID<'static>),
}

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListSongDisplayableField {
    DownloadStatus,
    TrackNo,
    Artists,
    Album,
    Song,
    Duration,
    Year,
    Plays,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ListStatus {
    New,
    Loading,
    InProgress,
    Loaded,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DownloadStatus {
    None,
    Queued,
    Downloading(Percentage),
    #[serde(skip)]
    Downloaded(Arc<InMemSong>),
    Failed,
    Retrying { times_retried: usize },
}

impl Default for DownloadStatus {
    fn default() -> Self {
        DownloadStatus::None
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlayState {
    NotPlaying,
    Playing(ListSongID),
    Paused(ListSongID),
    // May be the same as NotPlaying?
    Stopped,
    Error(ListSongID),
    Buffering(ListSongID),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AudioQuality {
    Best,
    High,
    Medium,
    Low,
}

impl Default for AudioQuality {
    fn default() -> Self {
        AudioQuality::Low
    }
}

impl PlayState {
    #[allow(dead_code)]
    pub fn list_icon(&self) -> char {
        match self {
            PlayState::Buffering(_) => '',
            PlayState::NotPlaying => '',
            PlayState::Playing(_) => '',
            PlayState::Paused(_) => '',
            PlayState::Stopped => '',
            PlayState::Error(_) => '',
        }
    }
}

impl DownloadStatus {
    pub fn list_icon(&self) -> char {
        match self {
            Self::Failed => 'X',
            Self::Queued => '↓',
            Self::None => ' ',
            Self::Downloading(_) => '↓',
            Self::Downloaded(_) => '✓',
            Self::Retrying { .. } => '↻',
        }
    }
    pub fn list_icon_str(&self) -> &'static str {
        match self {
            Self::Failed => "X",
            Self::Queued => "↓",
            Self::None => " ",
            Self::Downloading(_) => "↓",
            Self::Downloaded(_) => "✓",
            Self::Retrying { .. } => "↻",
        }
    }
}

fn compute_artists_string(artists: &[ListSongArtist]) -> String {
    Itertools::intersperse(artists.iter().map(|a| a.name.as_str()), ", ").collect()
}

impl ListSong {
    pub fn get_fields<const N: usize>(
        &self,
        fields: [ListSongDisplayableField; N],
    ) -> [Cow<'_, str>; N] {
        fields.map(|field| self.get_field(field))
    }
    pub fn get_field(&self, field: ListSongDisplayableField) -> Cow<'_, str> {
        match field {
            ListSongDisplayableField::DownloadStatus => {
                Cow::Borrowed(self.download_status.list_icon_str())
            },
            ListSongDisplayableField::TrackNo => self
                .track_no
                .map(|track_no| track_no.to_string())
                .unwrap_or_default()
                .into(),
            ListSongDisplayableField::Artists => compute_artists_string(&self.artists).into(),
            ListSongDisplayableField::Album => self
                .album
                .as_ref()
                .map(|album| album.as_ref().name.as_str())
                .unwrap_or_default()
                .into(),
            ListSongDisplayableField::Year => self
                .year
                .as_ref()
                .map(|year| year.as_str())
                .unwrap_or_default()
                .into(),
            ListSongDisplayableField::Song => self.title.as_str().into(),
            ListSongDisplayableField::Duration => self.duration_string.as_str().into(),
            ListSongDisplayableField::Plays => self.plays.as_str().into(),
        }
    }
    pub fn create_with_metadata(
        video_id: VideoID<'static>,
        title: String,
        artists: Vec<String>,
        album: Option<String>,
        duration_string: String,
        thumbnail_url: Option<String>,
    ) -> Self {
        let thumb = thumbnail_url.map(|url| {
            let thumb = Thumbnail {
                height: 200,
                width: 200,
                url,
            };
            vec![thumb]
        });
        let list_artists: Vec<ListSongArtist> = artists
            .iter()
            .map(|name| ListSongArtist {
                name: name.clone(),
                id: None,
            })
            .collect();
        let list_album = album.map(|name| MaybeRc::Owned(ListSongAlbum {
            name,
            id: AlbumOrUploadAlbumID::Album(AlbumID::from_raw("")),
        }));
        ListSong {
            video_id,
            track_no: None,
            plays: String::new(),
            title,
            explicit: None,
            download_status: DownloadStatus::None,
            id: ListSongID(0),
            duration_string,
            actual_duration: None,
            year: None,
            album_art: AlbumArtState::Init,
            artists: MaybeRc::Owned(list_artists),
            thumbnails: MaybeRc::Owned(thumb.unwrap_or_default()),
            album: list_album,
        }
    }
}

impl Default for BrowserSongsList {
    fn default() -> Self {
        BrowserSongsList {
            state: ListStatus::New,
            list: Vec::new(),
            next_id: ListSongID(0),
        }
    }
}

impl BrowserSongsList {
    pub fn get_list_iter(&self) -> std::slice::Iter<'_, ListSong> {
        self.list.iter()
    }
    pub fn get_list_iter_mut(&mut self) -> std::slice::IterMut<'_, ListSong> {
        self.list.iter_mut()
    }
    pub fn sort(&mut self, field: ListSongDisplayableField, direction: SortDirection) {
        self.list.sort_by(|a, b| match direction {
            SortDirection::Asc => a
                .get_field(field)
                .partial_cmp(&b.get_field(field))
                .unwrap_or(std::cmp::Ordering::Equal),
            SortDirection::Desc => b
                .get_field(field)
                .partial_cmp(&a.get_field(field))
                .unwrap_or(std::cmp::Ordering::Equal),
        });
    }
    pub fn clear(&mut self) {
        // We can't reset the ID, so it's left out and we'll keep incrementing.
        self.state = ListStatus::New;
        self.list.clear();
    }
    pub fn append_raw_album_songs(
        &mut self,
        raw_list: Vec<AlbumSong>,
        album: ParsedSongAlbum,
        year: String,
        artists: Vec<ParsedSongArtist>,
        thumbnails: Vec<Thumbnail>,
    ) {
        // The album data is shared by all the songs.
        // So no need to clone/allocate for eache one.
        // Instead we'll share ownership via Rc.
        let year = Rc::new(year);
        let album = Rc::new(ListSongAlbum::from(album));
        let artists = Rc::new(artists.into_iter().map(Into::into).collect::<Vec<_>>());
        let thumbnails = Rc::new(thumbnails);
        for song in raw_list {
            self.add_raw_album_song(
                song,
                album.clone(),
                year.clone(),
                artists.clone(),
                thumbnails.clone(),
            );
        }
    }
    pub fn append_raw_playlist_items(&mut self, raw_list: Vec<PlaylistItem>) {
        for song in raw_list {
            self.add_raw_playlist_item(song);
        }
    }
    pub fn append_raw_search_result_songs(&mut self, raw_list: Vec<SearchResultSong>) {
        for song in raw_list {
            self.add_raw_search_result_song(song);
        }
    }
    pub fn add_raw_album_song(
        &mut self,
        song: AlbumSong,
        album: Rc<ListSongAlbum>,
        year: Rc<String>,
        artists: Rc<Vec<ListSongArtist>>,
        thumbnails: Rc<Vec<Thumbnail>>,
    ) -> ListSongID {
        let id = self.create_next_id();
        let AlbumSong {
            video_id,
            track_no,
            duration,
            plays,
            title,
            explicit,
            ..
        } = song;
        self.list.push(ListSong {
            download_status: DownloadStatus::None,
            id,
            year: Some(year),
            artists: MaybeRc::Rc(artists),
            album: Some(MaybeRc::Rc(album)),
            actual_duration: None,
            video_id,
            track_no: Some(track_no),
            plays,
            title,
            explicit: Some(explicit),
            duration_string: duration,
            thumbnails: MaybeRc::Rc(thumbnails),
            album_art: Default::default(),
        });
        id
    }
    pub fn add_raw_search_result_song(&mut self, song: SearchResultSong) -> ListSongID {
        let id = self.create_next_id();
        let SearchResultSong {
            title,
            artist,
            album,
            duration,
            plays: _,
            explicit,
            video_id,
            thumbnails,
            ..
        } = song;
        self.list.push(ListSong {
            download_status: DownloadStatus::None,
            id,
            year: None,
            artists: MaybeRc::Owned(vec![ListSongArtist {
                name: artist,
                id: None,
            }]),
            album: album.map(Into::into).map(MaybeRc::Owned),
            actual_duration: None,
            video_id,
            track_no: None,
            plays: String::new(),
            title,
            explicit: Some(explicit),
            duration_string: duration,
            thumbnails: MaybeRc::Owned(thumbnails),
            album_art: Default::default(),
        });
        id
    }
    fn add_raw_playlist_item(&mut self, item: PlaylistItem) -> ListSongID {
        let id = self.create_next_id();
        let (track_no, title, video_id, duration, artists, album, thumbnails, explicit) = match item
        {
            PlaylistItem::Song(PlaylistSong {
                video_id,
                album,
                duration,
                title,
                artists,
                thumbnails,
                track_no,
                explicit,
                ..
            }) => (
                track_no,
                title,
                video_id,
                duration,
                artists.into_iter().map(Into::into).collect(),
                Some(album.into()),
                thumbnails,
                Some(explicit),
            ),
            PlaylistItem::Video(PlaylistVideo {
                video_id,
                duration,
                title,
                thumbnails,
                track_no,
                ..
            }) => (
                track_no,
                title,
                video_id,
                duration,
                vec![],
                None,
                thumbnails,
                None,
            ),
            // Episode has no video id, so we can't currently handle it as a ListSong...
            PlaylistItem::Episode(PlaylistEpisode { .. }) => unimplemented!(
                "One of the playlist items is a podcast episode, handling these is not currently implemented"
            ),
            PlaylistItem::UploadSong(PlaylistUploadSong {
                video_id,
                duration,
                title,
                artists,
                album,
                thumbnails,
                track_no,
                ..
            }) => (
                track_no,
                title,
                video_id,
                duration,
                artists.into_iter().map(Into::into).collect(),
                album.map(Into::into),
                thumbnails,
                None,
            ),
        };
        self.list.push(ListSong {
            download_status: DownloadStatus::None,
            id,
            year: None,
            artists: MaybeRc::Owned(artists),
            album: album.map(MaybeRc::Owned),
            actual_duration: None,
            video_id,
            track_no: Some(track_no),
            plays: String::new(),
            title,
            explicit,
            duration_string: duration,
            thumbnails: MaybeRc::Owned(thumbnails),
            album_art: Default::default(),
        });
        id
    }
    // Returns the ID of the first song added.
    pub fn push_song_list(&mut self, mut song_list: Vec<ListSong>) -> ListSongID {
        let first_id = self.create_next_id();
        if let Some(song) = song_list.first_mut() {
            song.id = first_id;
        };
        // XXX: Below panics - consider a better option.
        self.list.push(song_list.remove(0));
        for mut song in song_list {
            song.id = self.create_next_id();
            self.list.push(song);
        }
        first_id
    }
    /// Safely deletes the song at index if it exists, and returns it.
    pub fn remove_song_index(&mut self, idx: usize) -> Option<ListSong> {
        // Guard against index out of bounds
        if self.list.len() <= idx {
            return None;
        }
        Some(self.list.remove(idx))
    }
    pub fn create_next_id(&mut self) -> ListSongID {
        let id = self.next_id;
        self.next_id.0 += 1;
        id
    }
    pub fn add_song_thumbnail(&mut self, song_thumbnail: SongThumbnail) {
        // Thumbnail is refcounted since it could be shared by multiple songs on the
        // playlist (even if its a video thumbnail).
        let thumbnail_shared = Rc::new(song_thumbnail);
        for song in &mut self.list {
            if !matches!(song.album_art, AlbumArtState::Downloaded(_))
                && SongThumbnailID::from(&*song) == thumbnail_shared.song_thumbnail_id
            {
                song.album_art = AlbumArtState::Downloaded(thumbnail_shared.clone());
            }
            tracing::info!("Album art updated");
        }
    }
    pub fn set_song_thumbnail_error(&mut self, thumbnail_id: SongThumbnailID<'_>) {
        for song in &mut self.list {
            if !matches!(song.album_art, AlbumArtState::Downloaded(_))
                && SongThumbnailID::from(&*song) == thumbnail_id
            {
                song.album_art = AlbumArtState::Error;
            }
            tracing::info!("Album art updated");
        }
    }
    pub fn get_song_from_idx(&self, idx: usize) -> Option<&ListSong> {
        self.list.get(idx)
    }

}
