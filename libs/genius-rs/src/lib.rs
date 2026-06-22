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
        let slug_hit = search::hit_from_path(artist, title);
        if scrape::page_exists(&self.client, &slug_hit.path).await {
            return Ok(Some(slug_hit));
        }
        let hits = search::search(&self.client, artist, title, self.token.as_deref()).await?;
        Ok(hits.into_iter().next())
    }

    /// Fetch lyrics for a song given its Genius path (e.g., "/Fidlar-wasted-lyrics").
    pub async fn fetch_lyrics(&self, song_path: &str) -> Result<String, String> {
        scrape::fetch_lyrics(&self.client, song_path).await
    }

    /// Fetch all annotations for a song given its Genius path.
    pub async fn fetch_annotations(&self, song_path: &str) -> Result<Vec<Annotation>, String> {
        scrape::fetch_annotations(&self.client, song_path).await
    }

    /// Search and fetch lyrics in one call.
    /// Tries slug URL first, falls back to search API.
    pub async fn find_and_fetch(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<(SongHit, String), String> {
        let slug_hit = search::hit_from_path(artist, title);
        match self.fetch_lyrics(&slug_hit.path).await {
            Ok(lyrics) => return Ok((slug_hit, lyrics)),
            Err(_) => {}
        }
        let hit = self
            .find_song(artist, title)
            .await?
            .ok_or_else(|| format!("No Genius result for '{} - {}'", artist, title))?;
        let lyrics = self.fetch_lyrics(&hit.path).await?;
        Ok((hit, lyrics))
    }

    /// Search and fetch both lyrics and annotations in one call.
    pub async fn find_fetch_all(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<(SongHit, String, Vec<Annotation>), String> {
        let slug_hit = search::hit_from_path(artist, title);
        match self.fetch_lyrics(&slug_hit.path).await {
            Ok(lyrics) => {
                let annotations = self.fetch_annotations(&slug_hit.path).await.unwrap_or_default();
                return Ok((slug_hit, lyrics, annotations));
            }
            Err(_) => {}
        }
        let hit = self
            .find_song(artist, title)
            .await?
            .ok_or_else(|| format!("No Genius result for '{} - {}'", artist, title))?;
        let lyrics = self.fetch_lyrics(&hit.path).await?;
        let annotations = self.fetch_annotations(&hit.path).await.unwrap_or_default();
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
