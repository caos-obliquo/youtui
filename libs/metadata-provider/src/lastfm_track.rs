use crate::{util, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

pub struct TrackSearchProvider {
    lastfm_key: Option<String>,
}

impl TrackSearchProvider {
    pub fn new(lastfm_key: Option<String>) -> Self {
        Self { lastfm_key }
    }
}

impl MetadataProvider for TrackSearchProvider {
    fn priority(&self) -> u8 {
        20
    }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        _album: Option<&'a str>,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        let lastfm_key = self.lastfm_key.clone();
        Box::pin(async move {
            let key = lastfm_key.as_deref()?;
            if key.is_empty() {
                return None;
            }

            let info_url = format!(
                "https://ws.audioscrobbler.com/2.0/?method=track.getInfo&api_key={}&artist={}&track={}&format=json",
                util::urlencoding(key), util::urlencoding(artist), util::urlencoding(title)
            );
            if let Some(meta) = fetch_track_info(client, &info_url).await {
                return Some(meta);
            }

            let search_url = format!(
                "https://ws.audioscrobbler.com/2.0/?method=track.search&api_key={}&track={}&format=json&limit=5",
                util::urlencoding(key), util::urlencoding(&util::norm_for_lfm(title))
            );
            if let Ok(resp) = client.get(&search_url).send().await {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(results) = data
                        .get("results")
                        .and_then(|r| r.get("trackmatches"))
                        .and_then(|m| m.get("track"))
                        .and_then(|t| t.as_array())
                    {
                        let artist_lower = artist.to_lowercase();
                        for result in results {
                            let result_artist = result.get("artist")?.as_str()?;
                            let result_name = result.get("name")?.as_str()?;
                            let result_lower = result_artist.to_lowercase();
                            let artist_words: Vec<&str> = artist_lower.split_whitespace().collect();
                            let result_words: Vec<&str> = result_lower.split_whitespace().collect();
                            let shares_word = artist_words.iter().any(|w| result_words.contains(w))
                                || result_words.iter().any(|w| artist_words.contains(w));
                            if !shares_word && !artist_lower.is_empty() {
                                continue;
                            }

                            let title_norm = util::norm_for_lfm(title);
                            let result_name_norm = util::norm_for_lfm(result_name);
                            if !title_norm.contains(&result_name_norm)
                                && !result_name_norm.contains(&title_norm)
                            {
                                continue;
                            }

                            let info_url = format!(
                                "https://ws.audioscrobbler.com/2.0/?method=track.getInfo&api_key={}&artist={}&track={}&format=json",
                                util::urlencoding(key), util::urlencoding(result_artist), util::urlencoding(result_name)
                            );
                            if let Some(meta) = fetch_track_info(client, &info_url).await {
                                return Some(meta);
                            }
                        }
                    }
                }
            }
            None
        })
    }
}

/// Call track.getInfo and parse response into ValidatedMetadata.
/// Returns None if request fails or response has no album/year data.
async fn fetch_track_info(client: &reqwest::Client, info_url: &str) -> Option<ValidatedMetadata> {
    let resp = client.get(info_url).send().await.ok()?;
    let data: serde_json::Value = resp.json().await.ok()?;
    let track = data.get("track")?;
    let album = track
        .get("album")
        .and_then(|a| a.get("title"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());
    let artist_name = track
        .get("artist")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let year = track
        .get("wiki")
        .and_then(|w| w.get("published"))
        .and_then(|p| p.as_str())
        .and_then(util::extract_year);
    if album.is_none() && year.is_none() {
        return None;
    }
    let track_no = track
        .get("album")
        .and_then(|a| a.get("@attr"))
        .and_then(|a| a.get("rank"))
        .and_then(|r| r.as_str())
        .and_then(|s| s.parse::<usize>().ok());
    Some(ValidatedMetadata {
        artist: artist_name,
        album,
        year,
        track_no,
        album_tracks: Vec::new(),
        genres: Vec::new(),
        styles: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_lastfm_track_getinfo_extracts_year() {
        let json = serde_json::json!({
            "track": {
                "name": "Master of Puppets",
                "artist": {"name": "Metallica", "mbid": "123"},
                "album": {
                    "title": "Master of Puppets",
                    "@attr": {"rank": "3"}
                },
                "wiki": {
                    "published": "2007-11-11"
                }
            }
        });
        let year = json
            .pointer("/track/wiki/published")
            .and_then(|p| p.as_str())
            .and_then(|d| crate::util::extract_year(d));
        assert_eq!(year, Some("2007".to_string()));
    }

    #[test]
    fn parse_lastfm_track_getinfo_missing_wiki() {
        let json = serde_json::json!({
            "track": {
                "name": "Test",
                "artist": {"name": "Test Artist"},
                "album": {"title": "Test Album"}
            }
        });
        let year = json
            .pointer("/track/wiki/published")
            .and_then(|p| p.as_str())
            .and_then(|d| crate::util::extract_year(d));
        assert_eq!(year, None);
    }

    #[test]
    fn parse_lastfm_track_title_matching() {
        let title = "Master";
        let results = vec!["Master of Puppets", "Battery", "Welcome Home (Sanitarium)"];
        let title_norm = crate::util::norm_for_lfm(title);
        let matching: Vec<&str> = results
            .into_iter()
            .filter(|result| {
                let result_name_norm = crate::util::norm_for_lfm(result);
                title_norm.contains(&result_name_norm) || result_name_norm.contains(&title_norm)
            })
            .collect();
        assert_eq!(matching, vec!["Master of Puppets"]);
    }

    #[test]
    fn parse_lastfm_track_artist_word_filter() {
        let artist = "Metallica";
        let results = vec!["Metallica", "Metallica Tribute", "NotTheSame"];
        let artist_lower = artist.to_lowercase();
        let artist_words: Vec<&str> = artist_lower.split_whitespace().collect();
        let matching: Vec<&str> = results
            .into_iter()
            .filter(|result| {
                let result_lower = result.to_lowercase();
                let result_words: Vec<&str> = result_lower.split_whitespace().collect();
                artist_words.iter().any(|w| result_words.contains(w))
                    || result_words.iter().any(|w| artist_words.contains(w))
            })
            .collect();
        assert_eq!(matching, vec!["Metallica", "Metallica Tribute"]);
    }
}
