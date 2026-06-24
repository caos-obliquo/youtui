use crate::util;
use crate::{AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

pub struct AlbumSearchProvider {
    lastfm_key: Option<String>,
}

impl AlbumSearchProvider {
    pub fn new(lastfm_key: Option<String>) -> Self {
        Self { lastfm_key }
    }
}

impl MetadataProvider for AlbumSearchProvider {
    fn priority(&self) -> u8 { 10 }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        let lastfm_key = self.lastfm_key.clone();
        Box::pin(async move {
            let key = lastfm_key.as_deref()?;
            if key.is_empty() { return None; }

            let search_album = util::norm_for_lfm(title);
            let album_search_url = format!(
                "https://ws.audioscrobbler.com/2.0/?method=album.search&api_key={}&album={}&format=json&limit=5",
                util::urlencoding(key), util::urlencoding(&search_album)
            );
            let resp = client.get(&album_search_url).send().await.ok()?;
            let data: serde_json::Value = resp.json().await.ok()?;
            let matches = data
                .get("results")?.get("albummatches")?.get("album")?.as_array()?;

            for match_album in matches {
                let match_artist = match_album.get("artist")?.as_str()?;
                let match_name = match_album.get("name")?.as_str()?;

                let artist_lower = artist.to_lowercase();
                let match_lower = match_artist.to_lowercase();
                let artist_words: Vec<&str> = artist_lower.split_whitespace().collect();
                let match_words: Vec<&str> = match_lower.split_whitespace().collect();
                let shares_word = artist_words.iter().any(|w| match_words.contains(w))
                    || match_words.iter().any(|w| artist_words.contains(w));
                if !shares_word && !artist_lower.is_empty() { continue; }

                let info_url = format!(
                    "https://ws.audioscrobbler.com/2.0/?method=album.getInfo&api_key={}&artist={}&album={}&format=json",
                    util::urlencoding(key), util::urlencoding(match_artist), util::urlencoding(match_name)
                );
                let info_resp = client.get(&info_url).send().await.ok()?;
                let info_data: serde_json::Value = info_resp.json().await.ok()?;
                let album_data = info_data.get("album")?;

                let year = album_data.get("releaseDate")
                    .or_else(|| album_data.get("release_date"))
                    .or_else(|| album_data.get("releasedate"))
                    .or_else(|| album_data.get("wiki").and_then(|w| w.get("published")))
                    .and_then(|d| d.as_str())
                    .and_then(util::extract_year);

                let mut album_tracks = Vec::new();
                if let Some(tracklist) = album_data
                    .get("tracks")?.get("track")?.as_array()
                {
                    for entry in tracklist {
                        let t_title = entry.get("name")?.as_str()?.to_string();
                        let duration_secs = util::extract_duration(
                            entry.get("duration").unwrap_or(&serde_json::Value::Null)
                        );
                        album_tracks.push(AlbumTrack { title: t_title, duration_secs });
                    }
                }
                let genres: Vec<String> = album_data
                    .get("toptags").and_then(|t| t.get("tag")).and_then(|t| t.as_array())
                    .map(|tags| {
                        let mut all: Vec<(String, u32)> = tags.iter().filter_map(|tag| {
                            let name = tag.get("name")?.as_str()?.to_string();
                            let count = tag.get("count")
                                .and_then(|c| c.as_str().and_then(|s| s.parse::<u32>().ok()))
                                .or_else(|| tag.get("count").and_then(|c| c.as_u64().map(|n| n as u32)))
                                .unwrap_or(0);
                            Some((name, count))
                        }).collect();
                        all.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
                        all.into_iter().take(5).map(|(n, _)| n).collect()
                    })
                    .unwrap_or_default();
                if !album_tracks.is_empty() {
                    return Some(ValidatedMetadata {
                        artist: Some(match_artist.to_string()),
                        album: Some(match_name.to_string()),
                        year,
                        track_no: None,
                        album_tracks,
                        genres,
                        styles: Vec::new(),
                    });
                }
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_lastfm_toptags_sorts_by_count() {
        let json = serde_json::json!({
            "toptags": {
                "tag": [
                    {"name": "death metal", "count": 150},
                    {"name": "metal", "count": 300},
                    {"name": "thrash metal", "count": 50},
                    {"name": "american", "count": 10},
                    {"name": "heavy metal", "count": 80}
                ]
            }
        });
        let genres: Vec<String> = json.get("toptags").and_then(|t| t.get("tag")).and_then(|t| t.as_array())
            .map(|tags| {
                let mut all: Vec<(String, u32)> = tags.iter().filter_map(|tag| {
                    let name = tag.get("name")?.as_str()?.to_string();
                    let count = tag.get("count")
                        .and_then(|c| c.as_str().and_then(|s| s.parse::<u32>().ok()))
                        .or_else(|| tag.get("count").and_then(|c| c.as_u64().map(|n| n as u32)))
                        .unwrap_or(0);
                    Some((name, count))
                }).collect();
                all.sort_by(|a, b| b.1.cmp(&a.1));
                all.into_iter().take(3).map(|(n, _)| n).collect()
            })
            .unwrap_or_default();
        assert_eq!(genres, vec!["metal", "death metal", "heavy metal"]);
    }

    #[test]
    fn parse_lastfm_toptags_empty() {
        let json = serde_json::json!({"toptags": {"tag": []}});
        let genres: Vec<String> = json.get("toptags").and_then(|t| t.get("tag")).and_then(|t| t.as_array())
            .map(|tags| {
                tags.iter().filter_map(|tag| {
                    let name = tag.get("name")?.as_str()?.to_string();
                    Some((name, 0u32))
                }).map(|(n, _)| n).collect()
            })
            .unwrap_or_default();
        assert!(genres.is_empty());
    }
}
