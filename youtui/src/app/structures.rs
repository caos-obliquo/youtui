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
    AlbumID, ArtistChannelID, Explicit, LikeStatus, UploadAlbumID, UploadArtistID, VideoID, YoutubeID,
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
pub struct ListSongID(pub usize);

// As this is a simple wrapper type we implement Copy for ease of handling
#[derive(Clone, PartialEq, Copy, Debug, Default, PartialOrd, Serialize, Deserialize)]
pub struct Percentage(pub u8);

#[derive(Clone, PartialEq, Copy, Debug, Default, Serialize, Deserialize)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

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
    pub start_offset: Option<Duration>,
    pub year: Option<Rc<String>>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
    pub album_art: AlbumArtState,
    pub artists: MaybeRc<Vec<ListSongArtist>>,
    pub thumbnails: MaybeRc<Vec<Thumbnail>>,
    pub album: Option<MaybeRc<ListSongAlbum>>,
    pub like_status: LikeStatus,
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
        let name = normalize_artist_name(&name);
        Self {
            name,
            id: id.map(ArtistOrUploadArtistID::Artist),
        }
    }
}

impl From<ParsedUploadArtist> for ListSongArtist {
    fn from(value: ParsedUploadArtist) -> Self {
        let ParsedUploadArtist { name, id } = value;
        let name = normalize_artist_name(&name);
        Self {
            name,
            id: id.map(ArtistOrUploadArtistID::UploadArtist),
        }
    }
}

