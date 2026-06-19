use super::MetadataProvider;
use crate::app::server::ValidatedMetadata;
use futures::future::BoxFuture;

pub struct GeniusProvider {
    token: Option<String>,
}

impl GeniusProvider {
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }
}

impl MetadataProvider for GeniusProvider {
    fn priority(&self) -> u8 { 40 }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        let token = self.token.clone();
        Box::pin(async move {
            let tok = token.as_deref()?;
            if tok.is_empty() { return None; }

            let search_url = format!(
                "https://api.genius.com/search?q={}+{}",
                artist.split_whitespace().collect::<Vec<_>>().join("+"),
                title.split_whitespace().collect::<Vec<_>>().join("+")
            );
            let resp = client
                .get(&search_url)
                .header("Authorization", format!("Bearer {}", tok))
                .send().await.ok()?;
            let data: serde_json::Value = resp.json().await.ok()?;
            let hit = data.pointer("/response/hits/0/result")?;

            // Extract release year from release_date_components
            let year = hit.get("release_date_components")
                .and_then(|c| c.get("year"))
                .and_then(|y| y.as_i64())
                .map(|y| y.to_string());

            // Extract album name if available
            let album = hit.get("album")
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());

            if year.is_some() || album.is_some() {
                tracing::info!("GeniusProvider: found metadata for {} - {} (year={:?}, album={:?})",
                    artist, title, year, album);
                Some(ValidatedMetadata {
                    artist: hit.get("primary_artist")
                        .and_then(|a| a.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string()),
                    album,
                    year,
                    track_no: None,
                    album_tracks: Vec::new(),
                    genres: Vec::new(),
                    styles: Vec::new(),
                })
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_genius_release_year_from_components() {
        let json = serde_json::json!({
            "result": {
                "id": 123,
                "title": "Test Song",
                "primary_artist": {"name": "Test Artist"},
                "release_date_components": {"year": 2003, "month": 6, "day": 15}
            }
        });
        let hit = json.get("result").unwrap();
        let year = hit.get("release_date_components")
            .and_then(|c| c.get("year"))
            .and_then(|y| y.as_i64())
            .map(|y| y.to_string());
        assert_eq!(year, Some("2003".to_string()));
    }

    #[test]
    fn parse_genius_missing_album() {
        let json = serde_json::json!({
            "result": {
                "id": 456,
                "title": "No Album",
                "primary_artist": {"name": "Artist"}
            }
        });
        let hit = json.get("result").unwrap();
        let album = hit.get("album")
            .and_then(|a| a.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        assert_eq!(album, None);
        // Year also missing
        let year = hit.get("release_date_components")
            .and_then(|c| c.get("year"))
            .and_then(|y| y.as_i64())
            .map(|y| y.to_string());
        assert_eq!(year, None);
    }
}
