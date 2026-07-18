mod lastfm_album;
mod lastfm_track;
mod discogs;
mod genius;
pub mod musicbrainz;
mod metal_api;
pub mod listenbrainz;
mod libre_fm;
mod merge;
pub mod genre_map;
pub mod util;
pub mod overrides;

pub use lastfm_album::AlbumSearchProvider;
pub use lastfm_track::TrackSearchProvider;
pub use discogs::DiscogsProvider;
pub use genius::GeniusProvider;
pub use musicbrainz::MusicBrainzProvider;
pub use metal_api::MetalApiProvider;
pub use listenbrainz::ListenBrainzProvider;
pub use libre_fm::LibreFMProvider;

pub use validated_metadata::{AlbumTrack, ValidatedMetadata};
mod validated_metadata;

use futures::future::BoxFuture;
use lru::LruCache;
use metadata_cache_sqlite::SqliteCache;
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
    pub providers: Vec<Box<dyn MetadataProvider>>,
    pub http_client: reqwest::Client,
    pub cache: Mutex<LruCache<String, ValidatedMetadata>>,
    pub overrides: Mutex<overrides::MetadataOverrides>,
    pub overrides_path: Option<PathBuf>,
    pub cache_path: Option<PathBuf>,
    pub sqlite_cache: Option<Mutex<SqliteCache>>,
}

