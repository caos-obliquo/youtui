use crate::util;
use crate::{MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

pub struct MusicBrainzProvider;

impl Default for MusicBrainzProvider {
    fn default() -> Self { Self }
}

impl MusicBrainzProvider {
    pub fn new() -> Self {
        Self
    }
}

impl MetadataProvider for MusicBrainzProvider {
    fn priority(&self) -> u8 { 7 }

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
            let resp = client.get(&mb_url).header("Accept", "application/json").send().await.ok()?;
            let data: serde_json::Value = resp.json().await.ok()?;
            let rec = data.get("recordings")?.as_array()?.first()?.clone();

            let artist_name = rec.get("artist-credit")?.as_array()?.first()
                .and_then(|c| c.get("name"))?.as_str()?.to_string();
            let year = rec.get("releases")?.as_array()?.first()
                .and_then(|r| r.get("date"))?.as_str()
                .and_then(|d| d.get(..4)).filter(|s| s.len() >= 4).map(|s| s.to_string());
            let album = rec.get("releases")?.as_array()?.first()
                .and_then(|r| r.get("title"))?.as_str()?.to_string();

            Some(ValidatedMetadata {
                artist: Some(artist_name),
                album: Some(album),
                year,
                track_no: None,
                album_tracks: Vec::new(),
                genres: Vec::new(),
                styles: Vec::new(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
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
        let rec = json.get("recordings").and_then(|a| a.as_array()).and_then(|a| a.first()).unwrap();
        let artist = rec.get("artist-credit").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|c| c.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string());
        let year = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|r| r.get("date")).and_then(|d| d.as_str()).and_then(|d| d.get(..4)).map(|s| s.to_string());
        let album = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|r| r.get("title")).and_then(|t| t.as_str()).map(|s| s.to_string());
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
        let rec = json.get("recordings").and_then(|a| a.as_array()).and_then(|a| a.first()).unwrap();
        let year = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
            .and_then(|r| r.get("date")).and_then(|d| d.as_str())
            .and_then(|d| d.get(..4)).filter(|s| s.len() >= 4).map(|s| s.to_string());
        assert_eq!(year, None, "Short date '07' should be rejected");
    }
}