pub fn normalize_artist_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() { return String::new(); }
    // Strip leading bracket prefix like "[hate5six] Sunami" -> "Sunami"
    let trimmed = {
        let s = trimmed;
        if s.starts_with('[') {
            if let Some(close) = s.find(']') {
                let after = s[close + 1..].trim();
                if !after.is_empty() { after } else { s }
            } else { s }
        } else { s }
    };
    // Strip YouTube " - Topic" suffix (auto-generated topic channels)
    let trimmed = {
        let s = trimmed;
        let lower = s.to_lowercase();
        if lower.ends_with(" - topic") {
            s[..s.len() - 8].trim()
        } else {
            s
        }
    };
    // Strip Discogs disambiguation suffix like " (2)", " (3)" etc.
    let trimmed = {
        let s = trimmed;
        if let Some(paren) = s.rfind(" (") {
            let inner = s[paren + 2..].trim_end_matches(')');
            if inner.chars().all(|c| c.is_ascii_digit()) {
                s[..paren].trim()
            } else {
                s
            }
        } else {
            s
        }
    };
    // Respect intentional lowercase names (e.g. "data da morte" should not become "Data da morte")
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return trimmed.to_string();
    };
    if first.is_lowercase() {
        return trimmed.to_string();
    }
    first.to_uppercase().to_string() + chars.as_str()
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
    LikeStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ListStatus {
    New,
    Loading,
    InProgress,
    Loaded,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum DownloadStatus {
    #[default]
    None,
    Queued,
    Downloading(Percentage),
    #[serde(skip)]
    Downloaded(Arc<InMemSong>),
    Failed,
    Retrying { times_retried: usize },
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum AudioQuality {
    #[default]
    Best,
    High,
    Medium,
    Low,
}

impl PlayState {
    pub fn list_icon(&self) -> char {
        match self {
            PlayState::Buffering(_) => '',
            PlayState::NotPlaying => '',
            PlayState::Playing(_) => '',
            PlayState::Paused(_) => '',
            PlayState::Stopped => '',
            PlayState::Error(_) => '',
        }
    }
}

impl DownloadStatus {
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
            ListSongDisplayableField::LikeStatus => match self.like_status {
                LikeStatus::Liked => Cow::Borrowed("\u{2665}"),
                _ => Cow::Borrowed(""),
            },
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
            album_art: AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            start_offset: None,
            artists: MaybeRc::Owned(list_artists),
            thumbnails: MaybeRc::Owned(thumb.unwrap_or_default()),
            album: list_album,
            like_status: LikeStatus::Indifferent,
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
    pub fn append_raw_search_result_songs(&mut self, raw_list: Vec<SearchResultSong>) -> ListSongID {
        let mut last_id = self.create_next_id();
        for song in raw_list {
            last_id = self.add_raw_search_result_song(song);
        }
        last_id
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
            like_status,
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
            album_art: AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            start_offset: None,
            like_status,
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
            plays,
            explicit,
            video_id,
            thumbnails,
            year,
            like_status,
            ..
        } = song;
        self.list.push(ListSong {
            download_status: DownloadStatus::None,
            id,
            year: year.map(std::rc::Rc::new),
            artists: MaybeRc::Owned(vec![ListSongArtist {
                name: normalize_artist_name(&artist),
                id: None,
            }]),
            album: album.map(|a| {
                let mut album: ListSongAlbum = a.into();
                // Strip "YouTube: " prefix from album name (uploader channel)
                let lower = album.name.to_lowercase();
                if lower.starts_with("youtube: ") {
                    album.name = album.name[9..].trim().to_string();
                }
                // Strip " - Topic" suffix from album name (auto-generated topic channel)
                let lower = album.name.to_lowercase();
                if lower.ends_with(" - topic") {
                    album.name = album.name[..album.name.len() - 8].trim().to_string();
                }
                // Strip leading bracket prefix like "[hate5six] Sunami" -> "Sunami"
                if album.name.starts_with('[') {
                    if let Some(close) = album.name.find(']') {
                        let after = album.name[close + 1..].trim().to_string();
                        if !after.is_empty() { album.name = after; }
                    }
                }
                album
            }).map(MaybeRc::Owned),
            actual_duration: None,
            video_id,
            track_no: None,
            plays,
            title,
            explicit: Some(explicit),
            thumbnails: MaybeRc::Owned(thumbnails),
            duration_string: duration,
            album_art: AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            start_offset: None,
            like_status,
        });
        id
    }
    fn add_raw_playlist_item(&mut self, item: PlaylistItem) -> ListSongID {
        let id = self.create_next_id();
        let (track_no, title, video_id, duration, artists, album, thumbnails, explicit, year, like_status) = match item
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
                year,
                like_status,
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
                year,
                like_status,
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
                None,
                LikeStatus::Indifferent,
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
                None,
                LikeStatus::Indifferent,
            ),
        };
        self.list.push(ListSong {
            download_status: DownloadStatus::None,
            id,
            year: year.map(std::rc::Rc::new),
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
            album_art: AlbumArtState::None,
            genres: Vec::new(),
            styles: Vec::new(),
            start_offset: None,
            like_status,
        });
        id
    }
    pub fn push_song_list(&mut self, song_list: Vec<ListSong>) -> ListSongID {
        let first_id = self.create_next_id();
        let mut iter = song_list.into_iter();
        if let Some(mut first) = iter.next() {
            first.id = first_id;
            self.list.push(first);
        }
        for mut song in iter {
            song.id = self.create_next_id();
            self.list.push(song);
        }
        first_id
    }
    pub fn insert_song_list_at(&mut self, song_list: Vec<ListSong>, position: usize) -> ListSongID {
        let pos = position.min(self.list.len());
        let first_id = self.create_next_id();
        for (i, mut song) in song_list.into_iter().enumerate() {
            song.id = if i == 0 { first_id } else { self.create_next_id() };
            self.list.insert(pos + i, song);
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
    pub fn insert_at(&mut self, idx: usize, song: ListSong) {
        let pos = idx.min(self.list.len());
        self.list.insert(pos, song);
    }
    pub fn insert_after(&mut self, idx: usize, song: ListSong) {
        let pos = (idx + 1).min(self.list.len());
        self.list.insert(pos, song);
    }
    pub fn remove_at(&mut self, idx: usize) {
        if idx < self.list.len() {
            self.list.remove(idx);
        }
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

    /// Update year/genres/styles at a specific index (cache enrichment).
    pub fn update_song_at(&mut self, idx: usize, year: Option<Rc<String>>, genres: Vec<String>, styles: Vec<String>) {
        if let Some(song) = self.list.get_mut(idx) {
            if year.is_some() || !genres.is_empty() || !styles.is_empty() {
                song.year = year;
                song.genres = genres;
                song.styles = styles;
            }
        }
    }
    /// Sort the underlying song list in place using the given comparator.
    pub fn sort_list_by<F>(&mut self, compare: F)
    where
        F: FnMut(&ListSong, &ListSong) -> std::cmp::Ordering,
    {
        self.list.sort_by(compare);
    }

}

/// Score-based fuzzy match: returns Some(score) if all query chars appear in target in order.
/// Higher score = better match. Score favors early match position and contiguous runs.
pub fn fuzzy_match(query: &str, target: &str) -> Option<u64> {
    if query.is_empty() {
        return Some(0);
    }
    let query = query.to_lowercase();
    let target = target.to_lowercase();
    let qb = query.as_bytes();
    let tb = target.as_bytes();
    let mut ti = 0;
    let mut score: u64 = 0;
    let mut first_match = None;
    let mut consecutive = 0;
    for &qc in qb {
        while ti < tb.len() && tb[ti] != qc {
            consecutive = 0;
            ti += 1;
        }
        if ti >= tb.len() {
            return None;
        }
        if first_match.is_none() {
            first_match = Some(ti);
        }
        // Bonus for consecutive match
        if consecutive > 0 {
            score += 10;
        }
        score += 1;
        consecutive += 1;
        ti += 1;
    }
    // Prefer matches that start earlier in the target string
    let start_bonus = first_match.map(|s| (tb.len().saturating_sub(s) * 5) as u64).unwrap_or(0);
    Some(score + start_bonus)
}

/// Check if text contains Japanese characters (hiragana, katakana, or kanji)
pub fn has_japanese(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{3040}'..='\u{309F}' | // Hiragana
            '\u{30A0}'..='\u{30FF}' | // Katakana
            '\u{3400}'..='\u{4DBF}' | // CJK Extension A
            '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        )
    })
}

