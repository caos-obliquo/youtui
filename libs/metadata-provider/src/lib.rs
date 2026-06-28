mod discogs;
mod genius;
pub mod genre_map;
mod lastfm_album;
mod lastfm_track;
mod metal_api;
mod musicbrainz;
pub mod overrides;
pub mod util;

pub use discogs::DiscogsProvider;
pub use genius::GeniusProvider;
pub use lastfm_album::AlbumSearchProvider;
pub use lastfm_track::TrackSearchProvider;
pub use metal_api::MetalApiProvider;
pub use musicbrainz::MusicBrainzProvider;
pub use validated_metadata::{AlbumTrack, ValidatedMetadata};
mod validated_metadata;

use futures::future::BoxFuture;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Mutex;

pub trait MetadataProvider: Send + Sync {
    fn priority(&self) -> u8;
    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        album: Option<&'a str>,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>>;
}

pub struct MetadataRegistry {
    providers: Vec<Box<dyn MetadataProvider>>,
    http_client: reqwest::Client,
    cache: Mutex<LruCache<String, ValidatedMetadata>>,
    overrides: Mutex<overrides::MetadataOverrides>,
    overrides_path: Option<PathBuf>,
    cache_path: Option<PathBuf>,
}

impl MetadataRegistry {
    pub fn new(
        http_client: reqwest::Client,
        lastfm_key: Option<String>,
        discogs_token: Option<String>,
        genius_token: Option<String>,
        overrides_path: Option<PathBuf>,
        cache_path: Option<PathBuf>,
    ) -> Self {
        let mut providers: Vec<Box<dyn MetadataProvider>> = vec![
            Box::new(MetalApiProvider::new()),
            Box::new(AlbumSearchProvider::new(lastfm_key.clone())),
            Box::new(TrackSearchProvider::new(lastfm_key.clone())),
            Box::new(DiscogsProvider::new(discogs_token.clone())),
            Box::new(GeniusProvider::new(genius_token.clone())),
            Box::new(MusicBrainzProvider::new()),
        ];
        providers.sort_by_key(|p| p.priority());
        let reg = Self {
            providers,
            http_client,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(200).unwrap())),
            overrides: Mutex::new(overrides::MetadataOverrides::load(overrides_path.clone())),
            overrides_path,
            cache_path,
        };
        reg.load_cache();
        reg
    }

    /// Score how closely a provider result matches the search query
    fn score_result(meta: &ValidatedMetadata, artist: &str, title: &str) -> i32 {
        let mut score = 0;
        let mut artist_ok = false;
        // Artist match is CRITICAL - heavy weight
        if let Some(ref a) = meta.artist {
            let a_low = util::norm_for_lfm(a).to_lowercase();
            let art_low = util::norm_for_lfm(artist).to_lowercase();
            if a_low == art_low {
                score += 50;
                artist_ok = true;
            } else if a_low.contains(&art_low) || art_low.contains(&a_low) {
                score += 10;
            }
        }
        // album_tracks present: +100 if artist matches (enables splitting),
        // +30 otherwise (tracklist without correct artist is low-confidence)
        if !meta.album_tracks.is_empty() {
            if artist_ok {
                score += 100;
            } else {
                score += 30;
            }
        }
        // album name present: +10
        if meta.album.is_some() {
            score += 10;
        }
        // year present: +5
        if meta.year.is_some() {
            score += 5;
        }
        // Album name matches or contains search title: +15 (strong signal)
        if let Some(ref a) = meta.album {
            let a_low = a.to_lowercase();
            let t_low = title.to_lowercase();
            if a_low == t_low {
                score += 15;
            } else if a_low.contains(&t_low) || t_low.contains(&a_low) {
                score += 7;
            }
            // Fuzzy: & vs "and" normalization
            let a_norm = a_low.replace(" & ", " and ").replace("&", "and");
            let t_norm = t_low.replace(" & ", " and ").replace("&", "and");
            if a_norm == t_norm {
                score += 10;
            }
        }
        // More tracks = more complete: +2 per track (up to +30)
        score += (meta.album_tracks.len() as i32).min(15) * 2;
        // PENALTY: if artist IS present but doesn't match at all - wrong band
        if !artist_ok {
            if let Some(ref a) = meta.artist {
                let a_low = util::norm_for_lfm(a).to_lowercase();
                let art_low = util::norm_for_lfm(artist).to_lowercase();
                if !a_low.contains(&art_low) && !art_low.contains(&a_low) {
                    score -= 500;
                }
            }
        }
        score
    }

    pub async fn resolve(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
    ) -> Result<ValidatedMetadata, anyhow::Error> {
        let cache_key = format!(
            "{}::{}",
            util::norm_for_lfm(&artist.to_lowercase()),
            util::norm_for_lfm(&title.to_lowercase()),
        );
        // Check user overrides first (persisted edits take priority)
        if let Some(overridden) = self.overrides.lock().unwrap().resolve(artist, title) {
            tracing::info!(
                "Metadata resolved by user override for {} - {}",
                artist,
                title
            );
            return Ok(overridden);
        }
        if let Some(cached) = self.cache.lock().unwrap().get(&cache_key) {
            if cached.album.is_some() || cached.year.is_some() {
                return Ok(cached.clone());
            }
        }

        // Try ALL providers, collect results, pick the best-scoring one
        let mut best: Option<(i32, ValidatedMetadata, u8)> = None;
        for provider in &self.providers {
            if let Some(meta) = provider
                .lookup(artist, title, album, &self.http_client)
                .await
            {
                let score = Self::score_result(&meta, artist, title);
                tracing::debug!(
                    "Provider priority {} scored {} for {} - {} (album: {:?}, tracks: {})",
                    provider.priority(),
                    score,
                    artist,
                    title,
                    meta.album.as_deref().unwrap_or("none"),
                    meta.album_tracks.len(),
                );
                if score > 0 {
                    let is_better = match &best {
                        None => true,
                        Some((best_score, _, _)) => score > *best_score,
                    };
                    if is_better {
                        best = Some((score, meta, provider.priority()));
                    }
                }
            }
        }

        if let Some((score, mut meta, priority)) = best {
            tracing::info!(
                "Metadata resolved by provider priority {} (score {}) for {} - {}",
                priority,
                score,
                artist,
                title
            );
            if !meta.genres.is_empty() {
                meta.genres = crate::genre_map::normalize_genres(&meta.genres);
            }
            if !meta.styles.is_empty() {
                meta.styles = crate::genre_map::normalize_genres(&meta.styles);
            }
            // Only cache meaningful results (score >= 20: album match + artist or better)
            if score >= 20 {
                self.cache.lock().unwrap().put(cache_key, meta.clone());
                self.save_cache();
            }
            return Ok(meta);
        }

        Ok(ValidatedMetadata::default())
    }

    fn cache_file_path(&self) -> Option<PathBuf> {
        self.cache_path
            .as_ref()
            .map(|p| p.join("metadata_cache.json"))
    }

    fn load_cache(&self) {
        let Some(path) = self.cache_file_path() else {
            return;
        };
        if !path.exists() {
            return;
        }
        match std::fs::read_to_string(&path) {
            Ok(data) => match serde_json::from_str::<Vec<(String, ValidatedMetadata)>>(&data) {
                Ok(entries) => {
                    let mut cache = self.cache.lock().unwrap();
                    for (key, meta) in entries {
                        cache.put(key, meta);
                    }
                    tracing::info!("Loaded {} entries from metadata cache", cache.len());
                }
                Err(e) => tracing::warn!("Failed to parse metadata cache: {}", e),
            },
            Err(e) => tracing::warn!("Failed to read metadata cache: {}", e),
        }
    }

    fn save_cache(&self) {
        let Some(path) = self.cache_file_path() else {
            return;
        };
        let entries: Vec<(String, ValidatedMetadata)> = {
            let cache = self.cache.lock().unwrap();
            cache.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };
        if entries.is_empty() {
            return;
        }
        match serde_json::to_string_pretty(&entries) {
            Ok(json) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                // Atomic write via temp file
                let tmp = path.with_extension("json.tmp");
                match std::fs::write(&tmp, &json) {
                    Ok(_) => {
                        let _ = std::fs::rename(&tmp, &path);
                    }
                    Err(e) => tracing::warn!("Failed to write metadata cache: {}", e),
                }
            }
            Err(e) => tracing::warn!("Failed to serialize metadata cache: {}", e),
        }
    }

    /// Cache-only lookup - no HTTP, no provider resolution.
    /// Returns None if not in LRU cache or if result is sparse (no album/year).
    pub fn lookup_cache(&self, key: &str) -> Option<ValidatedMetadata> {
        self.cache
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .filter(|m| m.album.is_some() || m.year.is_some())
    }

    pub fn save_override(&self, artist: &str, title: &str, meta: &ValidatedMetadata) {
        let mut overrides = self.overrides.lock().unwrap();
        overrides.set(artist, title, meta);
        if let Some(ref path) = self.overrides_path {
            overrides.save_to(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overrides::MetadataOverrides;

    fn make_meta(
        artist: Option<&str>,
        album: Option<&str>,
        year: Option<&str>,
        tracks: usize,
    ) -> ValidatedMetadata {
        ValidatedMetadata {
            artist: artist.map(String::from),
            album: album.map(String::from),
            year: year.map(String::from),
            album_tracks: (0..tracks)
                .map(|i| AlbumTrack {
                    title: format!("Track {}", i + 1),
                    duration_secs: 100.0,
                    artist: None,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn score_empty_metadata() {
        let meta = ValidatedMetadata::default();
        assert_eq!(MetadataRegistry::score_result(&meta, "Artist", "Title"), 0);
    }

    #[test]
    fn score_album_tracks_only() {
        let meta = make_meta(None, None, None, 3);
        // artist missing → tracklist +30, 3 tracks = 6 → 36
        assert_eq!(MetadataRegistry::score_result(&meta, "Artist", "Title"), 36);
    }

    #[test]
    fn score_exact_artist_and_album_title() {
        let meta = make_meta(Some("Metallica"), Some("Master of Puppets"), None, 0);
        let score = MetadataRegistry::score_result(&meta, "Metallica", "Master of Puppets");
        // album(10) + artist_exact(50) + album_title(15) + and_norm(10) = 85
        assert_eq!(score, 85);
    }

    #[test]
    fn score_artist_contains_bonus() {
        let meta = make_meta(Some("The Beatles Band"), None, None, 0);
        let score = MetadataRegistry::score_result(&meta, "Beatles", "Title");
        assert_eq!(score, 10); // contains match only (now +10)
    }

    #[test]
    fn score_and_normalization_boost() {
        let meta = make_meta(Some("Band"), Some("Rock & Roll"), None, 0);
        let score = MetadataRegistry::score_result(&meta, "Band", "Rock and Roll");
        // album(10) + artist_exact(50) + and_norm(10) = 70
        assert_eq!(score, 70);
    }

    #[test]
    fn score_year_bonus() {
        let meta = make_meta(None, None, Some("1986"), 0);
        let score = MetadataRegistry::score_result(&meta, "Artist", "Title");
        assert_eq!(score, 5);
    }

    #[test]
    fn score_track_count_capped() {
        let meta = make_meta(None, None, None, 20);
        // artist missing → tracklist +30, min(15,20)*2 = 30 → 60
        assert_eq!(MetadataRegistry::score_result(&meta, "Artist", "Title"), 60);
    }

    #[test]
    fn score_complete_metadata() {
        let meta = make_meta(
            Some("Metallica"),
            Some("Master of Puppets"),
            Some("1986"),
            8,
        );
        let score = MetadataRegistry::score_result(&meta, "Metallica", "Master of Puppets");
        // tracks(100) + album(10) + year(5) + artist_exact(50) + album_title(15) +
        // and_norm(10) + 8*2 = 206
        assert_eq!(score, 206);
    }

    #[test]
    fn score_album_contains_title() {
        let meta = make_meta(None, Some("The Complete Master of Puppets Live"), None, 0);
        let score = MetadataRegistry::score_result(&meta, "Any", "Master of Puppets");
        assert_eq!(score, 10 + 7); // album(10) + contains(7)
    }

    // --- resolve() integration tests ---

    struct MockProvider {
        priority_val: u8,
        result: Option<ValidatedMetadata>,
    }

    impl MetadataProvider for MockProvider {
        fn priority(&self) -> u8 {
            self.priority_val
        }

        fn lookup<'a>(
            &'a self,
            _artist: &'a str,
            _title: &'a str,
            _album: Option<&'a str>,
            _client: &'a reqwest::Client,
        ) -> futures::future::BoxFuture<'a, Option<ValidatedMetadata>> {
            let result = self.result.clone();
            Box::pin(async move { result })
        }
    }

    fn make_registry(providers: Vec<Box<dyn MetadataProvider>>) -> MetadataRegistry {
        MetadataRegistry {
            providers,
            http_client: reqwest::Client::new(),
            cache: std::sync::Mutex::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(200).unwrap(),
            )),
            overrides: std::sync::Mutex::new(MetadataOverrides::default()),
            overrides_path: None,
            cache_path: None,
        }
    }

    #[test]
    fn resolve_selects_highest_scored_provider() {
        // Provider 1: tracks + album + exact artist → ~195
        let p1 = MockProvider {
            priority_val: 1,
            result: Some(ValidatedMetadata {
                artist: Some("Metallica".into()),
                album: Some("Master of Puppets".into()),
                year: Some("1986".into()),
                album_tracks: vec![
                    AlbumTrack {
                        title: "Battery".into(),
                        duration_secs: 500.0,
                        artist: None,
                    },
                    AlbumTrack {
                        title: "Master of Puppets".into(),
                        duration_secs: 515.0,
                        artist: None,
                    },
                ],
                ..Default::default()
            }),
        };
        // Provider 2: album but no tracks, no artist → ~17
        let p2 = MockProvider {
            priority_val: 2,
            result: Some(ValidatedMetadata {
                artist: None,
                album: Some("Master of Puppets".into()),
                ..Default::default()
            }),
        };
        // Provider 3: wrong artist penalty → ~ -490
        let p3 = MockProvider {
            priority_val: 3,
            result: Some(ValidatedMetadata {
                artist: Some("Megadeth".into()),
                album: Some("Rust in Peace".into()),
                ..Default::default()
            }),
        };

        let reg = make_registry(vec![Box::new(p1), Box::new(p2), Box::new(p3)]);
        let result =
            futures::executor::block_on(reg.resolve("Metallica", "Master of Puppets", None))
                .unwrap();

        assert_eq!(result.artist, Some("Metallica".to_string()));
        assert_eq!(result.album, Some("Master of Puppets".to_string()));
        assert_eq!(result.year, Some("1986".to_string()));
        assert_eq!(result.album_tracks.len(), 2);
    }

    #[test]
    fn resolve_returns_default_when_no_match() {
        // Provider returns metadata that doesn't match at all
        let p = MockProvider {
            priority_val: 1,
            result: Some(ValidatedMetadata {
                artist: Some("Megadeth".into()),
                album: Some("Rust in Peace".into()),
                ..Default::default()
            }),
        };
        let reg = make_registry(vec![Box::new(p)]);
        let result =
            futures::executor::block_on(reg.resolve("Metallica", "Master of Puppets", None))
                .unwrap();

        // Default: all fields None/empty
        assert_eq!(result.artist, None);
        assert_eq!(result.album, None);
        assert_eq!(result.year, None);
        assert!(result.album_tracks.is_empty());
    }

    #[test]
    fn resolve_uses_album_param_for_better_match() {
        // Provider matches only when album is given
        let p = MockProvider {
            priority_val: 1,
            result: Some(ValidatedMetadata {
                artist: Some("Band".into()),
                album: Some("The Album".into()),
                year: Some("2020".into()),
                album_tracks: vec![AlbumTrack {
                    title: "Song".into(),
                    duration_secs: 200.0,
                    artist: None,
                }],
                ..Default::default()
            }),
        };
        let reg = make_registry(vec![Box::new(p)]);
        // Resolve with album param
        let result =
            futures::executor::block_on(reg.resolve("Band", "Song", Some("The Album"))).unwrap();
        assert_eq!(result.artist, Some("Band".to_string()));
        assert_eq!(result.album, Some("The Album".to_string()));
        assert_eq!(result.album_tracks.len(), 1);

        // Resolve without album param also works (provider always returns same)
        let result2 = futures::executor::block_on(reg.resolve("Band", "Song", None)).unwrap();
        assert_eq!(result2.album, Some("The Album".to_string()));
    }
}
