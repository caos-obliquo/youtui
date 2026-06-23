use crate::{util, AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

/// Provider for metal-api.dev - an approved REST API for Encyclopaedia Metallum.
/// Priority 5 (highest) - catches metal bands before any other provider.
/// API endpoint: https://metal-api.dev/
/// API is approved by Metal Archives but not officially supported.
pub struct MetalApiProvider;

impl MetalApiProvider {
    pub fn new() -> Self {
        Self
    }
}

const METAL_API_BASE: &str = "https://metal-api.dev";

impl MetadataProvider for MetalApiProvider {
    fn priority(&self) -> u8 {
        5
    }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        Box::pin(do_lookup(artist, title, client))
    }
}

async fn do_lookup(
    artist: &str,
    title: &str,
    client: &reqwest::Client,
) -> Option<ValidatedMetadata> {
    // Search for the band by name
    let band_name = artist.trim();
    if band_name.is_empty() {
        return None;
    }

    let search_url = format!(
        "{}/search/bands/name/{}",
        METAL_API_BASE,
        crate::util::urlencoding(band_name)
    );

    let search_resp = client
        .get(&search_url)
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;

    if !search_resp.status().is_success() {
        tracing::debug!("Metal-API search returned {}", search_resp.status());
        return None;
    }

    #[derive(serde::Deserialize)]
    struct BandSearchResult {
        id: Option<String>,
        #[allow(dead_code)]
        name: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct BandSearchResponse(Vec<BandSearchResult>);

    let bands: BandSearchResponse = search_resp.json().await.ok()?;
    let band = bands.0.first()?;
    let band_id = band.id.as_ref()?;

    // Get band details (discography)
    let band_url = format!("{}/bands/{}", METAL_API_BASE, band_id);
    let band_resp = client
        .get(&band_url)
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;

    if !band_resp.status().is_success() {
        return None;
    }

    // Parse band response looking for matching album
    #[derive(serde::Deserialize)]
    struct Song {
        #[allow(dead_code)]
        number: Option<String>,
        name: Option<String>,
        #[allow(dead_code)]
        length: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct Album {
        #[allow(dead_code)]
        id: Option<String>,
        name: Option<String>,
        #[allow(dead_code)]
        #[serde(rename = "type")]
        album_type: Option<String>,
        release_date: Option<String>,
        songs: Option<Vec<Song>>,
    }

    #[derive(serde::Deserialize)]
    struct BandDetail {
        #[allow(dead_code)]
        id: Option<String>,
        #[allow(dead_code)]
        name: Option<String>,
        #[allow(dead_code)]
        discography: Option<Vec<Album>>,
    }

    let detail: BandDetail = band_resp.json().await.ok()?;
    let clean_title = util::norm_for_lfm(title);
    let album_name = detail.discography.as_ref().and_then(|albums| {
        // Try to find matching album by song title match
        let matching_album = albums.iter().find(|a| {
            a.songs.as_ref().is_some_and(|songs| {
                songs.iter().any(|s| {
                    s.name
                        .as_ref()
                        .is_some_and(|n| util::norm_for_lfm(n) == clean_title)
                })
            })
        });
        // Fallback: try to match by album name containing a year tag
        matching_album.or_else(|| {
            albums.iter().find(|a| {
                a.name
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase().contains(&artist.to_lowercase()))
            })
        })
    });

    match album_name {
        Some(album) => {
            let year = album
                .release_date
                .as_ref()
                .and_then(|d| d.split(|c: char| !c.is_ascii_digit()).find(|p| p.len() == 4))
                .map(|s| s.to_string());

            let album_tracks: Vec<AlbumTrack> = album
                .songs
                .as_ref()
                .map(|songs| {
                    songs
                        .iter()
                        .filter_map(|s| {
                            let track_title = s.name.as_ref()?;
                            let dur = s.length.as_ref().and_then(|l| {
                                let parts: Vec<&str> = l.split(':').collect();
                                if parts.len() == 2 {
                                    let mins: f64 = parts[0].parse().ok()?;
                                    let secs: f64 = parts[1].parse().ok()?;
                                    Some(mins * 60.0 + secs)
                                } else if parts.len() == 1 {
                                    parts[0].parse::<f64>().ok()
                                } else {
                                    None
                                }
                            });
                            Some(AlbumTrack {
                                title: track_title.clone(),
                                duration_secs: dur.unwrap_or(0.0),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            tracing::info!(
                "Metal-API resolved: album={:?}, year={:?}, tracks={}",
                album.name,
                year,
                album_tracks.len()
            );

            Some(ValidatedMetadata {
                artist: Some(detail.name.unwrap_or_else(|| artist.to_string())),
                album: album.name.clone(),
                year,
                track_no: None,
                album_tracks,
                genres: vec![],
                styles: vec![],
            })
        }
        None => {
            tracing::debug!("Metal-API: no matching album found for {}", title);
            None
        }
    }
}
