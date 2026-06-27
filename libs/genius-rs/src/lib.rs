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
    /// Bearer search first (gives real song ID for annotations),
    /// then slug URL fallback, then public search.
    pub async fn find_song(&self, artist: &str, title: &str) -> Result<Option<SongHit>, String> {
        // Bearer search first - gives real song ID for annotations API
        if self.token.as_deref().is_some_and(|t| !t.is_empty()) {
            let hits = search::search(&self.client, artist, title, self.token.as_deref()).await
                .unwrap_or_default();
            if let Some(hit) = hits.into_iter().next() {
                tracing::info!("Genius: found via Bearer search (id={})", hit.id);
                return Ok(Some(hit));
            }
        }
        // Try full slug URL (no API call, but gives id=0)
        let slug_hit = search::hit_from_path(artist, title);
        if scrape::page_exists(&self.client, &slug_hit.path).await {
            tracing::info!("Genius: found via full slug");
            return Ok(Some(slug_hit));
        }
        // Try simplified slug (strip parenthetical extras)
        let simple_title = search::simplify_title(title);
        if simple_title != title {
            let simple_hit = search::hit_from_path(artist, simple_title);
            if scrape::page_exists(&self.client, &simple_hit.path).await {
                tracing::info!("Genius: found via simplified slug");
                return Ok(Some(simple_hit));
            }
        }
        // Public search API (no auth)
        tracing::info!("Genius: trying public search API");
        let hits = search::search(&self.client, artist, title, None).await?;
        let hit = hits.into_iter().next();
        if hit.is_some() {
            tracing::info!("Genius: found via public search");
        }
        Ok(hit)
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
    /// Tries slug URL first (with and without parenthetical extras),
    /// then search API + bearer search, then public search.
    /// Returns (hit, lyrics). Validates hit matches query for search API results.
    pub async fn find_and_fetch(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<(SongHit, String), String> {
        let slug_hit = search::hit_from_path(artist, title);
        tracing::info!("Genius: try full slug {}", slug_hit.path);
        match self.fetch_lyrics(&slug_hit.path).await {
            Ok((lyrics, _)) => return Ok((slug_hit, lyrics)),
            Err(e) => tracing::info!("Genius: full slug failed: {}", e),
        }
        // Simplified slug: strip parenthetical extras like "(Japanese Bonus Track)"
        let simple_title = search::simplify_title(title);
        if simple_title != title {
            let simple_hit = search::hit_from_path(artist, simple_title);
            tracing::info!("Genius: try simple slug {}", simple_hit.path);
            match self.fetch_lyrics(&simple_hit.path).await {
                Ok((lyrics, _)) => return Ok((simple_hit, lyrics)),
                Err(e) => tracing::info!("Genius: simple slug failed: {}", e),
            }
        }
        // Slug failed - try search API
        tracing::info!("Genius: fallback to search API");
        let hit = self
            .find_song(artist, title)
            .await?
            .ok_or_else(|| format!("No Genius result for '{} - {}'", artist, title))?;
        if !search::hit_matches_query(&hit, artist, title) {
            tracing::warn!("Genius: hit '{} - {}' rejected (query mismatch)", hit.artist, hit.title);
            return Err(format!(
                "Genius hit '{} - {}' does not match query '{} - {}'",
                hit.artist, hit.title, artist, title
            ));
        }
        tracing::info!("Genius: search hit '{} - {}' path={}", hit.artist, hit.title, hit.path);
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
    /// Tries full slug, simplified slug, Bearer search, then public search.
    /// Uses API for annotations when token is available, falls back to page scrape.
    pub async fn find_fetch_all(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<(SongHit, String, Vec<Annotation>), String> {
        let (lyrics, hit) = {
            // Try full slug URL first
            let slug_hit = search::hit_from_path(artist, title);
            match self.fetch_lyrics(&slug_hit.path).await {
                Ok((l, _)) => {
                    let real_hit = self.find_song(artist, title).await?.unwrap_or(slug_hit);
                    (l, real_hit)
                }
                Err(_) => {
                    // Try simplified slug (strip parenthetical extras)
                    let simple_title = search::simplify_title(title);
                    if simple_title != title {
                        let simple_hit = search::hit_from_path(artist, simple_title);
                        if let Ok((l, _)) = self.fetch_lyrics(&simple_hit.path).await {
                            let real_hit = self.find_song(artist, title).await?.unwrap_or(simple_hit);
                            let annotations = self.fetch_annotations_with_token(&real_hit.path, real_hit.id).await.unwrap_or_default();
                            return Ok((real_hit, l, annotations));
                        }
                    }
                    // Fallback to search API
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