impl MetadataRegistry {
    pub fn new(
        http_client: reqwest::Client,
        lastfm_key: Option<String>,
        discogs_token: Option<String>,
        genius_token: Option<String>,
        listenbrainz_token: Option<String>,
        musicbrainz_bearer_token: Option<String>,
        librefm_key: Option<String>,
        overrides_path: Option<PathBuf>,
        cache_path: Option<PathBuf>,
        sqlite_path: Option<PathBuf>,
    ) -> Self {
        let client_id = std::env::var("MUSICBRAINZ_CLIENT_ID").ok();
        let client_secret = std::env::var("MUSICBRAINZ_CLIENT_SECRET").ok();
        let mut providers: Vec<Box<dyn MetadataProvider>> = vec![
            Box::new(MetalApiProvider::new()),
            Box::new(AlbumSearchProvider::new(lastfm_key.clone())),
            Box::new(TrackSearchProvider::new(lastfm_key.clone())),
            Box::new(DiscogsProvider::new(discogs_token.clone())),
            Box::new(GeniusProvider::new(genius_token.clone())),
            Box::new(MusicBrainzProvider::new(
                client_id,
                client_secret,
                musicbrainz_bearer_token.clone(),
            )),
        ];
        if let Some(ref token) = listenbrainz_token {
            if !token.is_empty() {
                providers.push(Box::new(ListenBrainzProvider::new(token.clone())));
            }
        }
        if let Some(ref key) = librefm_key {
            if !key.is_empty() {
                providers.push(Box::new(LibreFMProvider::new(Some(key.clone()))));
            }
        }
        providers.sort_by_key(|p| p.priority());
        let sqlite_cache = sqlite_path.and_then(|path| {
            match SqliteCache::open(&path) {
                Ok(cache) => {
                    tracing::info!("Opened SQLite metadata cache at {}", path.display());
                    Some(Mutex::new(cache))
                }
                Err(e) => {
                    tracing::warn!("Failed to open SQLite metadata cache: {}", e);
                    None
                }
            }
        });
        let reg = Self {
            providers,
            http_client,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(200).unwrap())),
            overrides: Mutex::new(overrides::MetadataOverrides::load(overrides_path.clone())),
            overrides_path,
            cache_path,
            sqlite_cache,
        };
        reg.init_cache();
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
            if a_low == art_low { score += 50; artist_ok = true; }
            else if a_low.contains(&art_low) || art_low.contains(&a_low) { score += 10; }
        }
        // album_tracks present: +100 if artist matches (enables splitting),
        // +80 otherwise (tracklist without correct artist is reasonable)
        if !meta.album_tracks.is_empty() {
            if artist_ok { score += 100; }
            else { score += 80; }
        }
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
        // More tracks = more complete: +1 per track (up to +10)
        score += (meta.album_tracks.len() as i32).min(10) * 1;
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

        // Check SQLite cache on LRU miss
        if let Some(ref sqlite) = self.sqlite_cache {
            if let Ok(cache) = sqlite.lock() {
                if let Some(sqlite_meta) = cache.get(&cache_key) {
                    let domain_meta = domain_meta_from_sqlite(&sqlite_meta);
                    if domain_meta.album.is_some() || domain_meta.year.is_some() {
                        tracing::info!("Metadata resolved from SQLite cache for {} - {}", artist, title);
                        // Populate LRU
                        self.cache.lock().unwrap().put(cache_key.clone(), domain_meta.clone());
                        return Ok(domain_meta);
                    }
                }
            }
        }

        // Try ALL providers, collect results, pick the best-scoring one
        let mut best: Option<(i32, ValidatedMetadata, u8)> = None;
        let mut all_results: Vec<(i32, ValidatedMetadata, u8)> = Vec::new();
        for provider in &self.providers {
            if let Some(meta) = provider.lookup(artist, title, album, &self.http_client).await {
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
                        best = Some((score, meta.clone(), provider.priority()));
                    }
                    all_results.push((score, meta, provider.priority()));
                }
            }
        }

        if let Some((score, mut meta, priority)) = best {
            tracing::info!(
                "Metadata resolved by provider priority {} (score {}) for {} - {}",
                priority, score, artist, title
            );
            if all_results.len() > 1 {
                if let Some(merged_year) = merge::merge_year(&all_results) {
                    meta.year = Some(merged_year);
                }
                let (merged_genres, merged_styles) = merge::weighted_merge_genres(&all_results);
                if !merged_genres.is_empty() {
                    meta.genres = merged_genres;
                }
                if !merged_styles.is_empty() {
                    meta.styles = merged_styles;
                }
            } else {
                if !meta.genres.is_empty() {
                    meta.genres = crate::genre_map::normalize_genres(&meta.genres);
                }
                if !meta.styles.is_empty() {
                    meta.styles = crate::genre_map::normalize_genres(&meta.styles);
                }
            }
            crate::genre_map::expand_parent_genres(&mut meta.genres, &mut meta.styles);
            // Only cache meaningful results (score >= 20: album match + artist or better)
            if score >= 20 {
                self.cache.lock().unwrap().put(cache_key.clone(), meta.clone());
                // Write-through to SQLite
                if let Some(ref sqlite) = self.sqlite_cache {
                    if let Ok(cache) = sqlite.lock() {
                        let sqlite_meta = sqlite_meta_from_domain(&meta);
                        if let Err(e) = cache.put(&cache_key, &sqlite_meta) {
                            tracing::warn!("Failed to write to SQLite cache: {}", e);
                        }
                    }
                }
                self.save_cache();
            }
            return Ok(meta);
        }

        Ok(ValidatedMetadata::default())
    }

    /// Fast year/genre/style resolve. Queries LB (priority 6) + Last.fm (10, 20).
    /// LB returns year+genres+styles in one HTTP call, no rate limit.
    /// Last.fm adds year+genres fallback. Skip MB/Discogs/Genius (too slow for batch).
    /// Always caches result (even None) to prevent re-resolve.
    pub async fn resolve_fast(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
    ) -> Option<ValidatedMetadata> {
        let cache_key = format!("{}::{}",
            util::norm_for_lfm(&artist.to_lowercase()),
            util::norm_for_lfm(&title.to_lowercase()),
        );
        if let Some(overridden) = self.overrides.lock().unwrap().resolve(artist, title) {
            tracing::info!("resolve_fast: user override for {} - {}", artist, title);
            return Some(overridden);
        }
        if let Some(cached) = self.cache.lock().unwrap().get(&cache_key) {
            if cached.year.is_some() || !cached.genres.is_empty() {
                tracing::debug!("resolve_fast: LRU cache hit for {} - {} (year: {:?}, genres: {})", artist, title, cached.year.as_deref().unwrap_or("none"), cached.genres.len());
                return Some(cached.clone());
            }
        }
        if let Some(ref sqlite) = self.sqlite_cache {
            if let Ok(cache) = sqlite.lock() {
                if let Some(sqlite_meta) = cache.get(&cache_key) {
                    let domain_meta = domain_meta_from_sqlite(&sqlite_meta);
                    if domain_meta.year.is_some() || !domain_meta.genres.is_empty() {
                        tracing::debug!("resolve_fast: SQLite cache hit for {} - {} (year: {:?}, genres: {})", artist, title, domain_meta.year.as_deref().unwrap_or("none"), domain_meta.genres.len());
                        self.cache.lock().unwrap().put(cache_key.clone(), domain_meta.clone());
                        return Some(domain_meta);
                    }
                }
            }
        }
        // Collect results from fast providers: LB (6), Last.fm Album (10), Last.fm Track (20)
        let mut year: Option<String> = None;
        let mut genres: Vec<String> = Vec::new();
        let mut styles: Vec<String> = Vec::new();
        let mut mbid: Option<String> = None;
        for provider in &self.providers {
            let p = provider.priority();
            // Skip slow providers: MB (7), Discogs (8), Genius (40), MetalApi (5), LibreFM (8)
            if p != 6 && p != 10 && p != 20 { continue; }
            if let Some(meta) = provider.lookup(artist, title, album, &self.http_client).await {
                let provider_name = match p { 6 => "LB", 10 => "Last.fm Album", 20 => "Last.fm Track", _ => "?" };
                tracing::debug!("resolve_fast: {} returned for {} - {} (year: {:?}, genres: {}, styles: {})", provider_name, artist, title, meta.year.as_deref().unwrap_or("none"), meta.genres.len(), meta.styles.len());
                if year.is_none() && meta.year.is_some() {
                    year = meta.year;
                }
                for g in meta.genres {
                    if !genres.contains(&g) {
                        genres.push(g);
                    }
                }
                for s in meta.styles {
                    if !styles.contains(&s) {
                        styles.push(s);
                    }
                }
                if mbid.is_none() && meta.musicbrainz_release_group_id.is_some() {
                    mbid = meta.musicbrainz_release_group_id;
                }
            }
        }
        // Normalize and expand genres
        if !genres.is_empty() {
            genres = crate::genre_map::normalize_genres(&genres);
        }
        if !styles.is_empty() {
            styles = crate::genre_map::normalize_genres(&styles);
        }
        let mut meta = ValidatedMetadata {
            year: year.clone(),
            genres,
            styles,
            musicbrainz_release_group_id: mbid,
            ..ValidatedMetadata::default()
        };
        if !meta.genres.is_empty() || !meta.styles.is_empty() {
            crate::genre_map::expand_parent_genres(&mut meta.genres, &mut meta.styles);
        }
        // Always cache, even sparse
        self.cache.lock().unwrap().put(cache_key.clone(), meta.clone());
        if let Some(ref sqlite) = self.sqlite_cache {
            if let Ok(cache) = sqlite.lock() {
                if let Err(e) = cache.put(&cache_key, &sqlite_meta_from_domain(&meta)) {
                    tracing::warn!("Failed to write SQLite cache in resolve_fast: {}", e);
                }
            }
        }
        let found = year.is_some() || !meta.genres.is_empty();
        if found {
            tracing::info!("resolve_fast: {} - {} -> year={:?}, genres={}, styles={}", artist, title, year.as_deref().unwrap_or("none"), meta.genres.len(), meta.styles.len());
        } else {
            tracing::debug!("resolve_fast: {} - {} -> no data from any fast provider", artist, title);
        }
        if found { Some(meta) } else { None }
    }

    /// Fast year-only resolve. Skips slow providers (MB, Discogs, Genius)
    /// via priority filter (only 10/20 = Last.fm). Always caches result.
    pub async fn resolve_year_fast(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
    ) -> Result<Option<String>, anyhow::Error> {
        let cache_key = format!("{}::{}",
            util::norm_for_lfm(&artist.to_lowercase()),
            util::norm_for_lfm(&title.to_lowercase()),
        );
        if let Some(overridden) = self.overrides.lock().unwrap().resolve(artist, title) {
            return Ok(overridden.year);
        }
        if let Some(cached) = self.cache.lock().unwrap().get(&cache_key) {
            return Ok(cached.year.clone());
        }
        if let Some(ref sqlite) = self.sqlite_cache {
            if let Ok(cache) = sqlite.lock() {
                if let Some(sqlite_meta) = cache.get(&cache_key) {
                    let domain_meta = domain_meta_from_sqlite(&sqlite_meta);
                    self.cache.lock().unwrap().put(cache_key.clone(), domain_meta.clone());
                    return Ok(domain_meta.year);
                }
            }
        }
        let mut year: Option<String> = None;
        for provider in &self.providers {
            let p = provider.priority();
            // Only Last.fm (10 AlbumSearch, 20 TrackSearch) - skip others
            if p != 10 && p != 20 { continue; }
            if let Some(meta) = provider.lookup(artist, title, album, &self.http_client).await {
                if meta.year.is_some() {
                    year = meta.year;
                    break;
                }
            }
        }
        // Always cache, even None, to prevent re-resolve on every library load
        let meta = ValidatedMetadata { year: year.clone(), ..ValidatedMetadata::default() };
        self.cache.lock().unwrap().put(cache_key.clone(), meta.clone());
        if let Some(ref sqlite) = self.sqlite_cache {
            if let Ok(cache) = sqlite.lock() {
                let sqlite_meta = sqlite_meta_from_domain(&meta);
                let _ = cache.put(&cache_key, &sqlite_meta);
            }
        }
        Ok(year)
    }

    fn cache_file_path(&self) -> Option<PathBuf> {
        self.cache_path.as_ref().map(|p| p.join("metadata_cache.json"))
    }

    /// Initialize caches: load JSON into LRU, then import JSON entries into SQLite.
    fn init_cache(&self) {
        self.load_json_into_lru();
        self.import_json_to_sqlite();
    }

    fn load_json_into_lru(&self) {
        let Some(path) = self.cache_file_path() else { return; };
        if !path.exists() { return; }
        match std::fs::read_to_string(&path) {
            Ok(data) => {
                match serde_json::from_str::<Vec<(String, ValidatedMetadata)>>(&data) {
                    Ok(entries) => {
                        let mut cache = self.cache.lock().unwrap();
                        for (key, meta) in entries {
                            cache.put(key, meta);
                        }
                        tracing::info!("Loaded {} entries from metadata cache", cache.len());
                    }
                    Err(e) => tracing::warn!("Failed to parse metadata cache: {}", e),
                }
            }
            Err(e) => tracing::warn!("Failed to read metadata cache: {}", e),
        }
    }

    fn import_json_to_sqlite(&self) {
        let Some(ref sqlite) = self.sqlite_cache else { return; };
        let cache = self.cache.lock().unwrap();
        let Ok(sqlite_cache) = sqlite.lock() else { return; };
        for (key, meta) in cache.iter() {
            let sqlite_meta = sqlite_meta_from_domain(meta);
            if let Err(e) = sqlite_cache.put(&key, &sqlite_meta) {
                tracing::warn!("Failed to import cache entry to SQLite: {}", e);
            }
        }
    }

    fn save_cache(&self) {
        let Some(path) = self.cache_file_path() else { return; };
        let entries: Vec<(String, ValidatedMetadata)> = {
            let cache = self.cache.lock().unwrap();
            cache.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };
        if entries.is_empty() { return; }
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
        // Check LRU first
        if let Some(m) = self.cache.lock().unwrap().get(key).cloned()
            .filter(|m| m.album.is_some() || m.year.is_some()) {
            return Some(m);
        }
        // Fallback to SQLite — populate LRU for next time
        if let Some(ref sqlite) = self.sqlite_cache {
            if let Ok(cache) = sqlite.lock() {
                if let Some(sqlite_meta) = cache.get(key) {
                    let domain_meta = domain_meta_from_sqlite(&sqlite_meta);
                    if domain_meta.album.is_some() || domain_meta.year.is_some() {
                        self.cache.lock().unwrap().put(key.to_string(), domain_meta.clone());
                        return Some(domain_meta);
                    }
                }
            }
        }
        None
    }

    pub fn get_sqlite_cache(&self) -> Option<&Mutex<SqliteCache>> {
        self.sqlite_cache.as_ref()
    }

    pub fn save_override(&self, artist: &str, title: &str, meta: &ValidatedMetadata) {
        let mut overrides = self.overrides.lock().unwrap();
        overrides.set(artist, title, meta);
        if let Some(ref path) = self.overrides_path {
            overrides.save_to(path);
        }
    }
}

