pub mod annotations;
pub mod scrape;
pub mod search;

pub use scrape::Annotation;
use search::SongHit;

/// Genius.com API client with HTML scraping for lyrics and annotations.
pub struct GeniusClient {
    token: Option<String>,
    client: reqwest::Client,
}

impl GeniusClient {
    pub fn new(token: Option<String>, client: reqwest::Client) -> Self {
        Self { token, client }
    }

    pub fn with_default_client(token: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("genius-rs/0.1.0")
            .build()
            .expect("Failed to build reqwest client");
        Self { token, client }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Search Genius for a song by artist and title.
    /// Tries slug URL first (no API call), falls back to search API.
    pub async fn find_song(&self, artist: &str, title: &str) -> Result<Option<SongHit>, String> {
        // If Bearer token available, prefer API search (gives real song ID for annotations)
        if self.token.as_deref().is_some_and(|t| !t.is_empty()) {
            let hits = search::search(&self.client, artist, title, self.token.as_deref()).await
                .unwrap_or_default();
            if let Some(hit) = hits.into_iter().next() {
                return Ok(Some(hit));
            }
        }
        // Fallback: slug URL without API call
        let slug_hit = search::hit_from_path(artist, title);
        if scrape::page_exists(&self.client, &slug_hit.path).await {
            return Ok(Some(slug_hit));
        }
        // Final fallback: public search API (no auth)
        let hits = search::search(&self.client, artist, title, None).await?;
        Ok(hits.into_iter().next())
    }

    /// Fetch lyrics for a song given its Genius path (e.g., "/Fidlar-wasted-lyrics").
    pub async fn fetch_lyrics(&self, song_path: &str) -> Result<(String, String), String> {
        scrape::fetch_lyrics(&self.client, song_path).await
    }

    /// Fetch all annotations for a song given its Genius path.
    pub async fn fetch_annotations(&self, song_path: &str) -> Result<Vec<Annotation>, String> {
        scrape::fetch_annotations(&self.client, song_path).await
    }

    /// Search and fetch lyrics in one call.
    /// Tries slug URL first, falls back to search API.
    /// Returns (hit, lyrics). Validates hit matches query for search API results.
    pub async fn find_and_fetch(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<(SongHit, String), String> {
        let slug_hit = search::hit_from_path(artist, title);
        // Try slug URL: fetch_lyrics validates final URL matches expected path
        match self.fetch_lyrics(&slug_hit.path).await {
            Ok((lyrics, _final_url)) => return Ok((slug_hit, lyrics)),
            Err(_) => {}
        }
        // Slug failed or redirected — try search API
        let hit = self
            .find_song(artist, title)
            .await?
            .ok_or_else(|| format!("No Genius result for '{} - {}'", artist, title))?;
        // Validate search hit actually matches query
        if !search::hit_matches_query(&hit, artist, title) {
            return Err(format!(
                "Genius hit '{} - {}' does not match query '{} - {}'",
                hit.artist, hit.title, artist, title
            ));
        }
        let (lyrics, _) = self.fetch_lyrics(&hit.path).await?;
        Ok((hit, lyrics))
    }

    /// Fetch annotations using the Genius API with Bearer token.
    /// Falls back to page scraping if API fails or no token available.
    pub async fn fetch_annotations_with_token(
        &self,
        song_path: &str,
        song_id: i64,
    ) -> Result<Vec<Annotation>, String> {
        // Try API first if token available
        if let Some(ref token) = self.token {
            if !token.is_empty() {
                match annotations::fetch_from_api(&self.client, token, song_id).await {
                    Ok(anns) => return Ok(anns),
                    Err(e) => tracing::warn!("Annotation API failed: {}", e),
                }
            }
        }
        // Fallback: scrape from page HTML
        self.fetch_annotations(song_path).await
    }

    /// Search and fetch both lyrics and annotations in one call.
    /// Uses API for annotations when token is available, falls back to page scrape.
    pub async fn find_fetch_all(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<(SongHit, String, Vec<Annotation>), String> {
        // Get the real song ID from search API (needed for annotations API).
        // Lyrics can come from slug URL (faster, no API call).
        let (lyrics, hit) = {
            let slug_hit = search::hit_from_path(artist, title);
            match self.fetch_lyrics(&slug_hit.path).await {
                Ok((l, _)) => {
                    // Try to get real song ID for annotations
                    let real_hit = self.find_song(artist, title).await?.unwrap_or(slug_hit);
                    (l, real_hit)
                }
                Err(_) => {
                    let h = self.find_song(artist, title).await?
                        .ok_or_else(|| format!("No Genius result for '{} - {}'", artist, title))?;
                    let (l, _) = self.fetch_lyrics(&h.path).await?;
                    (l, h)
                }
            }
        };
        let annotations = self.fetch_annotations_with_token(&hit.path, hit.id).await.unwrap_or_default();
        Ok((hit, lyrics, annotations))
    }
}

impl std::fmt::Debug for GeniusClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeniusClient")
            .field("has_token", &self.token.is_some())
            .finish()
    }
}
