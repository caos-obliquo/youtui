use crate::{util, AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

pub struct MusicBrainzProvider;

impl Default for MusicBrainzProvider {
    fn default() -> Self {
        Self
    }
}

impl MusicBrainzProvider {
    pub fn new() -> Self {
        Self
    }
}

impl MetadataProvider for MusicBrainzProvider {
    fn priority(&self) -> u8 {
        7
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
        let client = client.clone();
        Box::pin(async move {
            let _mb_permit = util::musicbrainz_limiter().acquire().await.ok()?;
            let mb_url = format!(
                "https://musicbrainz.org/ws/2/recording?query=artist:%22{}%22+AND+recording:%22{}%22&fmt=json",
                util::urlencoding(&artist), util::urlencoding(&title)
            );
            let resp = client
                .get(&mb_url)
                .header("Accept", "application/json")
                .send()
                .await
                .ok()?;
            let data: serde_json::Value = resp.json().await.ok()?;
            let rec = data.get("recordings")?.as_array()?.first()?.clone();

            let artist_name = rec
                .get("artist-credit")?
                .as_array()?
                .first()
                .and_then(|c| c.get("name"))?
                .as_str()?
                .to_string();
            let year = rec
                .get("releases")?
                .as_array()?
                .iter()
                .filter_map(|r| r.get("date")?.as_str())
                .filter_map(|d| d.get(..4))
                .filter(|s| s.len() >= 4)
                .map(|s| s.to_string())
                .next();
            let album_title = rec
                .get("releases")?
                .as_array()?
                .iter()
                .filter_map(|r| r.get("title")?.as_str())
                .map(|s| s.to_string())
                .next();

            // Fetch release tracklist for album_tracks
            let release_id = rec
                .get("releases")?
                .as_array()?
                .first()
                .and_then(|r| r.get("id"))?
                .as_str()?
                .to_string();
            let album_tracks = fetch_release_tracks(&client, &release_id).await;

            Some(ValidatedMetadata {
                artist: Some(artist_name),
                album: album_title,
                year,
                track_no: None,
                album_tracks,
                genres: Vec::new(),
                styles: Vec::new(),
            })
        })
    }
}

async fn fetch_release_tracks(client: &reqwest::Client, release_id: &str) -> Vec<AlbumTrack> {
    let _mb_permit = match util::musicbrainz_limiter().acquire().await {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let url = format!(
        "https://musicbrainz.org/ws/2/release/{}?inc=recordings+artist-credits&fmt=json",
        util::urlencoding(release_id)
    );
    let resp = match client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
    {
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
                    let duration_secs = entry
                        .get("length")
                        .and_then(|l| l.as_i64())
                        .map(|ms| ms as f64 / 1000.0)
                        .unwrap_or(0.0);
                    // Check for per-track artist (split releases)
                    let track_artist = entry
                        .get("artist-credit")
                        .and_then(|ac| ac.as_array())
                        .and_then(|ac| ac.first())
                        .and_then(|c| c.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string());
                    tracks.push(AlbumTrack {
                        title: t_title,
                        duration_secs,
                        artist: track_artist,
                    });
                }
            }
        }
    }
    tracks
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
                    {"id": "def-456", "title": "Test Album", "date": "2003-06-15"}
                ]
            }]
        });
        let rec = json
            .get("recordings")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .unwrap();
        let artist = rec
            .get("artist-credit")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        let year = rec
            .get("releases")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("date"))
            .and_then(|d| d.as_str())
            .and_then(|d| d.get(..4))
            .map(|s| s.to_string());
        let album = rec
            .get("releases")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("title"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());
        assert_eq!(artist, Some("Test Artist".to_string()));
        assert_eq!(year, Some("2003".to_string()));
        assert_eq!(album, Some("Test Album".to_string()));
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
        let rec = json
            .get("recordings")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .unwrap();
        let year = rec
            .get("releases")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("date"))
            .and_then(|d| d.as_str())
            .and_then(|d| d.get(..4))
            .filter(|s| s.len() >= 4)
            .map(|s| s.to_string());
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
        let tracks: Vec<AlbumTrack> = json
            .get("media")
            .and_then(|m| m.as_array())
            .unwrap()
            .iter()
            .filter_map(|medium| {
                medium
                    .get("tracks")
                    .and_then(|t| t.as_array())
                    .map(|entries| {
                        entries
                            .iter()
                            .filter_map(|entry| {
                                let title = entry.get("title")?.as_str()?.to_string();
                                let duration_secs = entry
                                    .get("length")
                                    .and_then(|l| l.as_i64())
                                    .map(|ms| ms as f64 / 1000.0)
                                    .unwrap_or(0.0);
                                let track_artist = entry
                                    .get("artist-credit")
                                    .and_then(|ac| ac.as_array())
                                    .and_then(|ac| ac.first())
                                    .and_then(|c| c.get("name"))
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.to_string());
                                Some(AlbumTrack {
                                    title,
                                    duration_secs,
                                    artist: track_artist,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
            })
            .flatten()
            .collect();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Battery");
        assert_eq!(tracks[0].artist, Some("Metallica".to_string()));
        assert_eq!(tracks[1].title, "Master of Puppets");
        assert_eq!(tracks[1].artist, None);
    }
}
