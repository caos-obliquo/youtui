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
    pub async fn find_song(&self, artist: &str, title: &str) -> Result<Option<SongHit>, String> {
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
    pub async fn find_and_fetch(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<(SongHit, String), String> {
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