/// Convert domain ValidatedMetadata to SQLite cache format.
fn sqlite_meta_from_domain(meta: &ValidatedMetadata) -> metadata_cache_sqlite::ValidatedMetadata {
    metadata_cache_sqlite::ValidatedMetadata {
        artist: meta.artist.clone(),
        album: meta.album.clone(),
        year: meta.year.clone(),
        track_no: meta.track_no,
        album_tracks: meta.album_tracks.iter().map(|t| {
            metadata_cache_sqlite::AlbumTrack {
                title: t.title.clone(),
                duration_secs: t.duration_secs,
                artist: t.artist.clone(),
            }
        }).collect(),
        genres: meta.genres.clone(),
        styles: meta.styles.clone(),
        musicbrainz_release_group_id: meta.musicbrainz_release_group_id.clone(),
    }
}

/// Convert SQLite cache ValidatedMetadata back to domain format.
fn domain_meta_from_sqlite(meta: &metadata_cache_sqlite::ValidatedMetadata) -> ValidatedMetadata {
    ValidatedMetadata {
        artist: meta.artist.clone(),
        album: meta.album.clone(),
        year: meta.year.clone(),
        track_no: meta.track_no,
        album_tracks: meta.album_tracks.iter().map(|t| {
            AlbumTrack {
                title: t.title.clone(),
                duration_secs: t.duration_secs,
                artist: t.artist.clone(),
            }
        }).collect(),
        genres: meta.genres.clone(),
        styles: meta.styles.clone(),
        musicbrainz_release_group_id: meta.musicbrainz_release_group_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overrides::MetadataOverrides;

    fn make_meta(artist: Option<&str>, album: Option<&str>, year: Option<&str>, tracks: usize) -> ValidatedMetadata {
        ValidatedMetadata {
            artist: artist.map(String::from),
            album: album.map(String::from),
            year: year.map(String::from),
            album_tracks: (0..tracks).map(|i| AlbumTrack {
                title: format!("Track {}", i + 1),
                duration_secs: 100.0,
                artist: None,
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
        // artist missing → tracklist +80, 3 tracks = 3 → 83
        assert_eq!(MetadataRegistry::score_result(&meta, "Artist", "Title"), 83);
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
        assert_eq!(score, 10); // contains match only
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
        // artist missing → tracklist +80, min(10,20)*1 = 10 → 90
        assert_eq!(MetadataRegistry::score_result(&meta, "Artist", "Title"), 90);
    }

    #[test]
    fn score_complete_metadata() {
        let meta = make_meta(Some("Metallica"), Some("Master of Puppets"), Some("1986"), 8);
        let score = MetadataRegistry::score_result(&meta, "Metallica", "Master of Puppets");
        // tracks(100) + album(10) + year(5) + artist_exact(50) + album_title(15) + and_norm(10) + 8 = 198
        assert_eq!(score, 198);
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
            sqlite_cache: None,
        }
    }

    fn make_mock_meta(artist: Option<&str>, album: Option<&str>, year: Option<&str>,
                       tracks: usize, genres: Vec<&str>) -> ValidatedMetadata {
        ValidatedMetadata {
            artist: artist.map(String::from),
            album: album.map(String::from),
            year: year.map(String::from),
            album_tracks: (0..tracks).map(|i| AlbumTrack {
                title: format!("Track {}", i + 1),
                duration_secs: 100.0,
                artist: None,
            }).collect(),
            genres: genres.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn resolve_selects_highest_scored_provider() {
        // Provider 1: tracks + album + exact artist → high score
        let p1 = MockProvider {
            priority_val: 1,
            result: Some(ValidatedMetadata {
                artist: Some("Metallica".into()),
                album: Some("Master of Puppets".into()),
                year: Some("1986".into()),
                album_tracks: vec![
                    AlbumTrack { title: "Battery".into(), duration_secs: 500.0, artist: None },
                    AlbumTrack { title: "Master of Puppets".into(), duration_secs: 515.0, artist: None },
                ],
                ..Default::default()
            }),
        };
        // Provider 2: album but no tracks, no artist
        let p2 = MockProvider {
            priority_val: 2,
            result: Some(ValidatedMetadata {
                artist: None,
                album: Some("Master of Puppets".into()),
                ..Default::default()
            }),
        };
        // Provider 3: wrong artist penalty
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

        assert_eq!(result.artist, None);
        assert_eq!(result.album, None);
        assert_eq!(result.year, None);
        assert!(result.album_tracks.is_empty());
    }

    #[test]
    fn resolve_uses_album_param_for_better_match() {
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
        let result =
            futures::executor::block_on(reg.resolve("Band", "Song", Some("The Album"))).unwrap();
        assert_eq!(result.artist, Some("Band".to_string()));
        assert_eq!(result.album, Some("The Album".to_string()));
        assert_eq!(result.album_tracks.len(), 1);

        let result2 =
            futures::executor::block_on(reg.resolve("Band", "Song", None)).unwrap();
        assert_eq!(result2.album, Some("The Album".to_string()));
    }

    #[test]
    fn resolve_merge_year_from_multiple_providers() {
        // Two providers: same score, MB (priority 7) weight 3, other weight 1
        let mb = MockProvider {
            priority_val: 7,
            result: Some(make_mock_meta(Some("Artist"), Some("Album"), Some("2003"), 0, vec![])),
        };
        let other = MockProvider {
            priority_val: 1,
            result: Some(make_mock_meta(Some("Artist"), Some("Album"), Some("2004"), 0, vec![])),
        };
        let reg = make_registry(vec![Box::new(mb), Box::new(other)]);
        let result = futures::executor::block_on(
            reg.resolve("Artist", "Title", None)
        ).unwrap();
        // MB weight 3 > other weight 1 → MB year wins
        assert_eq!(result.year, Some("2003".to_string()));
    }

    #[test]
    fn resolve_merge_genres_from_multiple_providers() {
        let mb = MockProvider {
            priority_val: 7,
            result: Some(make_mock_meta(Some("Artist"), Some("Album"), Some("2003"), 0, vec!["Thrash metal"])),
        };
        let lb = MockProvider {
            priority_val: 6,
            result: Some(make_mock_meta(Some("Artist"), Some("Album"), Some("2003"), 0, vec!["Speed metal"])),
        };
        let reg = make_registry(vec![Box::new(mb), Box::new(lb)]);
        let result = futures::executor::block_on(
            reg.resolve("Artist", "Title", None)
        ).unwrap();
        assert!(result.genres.contains(&"Thrash metal".to_string()));
        assert!(result.genres.contains(&"Speed metal".to_string()));
    }
}
