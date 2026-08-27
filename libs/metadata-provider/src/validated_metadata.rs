use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedMetadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub track_no: Option<usize>,
    pub album_tracks: Vec<AlbumTrack>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
    #[serde(default)]
    pub subgenres: Vec<String>,
    #[serde(default)]
    pub genre_paths: Vec<(String, String)>,
    #[serde(default)]
    pub descriptors: Vec<String>,
    pub musicbrainz_release_group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumTrack {
    pub title: String,
    pub duration_secs: f64,
    pub artist: Option<String>,
}
