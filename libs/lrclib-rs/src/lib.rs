/// LRCLIB.net lyrics API client.
/// Provides free, open-source lyrics synced to song duration.
/// No API key required.
use serde::{Deserialize, Serialize};

/// A lyrics response from LRCLIB.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrcLibResponse {
    pub id: i64,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: Option<String>,
    pub duration: f64,
    pub instrumental: bool,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
}

/// LRCLIB client.
pub struct LrcLibClient {
    client: reqwest::Client,
    /// Base URL for the API (defaults to "https://lrclib.net")
    base_url: String,
}

impl LrcLibClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: "https://lrclib.net".to_string(),
        }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn set_base_url(&mut self, url: String) {
        self.base_url = url;
    }

    /// Fetch lyrics by exact artist/track match.
    /// Optionally accepts album name for better matching.
    pub async fn fetch_lyrics(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
    ) -> Result<String, String> {
        // Primary: exact match via /api/get
        let mut url = format!(
            "{}/api/get?artist_name={}&track_name={}",
            self.base_url,
            urlenc(artist),
            urlenc(title),
        );
        if let Some(album_name) = album {
            url.push_str("&album_name=");
            url.push_str(&urlenc(album_name));
        }

        tracing::info!("LRCLIB: trying exact match {}", url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<LrcLibResponse>().await {
                Ok(data) => {
                    if data.instrumental {
                        tracing::info!("LRCLIB: track is instrumental, no lyrics");
                        return Err("Instrumental track".to_string());
                    }
                    if let Some(ref lyrics) = data.plain_lyrics {
                        if !lyrics.trim().is_empty() {
                            tracing::info!("LRCLIB: found lyrics ({} chars)", lyrics.len());
                            return Ok(lyrics.clone());
                        }
                    }
                    tracing::info!("LRCLIB: empty plain_lyrics, trying synced");
                    if let Some(ref lyrics) = data.synced_lyrics {
                        if !lyrics.trim().is_empty() {
                            tracing::info!("LRCLIB: found synced lyrics ({} chars)", lyrics.len());
                            return Ok(lyrics.clone());
                        }
                    }
                }
                Err(e) => tracing::warn!("LRCLIB: JSON parse error: {}", e),
            },
            Ok(resp) => {
                let status = resp.status();
                tracing::info!("LRCLIB: exact match returned {}", status);
            }
            Err(e) => tracing::warn!("LRCLIB: request error: {}", e),
        }

        // Fallback: search API
        let query = format!("{} {}", artist, title);
        let search_url = format!("{}/api/search?q={}", self.base_url, urlenc(&query),);
        tracing::info!("LRCLIB: trying search {}", search_url);
        match self.client.get(&search_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Vec<LrcLibResponse>>().await {
                    Ok(results) => {
                        // Find best match by scoring
                        let best = Self::best_match(&results, artist, title);
                        if let Some(data) = best {
                            if data.instrumental {
                                return Err("Instrumental track".to_string());
                            }
                            if let Some(ref lyrics) = data.plain_lyrics {
                                if !lyrics.trim().is_empty() {
                                    tracing::info!(
                                        "LRCLIB: found lyrics via search ({} chars)",
                                        lyrics.len()
                                    );
                                    return Ok(lyrics.clone());
                                }
                            }
                            if let Some(ref lyrics) = data.synced_lyrics {
                                if !lyrics.trim().is_empty() {
                                    tracing::info!(
                                        "LRCLIB: found synced lyrics via search ({} chars)",
                                        lyrics.len()
                                    );
                                    return Ok(lyrics.clone());
                                }
                            }
                        }
                        tracing::info!(
                            "LRCLIB: search returned {} results, none match",
                            results.len()
                        );
                    }
                    Err(e) => tracing::warn!("LRCLIB: search JSON parse error: {}", e),
                }
            }
            Ok(resp) => tracing::info!("LRCLIB: search returned {}", resp.status()),
            Err(e) => tracing::warn!("LRCLIB: search request error: {}", e),
        }

        Err("No lyrics found on LRCLIB".to_string())
    }

    /// Find best matching result from search results.
    fn best_match<'a>(
        results: &'a [LrcLibResponse],
        artist: &str,
        title: &str,
    ) -> Option<&'a LrcLibResponse> {
        let artist_lower = artist.to_lowercase();
        let title_lower = title.to_lowercase();
        let artist_norm: String = artist_lower
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();
        let title_norm: String = title_lower
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();

        // Score each result
        let mut scored: Vec<(&LrcLibResponse, i32)> = results
            .iter()
            .map(|r| {
                let mut score = 0;
                let ra = r.artist_name.to_lowercase();
                let rt = r.track_name.to_lowercase();
                let ra_norm: String = ra.chars().filter(|c| c.is_alphanumeric()).collect();
                let rt_norm: String = rt.chars().filter(|c| c.is_alphanumeric()).collect();

                // Exact artist match = big bonus
                if ra_norm == artist_norm {
                    score += 50;
                } else if ra.contains(&artist_lower) || artist_lower.contains(&ra) {
                    score += 30;
                }

                // Exact title match = big bonus
                if rt_norm == title_norm {
                    score += 50;
                } else if rt.contains(&title_lower) || title_lower.contains(&rt) {
                    score += 30;
                }

                // Both match = highest confidence
                if ra_norm == artist_norm && rt_norm == title_norm {
                    score += 100;
                }

                (r, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        let (best, score) = scored.first()?;
        if *score > 0 {
            Some(best)
        } else {
            None
        }
    }
}

fn urlenc(s: &str) -> String {
    urlencoding(s)
}

/// Simple URL encoding (replaces spaces and special chars).
fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '&' => result.push_str("%26"),
            '?' => result.push_str("%3F"),
            '=' => result.push_str("%3D"),
            '/' => result.push_str("%2F"),
            '#' => result.push_str("%23"),
            '"' => result.push_str("%22"),
            '\'' => result.push_str("%27"),
            '+' => result.push_str("%2B"),
            ',' => result.push_str("%2C"),
            ';' => result.push_str("%3B"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlenc("Hello World"), "Hello%20World");
        assert_eq!(urlenc("a&b=c"), "a%26b%3Dc");
        assert_eq!(urlenc("simple"), "simple");
    }

    #[test]
    fn test_best_match_exact() {
        let results = vec![LrcLibResponse {
            id: 1,
            track_name: "Wasted".to_string(),
            artist_name: "FIDLAR".to_string(),
            album_name: None,
            duration: 180.0,
            instrumental: false,
            plain_lyrics: Some("lyrics".to_string()),
            synced_lyrics: None,
        }];
        let best = LrcLibClient::best_match(&results, "FIDLAR", "Wasted");
        assert!(best.is_some());
        assert_eq!(best.unwrap().id, 1);
    }

    #[test]
    fn test_best_match_no_match() {
        let results = vec![LrcLibResponse {
            id: 1,
            track_name: "Song A".to_string(),
            artist_name: "Artist A".to_string(),
            album_name: None,
            duration: 180.0,
            instrumental: false,
            plain_lyrics: Some("lyrics".to_string()),
            synced_lyrics: None,
        }];
        let best = LrcLibClient::best_match(&results, "FIDLAR", "Wasted");
        assert!(best.is_none());
    }

    #[test]
    fn test_best_match_case_insensitive() {
        let results = vec![LrcLibResponse {
            id: 1,
            track_name: "wasted".to_string(),
            artist_name: "fidlar".to_string(),
            album_name: None,
            duration: 180.0,
            instrumental: false,
            plain_lyrics: Some("lyrics".to_string()),
            synced_lyrics: None,
        }];
        let best = LrcLibClient::best_match(&results, "FIDLAR", "Wasted");
        assert!(best.is_some());
        assert_eq!(best.unwrap().id, 1);
    }
}