/// Copy text to system clipboard using `wl-copy`.
/// Wayland-only.
pub fn copy_to_clipboard(text: &str) {
    let _ = std::process::Command::new("wl-copy").arg(text).spawn();
}

#[cfg(test)]
mod normalize_tests {
    use super::normalize_artist_name;

    #[test]
    fn norm_already_capitalized() {
        assert_eq!(normalize_artist_name("Metallica"), "Metallica");
    }

    #[test]
    fn norm_lowercase() {
        // All-lowercase names preserved (intentional naming like "data da morte")
        assert_eq!(normalize_artist_name("metallica"), "metallica");
    }

    #[test]
    fn norm_uppercase() {
        assert_eq!(normalize_artist_name("METALLICA"), "METALLICA");
    }

    #[test]
    fn norm_single_char() {
        assert_eq!(normalize_artist_name("a"), "a");
    }

    #[test]
    fn norm_empty() {
        assert_eq!(normalize_artist_name(""), "");
    }

    #[test]
    fn norm_whitespace_padded() {
        // Whitespace stripped, all-lowercase preserved
        assert_eq!(normalize_artist_name("  metallica  "), "metallica");
    }

    #[test]
    fn norm_intentional_lowercase_preserved() {
        assert_eq!(normalize_artist_name("data da morte"), "data da morte");
    }
}

#[cfg(test)]
mod fuzzy_tests {
    use super::fuzzy_match;
    #[test]
    fn test_exact_match() {
        assert!(fuzzy_match("hello", "hello").is_some());
    }
    #[test]
    fn test_subsequence_match() {
        assert!(fuzzy_match("hlo", "hello").is_some());
    }
    #[test]
    fn test_no_match() {
        assert!(fuzzy_match("xyz", "hello").is_none());
    }
    #[test]
    fn test_case_insensitive() {
        assert!(fuzzy_match("HELLO", "hello").is_some());
    }
    #[test]
    fn test_empty_query() {
        assert!(fuzzy_match("", "anything").is_some());
    }
    #[test]
    fn test_early_position_higher_score() {
        let s1 = fuzzy_match("ab", "abc").unwrap();
        let s2 = fuzzy_match("ab", "xab").unwrap();
        assert!(s1 > s2, "earlier match should score higher");
    }
}
