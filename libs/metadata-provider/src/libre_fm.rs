use crate::util;
use crate::{AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

pub struct LibreFMProvider {
    api_key: Option<String>,
}

impl LibreFMProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }
}

impl MetadataProvider for LibreFMProvider {
    fn priority(&self) -> u8 { 8 }
    fn name(&self) -> &'static str { "Libre.fm" }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        album: Option<&'a str>,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        let api_key = self.api_key.clone();
        Box::pin(async move {
            let key = api_key.as_deref()?;
            if key.is_empty() { return None; }

            let search_album = album.map(util::norm_for_lfm).unwrap_or_else(|| util::norm_for_lfm(title));
            let album_search_url = format!(
                "https://libre.fm/2.0/?method=album.search&api_key={}&album={}&format=json&limit=5",
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
                    "https://libre.fm/2.0/?method=album.getInfo&api_key={}&artist={}&album={}&format=json",
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
                let tracks_val = album_data.get("tracks")?.get("track")?;
                let track_iter: Box<dyn Iterator<Item = &serde_json::Value>> = if let Some(arr) = tracks_val.as_array() {
                    Box::new(arr.iter())
                } else {
                    // Single-track album: Libre.fm returns object instead of array
                    Box::new(std::iter::once(tracks_val))
                };
                for entry in track_iter {
                    let t_title = entry.get("name")?.as_str()?.to_string();
                    let duration_secs = util::extract_duration(
                        entry.get("duration").unwrap_or(&serde_json::Value::Null)
                    );
                    album_tracks.push(AlbumTrack { title: t_title, duration_secs, artist: None });
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
                // When album hint matches the search title, the title IS the album name
                // (e.g., channel uploads where song title=album name). Skip track-presence check.
                let searching_by_album = album.is_some() && search_album == util::norm_for_lfm(title);
                // Verify searched track appears in album tracklist (unless title is album name)
                if !album_tracks.is_empty() && !searching_by_album {
                    let title_norm: String = crate::util::norm_for_lfm(title).to_lowercase().chars()
                        .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
                    let title_norm = title_norm.trim();
                    let track_found = album_tracks.iter().any(|t| {
                        let t_norm: String = crate::util::norm_for_lfm(&t.title).to_lowercase().chars()
                            .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
                        let t_norm = t_norm.trim();
                        t_norm == title_norm || t_norm.contains(title_norm) || title_norm.contains(t_norm)
                    });
                    if !track_found {
                        tracing::debug!(
                            "LibreFMProvider: track '{}' not found in album '{}' tracklist, skipping",
                            title, match_name
                        );
                        continue;
                    }
                }
                if !album_tracks.is_empty() {
            return Some(ValidatedMetadata {
                artist: Some(match_artist.to_string()),
                album: Some(match_name.to_string()),
                year,
                track_no: None,
                album_tracks,
                genres,
                styles: Vec::new(),
                musicbrainz_release_group_id: None,
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
    fn parse_librefm_album_getinfo() {
        // Libre.fm returns identical JSON structure to Last.fm album.getinfo
        let json = serde_json::json!({
            "album": {
                "name": "Ride the Lightning",
                "artist": "Metallica",
                "releaseDate": "1984-07-27",
                "tracks": {
                    "track": [
                        {"name": "Fight Fire with Fire", "duration": "285"},
                        {"name": "Ride the Lightning", "duration": "397"},
                        {"name": "For Whom the Bell Tolls", "duration": "310"}
                    ]
                },
                "toptags": {
                    "tag": [
                        {"name": "thrash metal", "count": 300},
                        {"name": "heavy metal", "count": 200},
                        {"name": "metal", "count": 150}
                    ]
                }
            }
        });
        let album_data = json.get("album").unwrap();
        let year = album_data.get("releaseDate")
            .and_then(|d| d.as_str())
            .and_then(crate::util::extract_year);
        assert_eq!(year, Some("1984".to_string()));

        let tracks_val = album_data.get("tracks").unwrap().get("track").unwrap();
        let arr = tracks_val.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].get("name").unwrap().as_str().unwrap(), "Fight Fire with Fire");

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
                all.sort_by(|a, b| b.1.cmp(&a.1));
                all.into_iter().take(3).map(|(n, _)| n).collect()
            })
            .unwrap_or_default();
        assert_eq!(genres, vec!["thrash metal", "heavy metal", "metal"]);
    }

    #[test]
    fn parse_librefm_single_track_album() {
        // Libre.fm may return object instead of array for single-track albums
        let json = serde_json::json!({
            "album": {
                "name": "Single Release",
                "artist": "Test Artist",
                "tracks": {
                    "track": {"name": "Only Song", "duration": "180"}
                },
                "toptags": {"tag": []}
            }
        });
        let album_data = json.get("album").unwrap();
        let tracks_val = album_data.get("tracks").unwrap().get("track").unwrap();
        let track_iter: Box<dyn Iterator<Item = &serde_json::Value>> = if let Some(arr) = tracks_val.as_array() {
            Box::new(arr.iter())
        } else {
            Box::new(std::iter::once(tracks_val))
        };
        let tracks: Vec<String> = track_iter.filter_map(|entry| {
            entry.get("name")?.as_str().map(|s| s.to_string())
        }).collect();
        assert_eq!(tracks, vec!["Only Song"]);
    }

    #[test]
    fn parse_librefm_year_from_release_date_field() {
        let json = serde_json::json!({
            "album": {
                "name": "Test Album",
                "artist": "Test Artist",
                "release_date": "1995-06-15",
                "tracks": {"track": []},
                "toptags": {"tag": []}
            }
        });
        let album_data = json.get("album").unwrap();
        let year = album_data.get("releaseDate")
            .or_else(|| album_data.get("release_date"))
            .or_else(|| album_data.get("releasedate"))
            .or_else(|| album_data.get("wiki").and_then(|w| w.get("published")))
            .and_then(|d| d.as_str())
            .and_then(crate::util::extract_year);
        assert_eq!(year, Some("1995".to_string()));
    }

    #[test]
    fn parse_librefm_year_from_releasedate_field() {
        let json = serde_json::json!({
            "album": {
                "name": "Test Album",
                "artist": "Test Artist",
                "releasedate": "1988-01-01",
                "tracks": {"track": []},
                "toptags": {"tag": []}
            }
        });
        let album_data = json.get("album").unwrap();
        let year = album_data.get("releaseDate")
            .or_else(|| album_data.get("release_date"))
            .or_else(|| album_data.get("releasedate"))
            .or_else(|| album_data.get("wiki").and_then(|w| w.get("published")))
            .and_then(|d| d.as_str())
            .and_then(crate::util::extract_year);
        assert_eq!(year, Some("1988".to_string()));
    }

    #[test]
    fn parse_librefm_genres_sorted_and_capped() {
        let json = serde_json::json!({
            "album": {
                "name": "Test Album",
                "artist": "Test Artist",
                "tracks": {"track": []},
                "toptags": {
                    "tag": [
                        {"name": "metal", "count": 100},
                        {"name": "rock", "count": 80},
                        {"name": "punk", "count": 60},
                        {"name": "alternative", "count": 40},
                        {"name": "indie", "count": 20},
                        {"name": "pop", "count": 10}
                    ]
                }
            }
        });
        let album_data = json.get("album").unwrap();
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
                all.sort_by(|a, b| b.1.cmp(&a.1));
                all.into_iter().take(5).map(|(n, _)| n).collect()
            })
            .unwrap_or_default();
        assert_eq!(genres.len(), 5);
        assert_eq!(genres[0], "metal");
        assert_eq!(genres[1], "rock");
        assert_eq!(genres[2], "punk");
        assert_eq!(genres[3], "alternative");
        assert_eq!(genres[4], "indie");
    }

    #[test]
    fn parse_librefm_empty_tags() {
        let json = serde_json::json!({
            "album": {
                "name": "Test Album",
                "artist": "Test Artist",
                "tracks": {"track": []},
                "toptags": {"tag": []}
            }
        });
        let album_data = json.get("album").unwrap();
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
                all.sort_by(|a, b| b.1.cmp(&a.1));
                all.into_iter().take(5).map(|(n, _)| n).collect()
            })
            .unwrap_or_default();
        assert!(genres.is_empty());
    }

    #[test]
    fn parse_librefm_duration_as_number() {
        let json = serde_json::json!({
            "album": {
                "name": "Test Album",
                "artist": "Test Artist",
                "tracks": {
                    "track": [
                        {"name": "Song One", "duration": 180},
                        {"name": "Song Two", "duration": 240.5}
                    ]
                },
                "toptags": {"tag": []}
            }
        });
        let album_data = json.get("album").unwrap();
        let tracks_val = album_data.get("tracks").unwrap().get("track").unwrap();
        let arr = tracks_val.as_array().unwrap();
        let dur0 = crate::util::extract_duration(arr[0].get("duration").unwrap_or(&serde_json::Value::Null));
        let dur1 = crate::util::extract_duration(arr[1].get("duration").unwrap_or(&serde_json::Value::Null));
        assert_eq!(dur0, 180.0);
        assert_eq!(dur1, 240.5);
    }
}
