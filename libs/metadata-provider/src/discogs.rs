use crate::util;
use crate::{AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

pub struct DiscogsProvider {
    token: Option<String>,
}

impl DiscogsProvider {
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }
}

impl MetadataProvider for DiscogsProvider {
    fn priority(&self) -> u8 { 8 }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        _album: Option<&'a str>,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        let token = self.token.clone();
        Box::pin(async move {
            if artist.is_empty() || title.is_empty() {
                return None;
            }

            let search_url = format!(
                "https://api.discogs.com/database/search?q={}+{}&type=master&format=album&format=cd",
                util::urlencoding(artist), util::urlencoding(title)
            );
            let results: Vec<serde_json::Value> = {
                let mut req = client.get(&search_url)
                    .header("User-Agent", "Youtui/0.1 +https://github.com/caos-obliquo/youtui");
                if let Some(ref t) = token {
                    req = req.header("Authorization", format!("Discogs token={}", t));
                }
                let r = req.send().await.ok()?;
                let d: serde_json::Value = r.json().await.ok()?;
                d.get("results").and_then(|a| a.as_array()).cloned().unwrap_or_default()
            };

            // Helper: find first result matching artist by title field ("Artist - Album Title")
            let find_artist_result = |items: &[serde_json::Value]| -> Option<serde_json::Value> {
                let art_low = artist.to_lowercase();
                for r in items {
                    if let Some(title) = r.get("title").and_then(|t| t.as_str()) {
                        let artist_part = title.split(" - ").next().unwrap_or("").to_lowercase();
                        if artist_part.contains(&art_low) || art_low.contains(&artist_part) {
                            return Some(r.clone());
                        }
                    }
                }
                None
            };

            // If exact search found nothing, try broader artist-only search
            let first = if results.is_empty() {
                tracing::debug!("Discogs exact search found nothing for {} - {}, trying artist fallback", artist, title);
                let fb_url = format!(
                    "https://api.discogs.com/database/search?q={}&type=master&format=album&format=cd",
                    util::urlencoding(artist)
                );
                let mut fb_req = client.get(&fb_url)
                    .header("User-Agent", "Youtui/0.1 +https://github.com/caos-obliquo/youtui");
                if let Some(ref t) = token {
                    fb_req = fb_req.header("Authorization", format!("Discogs token={}", t));
                }
                let fb_resp = fb_req.send().await.ok()?;
                let fb_data: serde_json::Value = fb_resp.json().await.ok()?;
                let fb_items = fb_data.get("results")?.as_array()?;
                find_artist_result(fb_items)
                    .or_else(|| fb_items.first().cloned())?
            } else {
                find_artist_result(&results)
                    .or_else(|| results.first().cloned())?
            };
            let year = first.get("year").and_then(|y| y.as_i64()).map(|y| y.to_string());
            let master_id = first.get("master_id")?.as_i64()?;

            let _d_permit = util::discogs_limiter().acquire().await.ok()?;
            let master_url = format!("https://api.discogs.com/masters/{}", master_id);
            let mut mreq = client.get(&master_url)
                .header("User-Agent", "Youtui/0.1 +https://github.com/caos-obliquo/youtui");
            if let Some(ref t) = token {
                mreq = mreq.header("Authorization", format!("Discogs token={}", t));
            }
            let mresp = mreq.send().await.ok()?;
            let mdata: serde_json::Value = mresp.json().await.ok()?;
            let tracklist = mdata.get("tracklist")?.as_array()?;

            let tracks: Vec<AlbumTrack> = tracklist.iter().filter_map(|entry| {
                let title = entry.get("title")?.as_str()?.to_string();
                let dur_str = entry.get("duration")?.as_str()?;
                let duration_secs = util::parse_discogs_duration(dur_str);
                let track_artist = entry.get("artists").and_then(|a| a.as_array())
                    .and_then(|a| a.first())
                    .and_then(|a| a.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                Some(AlbumTrack { title, duration_secs, artist: track_artist })
            }).collect();

            if !tracks.is_empty() {
                // Validate searched track appears in this album tracklist
                let title_norm = title.to_lowercase();
                let title_norm: String = title_norm.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
                let track_found = tracks.iter().any(|t| {
                    let t_norm: String = t.title.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
                    t_norm.contains(&title_norm) || title_norm.contains(&t_norm)
                });
                if track_found {
                    let album_name = mdata.get("title").and_then(|t| t.as_str()).map(|s| s.to_string());
                    let genres: Vec<String> = mdata.get("genres")
                        .and_then(|g| g.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    let styles: Vec<String> = mdata.get("styles")
                        .and_then(|s| s.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    tracing::info!("DiscogsProvider: {} tracks, {} genres, {} styles for {} - {}", tracks.len(), genres.len(), styles.len(), artist, title);
                    return Some(ValidatedMetadata {
                        artist: mdata.get("artists")
                            .and_then(|a| a.as_array())
                            .and_then(|a| a.first())
                            .and_then(|a| a.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string()),
                        album: album_name,
                        year,
                        track_no: None,
                        album_tracks: tracks,
                        genres,
                        styles,
                    });
                } else {
                    tracing::debug!("Discogs: album has {} tracks but '{}' not found - skipping", tracks.len(), title);
                }
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_discogs_master_extracts_genres_styles() {
        let json = serde_json::json!({
            "genres": ["Rock"],
            "styles": ["Death Metal", "Black Metal", "Technical Death Metal"],
            "tracklist": [
                {"title": "War Ensemble", "duration": "4:51"},
                {"title": "Raining Blood", "duration": "3:15"}
            ],
            "title": "Seasons in the Abyss"
        });
        let genres: Vec<String> = json.get("genres")
            .and_then(|g| g.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let styles: Vec<String> = json.get("styles")
            .and_then(|s| s.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        assert_eq!(genres, vec!["Rock"]);
        assert_eq!(styles, vec!["Death Metal", "Black Metal", "Technical Death Metal"]);
    }

    #[test]
    fn parse_discogs_master_empty_genres() {
        let json = serde_json::json!({
            "tracklist": [{"title": "Track 1", "duration": "3:00"}]
        });
        let genres: Vec<String> = json.get("genres")
            .and_then(|g| g.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        assert!(genres.is_empty());
    }

    #[test]
    fn parse_discogs_duration_mmss() {
        assert!((util::parse_discogs_duration("3:45") - 225.0).abs() < 0.01);
        assert!((util::parse_discogs_duration("1:00") - 60.0).abs() < 0.01);
        assert!((util::parse_discogs_duration("0:30") - 30.0).abs() < 0.01);
    }

    #[test]
    fn parse_discogs_duration_hmmss() {
        assert!((util::parse_discogs_duration("1:15:30") - 4530.0).abs() < 0.01);
    }
}
