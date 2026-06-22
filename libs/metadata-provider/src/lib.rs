mod lastfm_album;
mod lastfm_track;
mod discogs;
mod genius;
mod musicbrainz;
pub mod util;
pub mod overrides;

pub use lastfm_album::AlbumSearchProvider;
pub use lastfm_track::TrackSearchProvider;
pub use discogs::DiscogsProvider;
pub use genius::GeniusProvider;
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
        for provider in &self.providers {
            if let Some(meta) = provider.lookup(artist, title, &self.http_client).await {
                if meta.album.is_some() || meta.year.is_some() {
                    tracing::info!(
                        "Metadata resolved by provider priority {} for {} - {}",
                        provider.priority(), artist, title
                    );
                    self.cache.lock().unwrap().put(cache_key, meta.clone());
                    return Ok(meta);
                }
            }
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
