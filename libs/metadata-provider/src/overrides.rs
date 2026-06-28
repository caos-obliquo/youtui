use crate::ValidatedMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MetadataOverrides {
    pub entries: HashMap<String, OverrideEntry>,
    pub artist_genres: HashMap<String, OverrideEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OverrideEntry {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub genres: Option<Vec<String>>,
    pub styles: Option<Vec<String>>,
}

impl OverrideEntry {
    fn has_data(&self) -> bool {
        self.album.is_some()
            || self.year.is_some()
            || self.genres.is_some()
            || self.styles.is_some()
    }

    fn to_validated(&self) -> ValidatedMetadata {
        ValidatedMetadata {
            artist: self.artist.clone(),
            album: self.album.clone(),
            year: self.year.clone(),
            track_no: None,
            album_tracks: Vec::new(),
            genres: self.genres.clone().unwrap_or_default(),
            styles: self.styles.clone().unwrap_or_default(),
        }
    }
}

impl MetadataOverrides {
    pub fn load(path: Option<PathBuf>) -> Self {
        let path = match path {
            Some(p) => p,
            None => return Self::default(),
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse metadata_overrides.json: {}", e);
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save_to(&self, path: &PathBuf) {
        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(path, &content) {
                    tracing::error!("Failed to write metadata_overrides.json: {}", e);
                }
            }
            Err(e) => tracing::error!("Failed to serialize metadata_overrides: {}", e),
        }
    }

    pub fn resolve(&self, artist: &str, title: &str) -> Option<ValidatedMetadata> {
        let mut keys = vec![format!(
            "{}:{}",
            artist.to_lowercase(),
            title.to_lowercase()
        )];
        let norm = crate::util::norm_for_lfm(title);
        if norm != *title {
            keys.push(format!("{}:{}", artist.to_lowercase(), norm.to_lowercase()));
        }
        for key in &keys {
            if let Some(entry) = self.entries.get(key) {
                if entry.has_data() {
                    return Some(entry.to_validated());
                }
            }
        }
        let artist_key = artist.to_lowercase();
        if let Some(entry) = self.artist_genres.get(&artist_key) {
            if entry.has_data() {
                return Some(entry.to_validated());
            }
        }
        None
    }

    pub fn set(&mut self, artist: &str, title: &str, meta: &ValidatedMetadata) {
        let key = format!("{}:{}", artist.to_lowercase(), title.to_lowercase());
        let entry = OverrideEntry {
            artist: meta.artist.clone(),
            album: meta.album.clone(),
            year: meta.year.clone(),
            genres: if meta.genres.is_empty() {
                None
            } else {
                Some(meta.genres.clone())
            },
            styles: if meta.styles.is_empty() {
                None
            } else {
                Some(meta.styles.clone())
            },
        };
        self.entries.insert(key, entry);

        let has_genre_data = !meta.genres.is_empty() || !meta.styles.is_empty();
        if has_genre_data {
            let artist_entry = OverrideEntry {
                genres: Some(meta.genres.clone()),
                styles: Some(meta.styles.clone()),
                ..Default::default()
            };
            self.artist_genres
                .insert(artist.to_lowercase(), artist_entry);
        }
    }
}
