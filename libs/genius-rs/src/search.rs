#[derive(Debug, Clone)]
pub struct SongHit {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub album: Option<String>,
    pub thumbnail: Option<String>,
}

impl SongHit {
    pub fn lyrics_url(&self) -> String {
        format!("https://genius.com{}", self.path)
    }
}

/// Search Genius for a song. Tries Bearer token first, falls back to public API.
pub async fn search(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    token: Option<&str>,
) -> Result<Vec<SongHit>, String> {
    if let Some(tok) = token {
        if !tok.is_empty() {
            let url = format!(
                "https://api.genius.com/search?q={}+{}",
                urlenc(artist),
                urlenc(title)
            );
            tracing::info!("Genius Bearer search URL={}, token_len={}", url, tok.len());
            match client
                .get(&url)
                .header("Authorization", format!("Bearer {}", tok))
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    tracing::info!("Genius Bearer search status={}", status);
                    match resp.json::<serde_json::Value>().await {
                        Ok(data) => {
                            tracing::info!("Genius Bearer response keys: {:?}", data.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                            let hits = parse_hits(&data, "/response/hits");
                            tracing::info!("Genius Bearer search parsed {} hits", hits.len());
                            if !hits.is_empty() {
                                return Ok(hits);
                            }
                        }
                        Err(e) => tracing::warn!("Genius Bearer JSON parse error: {}", e),
                    }
                }
                Err(e) => tracing::warn!("Genius Bearer search error: {}", e),
            }
        }
    }

    let url = format!(
        "https://genius.com/api/search/song?q={}+{}",
        urlenc(title),
        urlenc(artist)
    );
    tracing::info!("Genius: public search URL={}", url);
    match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            tracing::info!("Genius: public search status={}", status);
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let hits = parse_hits(&data, "/response/sections/0/hits");
                tracing::info!("Genius: public search returned {} hits", hits.len());
                if !hits.is_empty() {
                    return Ok(hits);
                }
            }
            Err(format!("No results from Genius: {}", url))
        }
        Err(e) => {
            tracing::warn!("Genius: public search request failed: {}", e);
            Err(format!("Genius search request failed: {}", e))
        }
    }
}

fn parse_hits(data: &serde_json::Value, pointer: &str) -> Vec<SongHit> {
    let mut hits = Vec::new();
    if let Some(arr) = data.pointer(pointer).and_then(|v| v.as_array()) {
        for item in arr {
            let result = match item.get("result") {
                Some(r) => r,
                None => continue,
            };
            let id = match result.get("id").and_then(|v| v.as_i64()) {
                Some(id) => id,
                None => continue,
            };
            let path = match result.get("path").and_then(|v| v.as_str()) {
                Some(p) => p.to_string(),
                None => continue,
            };
            let title = match result.get("title").and_then(|v| v.as_str()) {
                Some(t) => t.to_string(),
                None => continue,
            };
            let artist = result
                .get("primary_artist")
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let year = result
                .get("release_date_components")
                .and_then(|c| c.get("year"))
                .and_then(|v| v.as_i64());
            let album = result
                .get("album")
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let thumbnail = result
                .get("song_art_image_thumbnail_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            hits.push(SongHit {
                id,
                path,
                title,
                artist,
                year,
                album,
                thumbnail,
            });
        }
    }
    hits
}

fn urlenc(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join("+")
}

/// Strip parenthetical/bracketed extras from title for slug fallback.
/// "Shitty Jobz (Japanese Bonus Track)" → "Shitty Jobz"
pub fn simplify_title(title: &str) -> &str {
    if let Some(idx) = title.find('(') {
        title[..idx].trim()
    } else if let Some(idx) = title.find('[') {
        title[..idx].trim()
    } else {
        title
    }
}

/// Compute a Genius URL slug from artist and title.
/// Used as a fallback when search API returns wrong results.
/// e.g., ("Love Letter", "Love Letter") → "/love-letter-love-letter-lyrics"
pub fn compute_path(artist: &str, title: &str) -> String {
    let slug = |s: &str| -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-")
    };
    format!("/{}-{}-lyrics", slug(artist), slug(title))
}

/// Create a synthetic SongHit from a computed slug path (no API call needed).
pub fn hit_from_path(artist: &str, title: &str) -> SongHit {
    SongHit {
        id: 0,
        path: compute_path(artist, title),
        title: title.to_string(),
        artist: artist.to_string(),
        year: None,
        album: None,
        thumbnail: None,
    }
}

/// Check if a SongHit actually matches the queried artist and title.
/// Normalizes both sides (lowercase, strip trailing punctuation) for fuzzy comparison.
pub fn hit_matches_query(hit: &SongHit, artist: &str, title: &str) -> bool {
    let norm = |s: &str| -> String {
        s.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string()
    };
    let hit_artist = norm(&hit.artist);
    let hit_title = norm(&hit.title);
    let query_artist = norm(artist);
    let query_title = norm(title);
    // Artist AND title must both match (no artist-only match)
    let artist_ok = hit_artist == query_artist
        || hit_artist.contains(&query_artist)
        || query_artist.contains(&hit_artist);
    let title_ok = hit_title.contains(&query_title)
        || query_title.contains(&hit_title);
    artist_ok && title_ok
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hits_bearer() {
        let json = serde_json::json!({
            "response": {
                "hits": [
                    {
                        "result": {
                            "id": 2890914,
                            "path": "/Fidlar-wasted-lyrics",
                            "title": "Wasted",
                            "primary_artist": {"name": "FIDLAR"},
                            "release_date_components": {"year": 2013},
                            "album": {"name": "Don't Fuck With Vol. 02"},
                            "song_art_image_thumbnail_url": "https://images.genius.com/abc.jpg"
                        }
                    }
                ]
            }
        });
        let hits = parse_hits(&json, "/response/hits");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 2890914);
        assert_eq!(hits[0].path, "/Fidlar-wasted-lyrics");
        assert_eq!(hits[0].title, "Wasted");
        assert_eq!(hits[0].artist, "FIDLAR");
        assert_eq!(hits[0].year, Some(2013));
        assert_eq!(hits[0].album, Some("Don't Fuck With Vol. 02".to_string()));
    }

    #[test]
    fn test_parse_hits_public() {
        let json = serde_json::json!({
            "response": {
                "sections": [
                    {
                        "hits": [
                            {
                                "result": {
                                    "id": 123,
                                    "path": "/test-song-lyrics",
                                    "title": "Test Song",
                                    "primary_artist": {"name": "Test Artist"}
                                }
                            }
                        ]
                    }
                ]
            }
        });
        let hits = parse_hits(&json, "/response/sections/0/hits");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 123);
    }

    #[test]
    fn test_parse_hits_empty() {
        let hits = parse_hits(&serde_json::json!({"response": {}}), "/response/hits");
        assert!(hits.is_empty());
    }

    #[test]
    fn test_compute_path() {
        assert_eq!(compute_path("FIDLAR", "Wasted"), "/fidlar-wasted-lyrics");
        assert_eq!(compute_path("Love Letter", "Love Letter"), "/love-letter-love-letter-lyrics");
        assert_eq!(compute_path("Alice in Chains", "It Ain't Like That"), "/alice-in-chains-it-aint-like-that-lyrics");
    }
}
