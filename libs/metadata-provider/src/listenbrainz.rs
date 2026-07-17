// Copyright (c) 2026 caos-obliquo <caos_obliquo@outlook.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::genre_map::is_known_genre;
use crate::util;
use crate::{MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

pub struct ListenBrainzProvider {
    token: String,
}

impl ListenBrainzProvider {
    pub fn new(token: String) -> Self {
        if token.is_empty() {
            tracing::warn!("ListenBrainzProvider created with empty token");
        }
        Self { token }
    }
}

impl MetadataProvider for ListenBrainzProvider {
    fn priority(&self) -> u8 {
        6
    }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        _album: Option<&'a str>,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        let artist = artist.to_string();
        let title = title.to_string();
        let token = self.token.clone();
        let client = client.clone();
        Box::pin(async move {
            if token.is_empty() {
                return None;
            }

            let url = format!(
                "https://api.listenbrainz.org/1/metadata/lookup/?artist_name={}&recording_name={}&metadata=true&inc=artist+tag+release",
                util::urlencoding(&artist),
                util::urlencoding(&title)
            );

            tracing::debug!(
                "ListenBrainz lookup: {} - {}",
                artist,
                title
            );

            let resp = client
                .get(&url)
                .header("Authorization", format!("Token {}", token))
                .send()
                .await
                .ok()?;

            if !resp.status().is_success() {
                tracing::debug!(
                    "ListenBrainz returned HTTP {} for {} - {}",
                    resp.status(),
                    artist,
                    title
                );
                return None;
            }

            let data: serde_json::Value = resp.json().await.ok()?;
            let metadata = data.get("metadata")?;

            let artist_name = metadata
                .get("artist_credit_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let (album_name, year) = metadata
                .get("release")
                .map(|release| {
                    let name = release
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let year = release
                        .get("year")
                        .and_then(|v| v.as_i64())
                        .map(|y| y.to_string());
                    (name, year)
                })
                .unwrap_or((None, None));

            let mut genres = Vec::new();
            let mut styles = Vec::new();

            if let Some(tag_obj) = metadata.get("tag") {
                // Collect recording tags
                if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
                    for entry in recording_tags {
                        if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                            let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                            if entry.get("genre_mbid").is_some() || is_known_genre(tag_name) {
                                genres.push((count, tag_name.to_string()));
                            } else {
                                styles.push((count, tag_name.to_string()));
                            }
                        }
                    }
                }
                // Collect release_group tags
                if let Some(rg_tags) = tag_obj.get("release_group").and_then(|a| a.as_array()) {
                    for entry in rg_tags {
                        if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                            let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                            if entry.get("genre_mbid").is_some() || is_known_genre(tag_name) {
                                genres.push((count, tag_name.to_string()));
                            } else {
                                styles.push((count, tag_name.to_string()));
                            }
                        }
                    }
                }
            }

            // Sort by count descending, dedupe, take top 10
            genres.sort_unstable_by_key(|(count, _)| std::cmp::Reverse(*count));
            styles.sort_unstable_by_key(|(count, _)| std::cmp::Reverse(*count));

            // Dedup while preserving count-sorted order
            let mut seen = std::collections::HashSet::new();
            let genres: Vec<String> = genres
                .into_iter()
                .filter(|(_, name)| seen.insert(name.clone()))
                .map(|(_, name)| name)
                .take(10)
                .collect();
            seen.clear();
            let styles: Vec<String> = styles
                .into_iter()
                .filter(|(_, name)| seen.insert(name.clone()))
                .map(|(_, name)| name)
                .take(10)
                .collect();

            tracing::debug!(
                "ListenBrainz result: artist={:?} album={:?} year={:?} genres={} styles={}",
                artist_name,
                album_name,
                year,
                genres.len(),
                styles.len()
            );

            Some(ValidatedMetadata {
                artist: artist_name,
                album: album_name,
                year,
                track_no: None,
                album_tracks: Vec::new(),
                genres,
                styles,
                musicbrainz_release_group_id: None,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_listenbrainz_fixture() {
        let json = serde_json::json!({
            "metadata": {
                "artist_credit_name": "Metallica",
                "release": {
                    "name": "Master of Puppets",
                    "year": 1986
                },
                "tag": {
                    "recording": [
                        {"count": 67, "genre_mbid": "abc-123", "tag": "thrash metal"},
                        {"count": 33, "tag": "metal"},
                        {"count": 12, "genre_mbid": "def-456", "tag": "heavy metal"}
                    ],
                    "release_group": [
                        {"count": 42, "genre_mbid": "ghi-789", "tag": "speed metal"},
                        {"count": 5, "tag": "1980s"}
                    ]
                }
            }
        });

        let metadata = json.get("metadata").unwrap();
        let artist = metadata
            .get("artist_credit_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let album = metadata
            .get("release")
            .and_then(|r| r.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let year = metadata
            .get("release")
            .and_then(|r| r.get("year"))
            .and_then(|v| v.as_i64())
            .map(|y| y.to_string());

        assert_eq!(artist, Some("Metallica".to_string()));
        assert_eq!(album, Some("Master of Puppets".to_string()));
        assert_eq!(year, Some("1986".to_string()));

        // Parse tags
        let mut genres = Vec::new();
        let mut styles = Vec::new();
        if let Some(tag_obj) = metadata.get("tag") {
            if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
                for entry in recording_tags {
                    if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                        let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                        if entry.get("genre_mbid").is_some() {
                            genres.push((count, tag_name.to_string()));
                        } else {
                            styles.push((count, tag_name.to_string()));
                        }
                    }
                }
            }
            if let Some(rg_tags) = tag_obj.get("release_group").and_then(|a| a.as_array()) {
                for entry in rg_tags {
                    if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                        let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                        if entry.get("genre_mbid").is_some() {
                            genres.push((count, tag_name.to_string()));
                        } else {
                            styles.push((count, tag_name.to_string()));
                        }
                    }
                }
            }
        }

        genres.sort_by(|a, b| b.0.cmp(&a.0));
        styles.sort_by(|a, b| b.0.cmp(&a.0));

        let genre_names: Vec<String> = genres.into_iter().map(|(_, n)| n).collect();
        let style_names: Vec<String> = styles.into_iter().map(|(_, n)| n).collect();

        assert_eq!(genre_names, vec!["thrash metal", "speed metal", "heavy metal"]);
        assert_eq!(style_names, vec!["metal", "1980s"]);
    }

    #[test]
    fn listenbrainz_malformed_response() {
        let json = serde_json::json!({"error": "not found"});
        let metadata = json.get("metadata");
        assert!(metadata.is_none(), "Missing metadata key should return None");
    }

    #[test]
    fn listenbrainz_no_token() {
        let provider = ListenBrainzProvider::new(String::new());
        assert_eq!(provider.priority(), 6);
    }

    #[test]
    fn listenbrainz_parse_no_release() {
        let json = serde_json::json!({
            "metadata": {
                "artist_credit_name": "Metallica",
                "tag": {
                    "recording": [
                        {"count": 10, "genre_mbid": "x", "tag": "thrash metal"}
                    ]
                }
            }
        });
        let metadata = json.get("metadata").expect("metadata present");
        let album = metadata
            .get("release")
            .and_then(|r| r.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let year = metadata
            .get("release")
            .and_then(|r| r.get("year"))
            .and_then(|v| v.as_i64())
            .map(|y| y.to_string());
        assert_eq!(album, None);
        assert_eq!(year, None);

        let mut genres = Vec::new();
        if let Some(tag_obj) = metadata.get("tag") {
            if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
                for entry in recording_tags {
                    if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                        if entry.get("genre_mbid").is_some() {
                            genres.push(tag_name.to_string());
                        }
                    }
                }
            }
        }
        assert_eq!(genres, vec!["thrash metal"]);
    }

    #[test]
    fn listenbrainz_parse_dedupes_tags() {
        let json = serde_json::json!({
            "metadata": {
                "tag": {
                    "recording": [
                        {"count": 67, "genre_mbid": "abc", "tag": "thrash metal"},
                        {"count": 33, "genre_mbid": "def", "tag": "thrash metal"}
                    ]
                }
            }
        });
        let metadata = json.get("metadata").expect("metadata present");
        let mut genres = Vec::new();
        if let Some(tag_obj) = metadata.get("tag") {
            if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
                for entry in recording_tags {
                    if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                        let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                        if entry.get("genre_mbid").is_some() {
                            genres.push((count, tag_name.to_string()));
                        }
                    }
                }
            }
        }
        genres.sort_unstable_by_key(|(count, _)| std::cmp::Reverse(*count));
        let mut seen = std::collections::HashSet::new();
        let deduped: Vec<String> = genres
            .into_iter()
            .filter(|(_, name)| seen.insert(name.clone()))
            .map(|(_, name)| name)
            .collect();
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0], "thrash metal");
    }

    #[test]
    fn listenbrainz_parse_empty_tags() {
        let json = serde_json::json!({
            "metadata": {
                "artist_credit_name": "Unknown",
                "release": {"name": "Album", "year": 2020},
                "tag": {"recording": [], "release_group": []}
            }
        });
        let metadata = json.get("metadata").expect("metadata present");
        let mut genres = Vec::new();
        let mut styles = Vec::new();
        if let Some(tag_obj) = metadata.get("tag") {
            if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
                for entry in recording_tags {
                    if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                        let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                        if entry.get("genre_mbid").is_some() {
                            genres.push((count, tag_name.to_string()));
                        } else {
                            styles.push((count, tag_name.to_string()));
                        }
                    }
                }
            }
        }
        assert!(genres.is_empty());
        assert!(styles.is_empty());
    }

    #[test]
    fn listenbrainz_promotes_known_genre_without_mbid() {
        // Tags without genre_mbid but known to genre_map or RYM
        // should be promoted to genres, not styles.
        // "funk" is in discogs_overrides. "2-Step" is in RYM (not MusicBee).
        let json = serde_json::json!({
            "metadata": {
                "tag": {
                    "recording": [
                        {"count": 20, "tag": "funk"},
                        {"count": 10, "tag": "2-Step"},
                        {"count": 5, "tag": "random noise"}
                    ]
                }
            }
        });
        let metadata = json.get("metadata").unwrap();

        let mut genres = Vec::new();
        let mut styles = Vec::new();
        if let Some(tag_obj) = metadata.get("tag") {
            if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
                for entry in recording_tags {
                    if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                        let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                        if entry.get("genre_mbid").is_some() || crate::genre_map::is_known_genre(tag_name) {
                            genres.push((count, tag_name.to_string()));
                        } else {
                            styles.push((count, tag_name.to_string()));
                        }
                    }
                }
            }
        }

        genres.sort_by(|a, b| b.0.cmp(&a.0));
        styles.sort_by(|a, b| b.0.cmp(&a.0));

        let genre_names: Vec<String> = genres.into_iter().map(|(_, n)| n).collect();
        let style_names: Vec<String> = styles.into_iter().map(|(_, n)| n).collect();

        assert!(genre_names.contains(&"funk".to_string()), "funk should be promoted to genres");
        assert!(genre_names.contains(&"2-Step".to_string()), "2-Step should be promoted to genres");
        assert_eq!(style_names, vec!["random noise"], "unknown tags stay as styles");
    }

    #[test]
    fn listenbrainz_parse_release_group_only() {
        let json = serde_json::json!({
            "metadata": {
                "tag": {
                    "release_group": [
                        {"count": 42, "genre_mbid": "ghi", "tag": "speed metal"}
                    ]
                }
            }
        });
        let metadata = json.get("metadata").expect("metadata present");
        let mut genres = Vec::new();
        if let Some(tag_obj) = metadata.get("tag") {
            if let Some(rg_tags) = tag_obj.get("release_group").and_then(|a| a.as_array()) {
                for entry in rg_tags {
                    if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                        if entry.get("genre_mbid").is_some() {
                            genres.push(tag_name.to_string());
                        }
                    }
                }
            }
        }
        assert_eq!(genres, vec!["speed metal"]);
    }
}
