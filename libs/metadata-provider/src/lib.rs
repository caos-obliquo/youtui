mod lastfm_album;
mod lastfm_track;
mod discogs;
mod genius;
mod musicbrainz;
mod metal_api;
pub mod genre_map;
pub mod util;
pub mod overrides;

pub use lastfm_album::AlbumSearchProvider;
pub use lastfm_track::TrackSearchProvider;
pub use discogs::DiscogsProvider;
pub use genius::GeniusProvider;
pub use musicbrainz::MusicBrainzProvider;
pub use metal_api::MetalApiProvider;

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
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>>;
}

pub struct MetadataRegistry {
    providers: Vec<Box<dyn MetadataProvider>>,
    http_client: reqwest::Client,
    cache: Mutex<LruCache<String, ValidatedMetadata>>,
    overrides: Mutex<overrides::MetadataOverrides>,
    overrides_path: Option<PathBuf>,
}

impl MetadataRegistry {
    pub fn new(
        http_client: reqwest::Client,
        lastfm_key: Option<String>,
        discogs_token: Option<String>,
        genius_token: Option<String>,
        overrides_path: Option<PathBuf>,
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
        Self {
            providers,
            http_client,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(200).unwrap())),
            overrides: Mutex::new(overrides::MetadataOverrides::load(overrides_path.clone())),
            overrides_path,
        }
    }

    /// Score how closely a provider result matches the search query
    fn score_result(meta: &ValidatedMetadata, artist: &str, title: &str) -> i32 {
        let mut score = 0;
        let mut artist_ok = false;
        // Artist match is CRITICAL — heavy weight
        if let Some(ref a) = meta.artist {
            let a_low = util::norm_for_lfm(a).to_lowercase();
            let art_low = util::norm_for_lfm(artist).to_lowercase();
            if a_low == art_low { score += 50; artist_ok = true; }
            else if a_low.contains(&art_low) || art_low.contains(&a_low) { score += 10; }
        }
        // album_tracks present: +100 (most important — enables splitting)
        if !meta.album_tracks.is_empty() { score += 100; }
        // album name present: +10
        if meta.album.is_some() { score += 10; }
        // year present: +5
        if meta.year.is_some() { score += 5; }
        // Album name matches or contains search title: +15 (strong signal)
        if let Some(ref a) = meta.album {
            let a_low = a.to_lowercase();
            let t_low = title.to_lowercase();
            if a_low == t_low { score += 15; }
            else if a_low.contains(&t_low) || t_low.contains(&a_low) { score += 7; }
            // Fuzzy: & vs "and" normalization
            let a_norm = a_low.replace(" & ", " and ").replace("&", "and");
            let t_norm = t_low.replace(" & ", " and ").replace("&", "and");
            if a_norm == t_norm { score += 10; }
        }
        // More tracks = more complete: +2 per track (up to +30)
        score += (meta.album_tracks.len() as i32).min(15) * 2;
        // PENALTY: if artist IS present but doesn't match at all — wrong band
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
    ) -> Result<ValidatedMetadata, anyhow::Error> {
        let cache_key = format!("{}::{}",
            util::norm_for_lfm(&artist.to_lowercase()),
            util::norm_for_lfm(&title.to_lowercase()),
        );
        // Check user overrides first (persisted edits take priority)
        if let Some(overridden) = self.overrides.lock().unwrap().resolve(artist, title) {
            tracing::info!("Metadata resolved by user override for {} - {}", artist, title);
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
            if let Some(meta) = provider.lookup(artist, title, &self.http_client).await {
                let score = Self::score_result(&meta, artist, title);
                tracing::debug!(
                    "Provider priority {} scored {} for {} - {} (album: {:?}, tracks: {})",
                    provider.priority(), score, artist, title,
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

        if let Some((_, mut meta, priority)) = best {
            tracing::info!(
                "Metadata resolved by provider priority {} (score {}) for {} - {}",
                priority, Self::score_result(&meta, artist, title), artist, title
            );
            if !meta.genres.is_empty() {
                meta.genres = crate::genre_map::normalize_genres(&meta.genres);
            }
            if !meta.styles.is_empty() {
                meta.styles = crate::genre_map::normalize_genres(&meta.styles);
            }
            self.cache.lock().unwrap().put(cache_key, meta.clone());
            return Ok(meta);
        }

        Ok(ValidatedMetadata::default())
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

    fn make_meta(artist: Option<&str>, album: Option<&str>, year: Option<&str>, tracks: usize) -> ValidatedMetadata {
        ValidatedMetadata {
            artist: artist.map(String::from),
            album: album.map(String::from),
            year: year.map(String::from),
            album_tracks: (0..tracks).map(|i| AlbumTrack {
                title: format!("Track {}", i + 1),
                duration_secs: 100.0,
            }).collect(),
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
        // 100 (tracks) + 3*2 = 106
        assert_eq!(MetadataRegistry::score_result(&meta, "Artist", "Title"), 106);
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
        // 100 (tracks) + min(15,20)*2 = 130
        assert_eq!(MetadataRegistry::score_result(&meta, "Artist", "Title"), 130);
    }

    #[test]
    fn score_complete_metadata() {
        let meta = make_meta(Some("Metallica"), Some("Master of Puppets"), Some("1986"), 8);
        let score = MetadataRegistry::score_result(&meta, "Metallica", "Master of Puppets");
        // tracks(100) + album(10) + year(5) + artist_exact(50) + album_title(15) + and_norm(10) + 8*2 = 206
        assert_eq!(score, 206);
    }

    #[test]
    fn score_album_contains_title() {
        let meta = make_meta(None, Some("The Complete Master of Puppets Live"), None, 0);
        let score = MetadataRegistry::score_result(&meta, "Any", "Master of Puppets");
        assert_eq!(score, 10 + 7); // album(10) + contains(7)
    }
}
