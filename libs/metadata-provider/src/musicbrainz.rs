use crate::util;
use crate::{AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;
use std::sync::{Arc, Mutex};

pub struct MusicBrainzProvider {
    client_id: Option<String>,
    client_secret: Option<String>,
    /// (access_token, refresh_token) shared for auto-refresh write-back
    tokens: Arc<Mutex<(Option<String>, Option<String>)>>,
}

impl MusicBrainzProvider {
    pub fn new(
        client_id: Option<String>,
        client_secret: Option<String>,
        bearer_token: Option<String>,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            tokens: Arc::new(Mutex::new((bearer_token, None))),
        }
    }

    /// Attempt OAuth2 token refresh using refresh_token + client credentials.
    /// Returns new access_token on success, None on failure.
    async fn try_refresh_token(
        client: &reqwest::Client,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> Option<String> {
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];
        let resp = client
            .post("https://musicbrainz.org/oauth2/token")
            .form(&params)
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let data: serde_json::Value = resp.json().await.ok()?;
        data.get("access_token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    }
}

impl MetadataProvider for MusicBrainzProvider {
    fn priority(&self) -> u8 { 7 }
    fn name(&self) -> &'static str { "MusicBrainz" }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        _album: Option<&'a str>,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        let artist = artist.to_string();
        let title = title.to_string();
        let client = client.clone();
        let tokens = Arc::clone(&self.tokens);
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        Box::pin(async move {
            let _permit = util::musicbrainz_limiter().acquire().await.ok()?;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            let initial_token = tokens.lock().unwrap().0.clone();

            let mb_url = format!(
                "https://musicbrainz.org/ws/2/recording?query=artist:%22{}%22+AND+recording:%22{}%22&fmt=json",
                util::urlencoding(&artist), util::urlencoding(&title)
            );
            let mut req = client.get(&mb_url).header("Accept", "application/json");
            if let Some(ref token) = initial_token {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            let resp = match req.send().await {
                Ok(r) => r,
                Err(_) => return None,
            };

            // Auto-refresh on 401: try refresh_token if available
            if resp.status().as_u16() == 401 {
                let refresh_token = tokens.lock().unwrap().1.clone();
                if let (Some(rt), Some(cid), Some(cs)) = (&refresh_token, &client_id, &client_secret) {
                    if let Some(new_token) = Self::try_refresh_token(&client, cid, cs, rt).await {
                        // Drop guard before await - MutexGuard is not Send
                        {
                            let mut guard = tokens.lock().unwrap();
                            guard.0 = Some(new_token.clone());
                        }
                        // Retry original request with new token
                        let mut retry = client.get(&mb_url).header("Accept", "application/json");
                        retry = retry.header("Authorization", format!("Bearer {}", new_token));
                        let retry_resp = match retry.send().await {
                            Ok(r) => r,
                            Err(_) => return None,
                        };
                        if !retry_resp.status().is_success() {
                            return None;
                        }
                        let data: serde_json::Value = match retry_resp.json().await {
                            Ok(d) => d,
                            Err(_) => return None,
                        };
                        return Self::parse_recording_response(&client, &data, &tokens, &_permit).await;
                    }
                }
                return None;
            }

            let data: serde_json::Value = match resp.json().await {
                Ok(d) => d,
                Err(_) => return None,
            };
            Self::parse_recording_response(&client, &data, &tokens, &_permit).await
        })
    }
}

impl MusicBrainzProvider {
    /// Parse recording JSON response into ValidatedMetadata with MBID capture.
    async fn parse_recording_response(
        client: &reqwest::Client,
        data: &serde_json::Value,
        tokens: &Arc<Mutex<(Option<String>, Option<String>)>>,
        _permit: &tokio::sync::SemaphorePermit<'_>,
    ) -> Option<ValidatedMetadata> {
        let rec = data.get("recordings")?.as_array()?.first()?.clone();

        let artist_name = rec.get("artist-credit")?.as_array()?.first()
            .and_then(|c| c.get("name"))?.as_str()?.to_string();
        let year = rec.get("releases")?.as_array()?.iter()
            .filter_map(|r| r.get("date")?.as_str())
            .filter_map(|d| d.get(..4)).filter(|s| s.len() >= 4)
            .map(|s| s.to_string()).next();
        let album_title = rec.get("releases")?.as_array()?.iter()
            .filter_map(|r| r.get("title")?.as_str())
            .map(|s| s.to_string()).next();

        // Fetch release tracklist
        let release_id = rec.get("releases")?.as_array()?.first()
            .and_then(|r| r.get("id"))?.as_str()?.to_string();
        let bearer = tokens.lock().unwrap().0.clone();
        let album_tracks = fetch_release_tracks(client, &release_id, bearer.clone(), _permit).await;

        // Capture release_group_id for CAA lookups
        let release_group_id = rec.get("releases")?.as_array()?.first()
            .and_then(|r| r.get("release-group"))
            .and_then(|rg| rg.get("id"))
            .and_then(|id| id.as_str())
            .map(|s| s.to_string());
        let (genres, styles) = if let Some(ref rg_id) = release_group_id {
            fetch_release_group_genres(client, rg_id, bearer.clone(), _permit).await
        } else {
            (Vec::new(), Vec::new())
        };

        Some(ValidatedMetadata {
            artist: Some(artist_name),
            album: album_title,
            year,
            track_no: None,
            album_tracks,
            genres,
            styles,
            musicbrainz_release_group_id: release_group_id,
            subgenres: Vec::new(),
            genre_paths: Vec::new(),
            descriptors: Vec::new(),
        })
    }
}

async fn fetch_release_tracks<'a>(
    client: &reqwest::Client,
    release_id: &'a str,
    bearer_token: Option<String>,
    _permit: &'a tokio::sync::SemaphorePermit<'_>,
) -> Vec<AlbumTrack> {
    let url = format!(
        "https://musicbrainz.org/ws/2/release/{}?inc=recordings+artist-credits&fmt=json",
        util::urlencoding(release_id)
    );
    let mut req = client.get(&url).header("Accept", "application/json");
    if let Some(ref token) = bearer_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let data: serde_json::Value = match resp.json().await {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut tracks = Vec::new();
    if let Some(media) = data.get("media").and_then(|m| m.as_array()) {
        for medium in media {
            if let Some(entries) = medium.get("tracks").and_then(|t| t.as_array()) {
                for entry in entries {
                    let t_title = match entry.get("title").and_then(|t| t.as_str()) {
                        Some(s) => s.to_string(),
                        None => continue,
                    };
                    let duration_secs = entry.get("length").and_then(|l| l.as_i64())
                        .map(|ms| ms as f64 / 1000.0)
                        .unwrap_or(0.0);
                    // Check for per-track artist (split releases)
                    let track_artist = entry.get("artist-credit").and_then(|ac| ac.as_array())
                        .and_then(|ac| ac.first())
                        .and_then(|c| c.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string());
                    tracks.push(AlbumTrack { title: t_title, duration_secs, artist: track_artist });
                }
            }
        }
    }
    tracks
}

async fn fetch_release_group_genres<'a>(
    client: &reqwest::Client,
    release_group_id: &'a str,
    bearer_token: Option<String>,
    _permit: &'a tokio::sync::SemaphorePermit<'_>,
) -> (Vec<String>, Vec<String>) {
    let url = format!(
        "https://musicbrainz.org/ws/2/release-group/{}?inc=genres&fmt=json",
        util::urlencoding(release_group_id)
    );
    let mut req = client.get(&url).header("Accept", "application/json");
    if let Some(ref token) = bearer_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(_) => return (Vec::new(), Vec::new()),
    };
    let data: serde_json::Value = match resp.json().await {
        Ok(d) => d,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let mut genres = Vec::new();
    if let Some(arr) = data.get("genres").and_then(|g| g.as_array()) {
        for g in arr {
            if let Some(name) = g.get("name").and_then(|n| n.as_str()) {
                genres.push(name.to_string());
            }
        }
    }

    let genre_set: std::collections::HashSet<String> =
        genres.iter().map(|g| g.to_lowercase()).collect();
    let mut styles = Vec::new();
    if let Some(arr) = data.get("tags").and_then(|t| t.as_array()) {
        for t in arr {
            if let Some(name) = t.get("name").and_then(|n| n.as_str()) {
                if !genre_set.contains(&name.to_lowercase()) {
                    styles.push(name.to_string());
                }
            }
        }
    }

    (genres, styles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_musicbrainz_recording() {
        let json = serde_json::json!({
            "recordings": [{
                "id": "abc-123",
                "title": "Test Song",
                "artist-credit": [{"name": "Test Artist"}],
                "releases": [
                    {"id": "def-456", "title": "Test Album", "date": "2003-06-15",
                     "release-group": {"id": "rg-789"}}
                ]
            }]
        });
        let rec = json.get("recordings").and_then(|a| a.as_array()).and_then(|a| a.first()).unwrap();
        let artist = rec.get("artist-credit").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|c| c.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string());
        let year = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|r| r.get("date")).and_then(|d| d.as_str()).and_then(|d| d.get(..4)).map(|s| s.to_string());
        let album = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|r| r.get("title")).and_then(|t| t.as_str()).map(|s| s.to_string());
        let rg_id = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|r| r.get("release-group")).and_then(|rg| rg.get("id"))
            .and_then(|id| id.as_str()).map(|s| s.to_string());
        assert_eq!(artist, Some("Test Artist".to_string()));
        assert_eq!(year, Some("2003".to_string()));
        assert_eq!(album, Some("Test Album".to_string()));
        assert_eq!(rg_id, Some("rg-789".to_string()));
    }

    #[test]
    fn parse_musicbrainz_short_date_rejected() {
        let json = serde_json::json!({
            "recordings": [{
                "id": "abc-123",
                "title": "Test Song",
                "artist-credit": [{"name": "Test Artist"}],
                "releases": [
                    {"id": "def-456", "title": "Test Album", "date": "07"}
                ]
            }]
        });
        let rec = json.get("recordings").and_then(|a| a.as_array()).and_then(|a| a.first()).unwrap();
        let year = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|r| r.get("date")).and_then(|d| d.as_str())
            .and_then(|d| d.get(..4)).filter(|s| s.len() >= 4).map(|s| s.to_string());
        assert_eq!(year, None, "Short date '07' should be rejected");
    }

    #[test]
    fn parse_release_tracks() {
        let json = serde_json::json!({
            "media": [{
                "tracks": [
                    {
                        "position": 1,
                        "title": "Battery",
                        "length": 315000,
                        "artist-credit": [{"name": "Metallica"}]
                    },
                    {
                        "position": 2,
                        "title": "Master of Puppets",
                        "length": 515000
                    }
                ]
            }]
        });
        let tracks: Vec<AlbumTrack> = json.get("media").and_then(|m| m.as_array()).unwrap()
            .iter().filter_map(|medium| {
                medium.get("tracks").and_then(|t| t.as_array()).map(|entries| {
                    entries.iter().filter_map(|entry| {
                        let title = entry.get("title")?.as_str()?.to_string();
                        let duration_secs = entry.get("length").and_then(|l| l.as_i64())
                            .map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);
                        let track_artist = entry.get("artist-credit").and_then(|ac| ac.as_array())
                            .and_then(|ac| ac.first())
                            .and_then(|c| c.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                        Some(AlbumTrack { title, duration_secs, artist: track_artist })
                    }).collect::<Vec<_>>()
                })
            }).flatten().collect();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Battery");
        assert_eq!(tracks[0].artist, Some("Metallica".to_string()));
        assert_eq!(tracks[1].title, "Master of Puppets");
        assert_eq!(tracks[1].artist, None);
    }
}
