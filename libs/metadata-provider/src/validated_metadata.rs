#[derive(Debug, Default, Clone, PartialEq)]
pub struct ValidatedMetadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub track_no: Option<usize>,
    pub album_tracks: Vec<AlbumTrack>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumTrack {
    pub title: String,
    pub duration_secs: f64,
}
