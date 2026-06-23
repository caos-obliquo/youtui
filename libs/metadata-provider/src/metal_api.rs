#![allow(dead_code)]
use crate::{util, AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;

/// Provider for Encyclopaedia Metallum (Metal Archives) data.
/// Priority 5 (highest) — catches metal bands before any other provider.
///
/// Tries sources in order:
///   1. https://metal-api.dev/ — approved community REST API (primary)
///   2. http://localhost:5000/ — optional Rust proxy (bypasses Cloudflare via Chromium)
///
/// The Rust proxy is in libs/metal-proxy/ — run with:
///   cargo run --release -p metal-proxy
/// (requires Chromium installed)
pub struct MetalApiProvider;

impl MetalApiProvider {
    pub fn new() -> Self { Self }
}

const METAL_API: &str = "https://metal-api.dev";
const LOCAL_PROXY: &str = "http://localhost:5000";

impl MetadataProvider for MetalApiProvider {
    fn priority(&self) -> u8 { 5 }

    fn lookup<'a>(
        &'a self, artist: &'a str, title: &'a str, client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<ValidatedMetadata>> {
        Box::pin(do_lookup(artist, title, client))
    }
}

async fn do_lookup(artist: &str, title: &str, client: &reqwest::Client) -> Option<ValidatedMetadata> {
    let band = artist.trim();
    if band.is_empty() { return None; }

    // 1. Try metal-api.dev (official approved API)
    let result = try_metal_api(band, title, client).await;
    if result.is_some() { return result; }

    tracing::debug!("metal-api.dev unavailable, trying local proxy");
    // 2. Try local Playwright proxy (optional sidecar)
    try_local_proxy(band, title, client).await
}

async fn try_metal_api(artist: &str, title: &str, client: &reqwest::Client) -> Option<ValidatedMetadata> {
    let search_url = format!("{}/search/bands/name/{}", METAL_API, crate::util::urlencoding(artist));
    let search_resp = client.get(&search_url).header("Accept", "application/json").send().await.ok()?;
    if !search_resp.status().is_success() {
        tracing::debug!("metal-api.dev returned {}", search_resp.status());
        return None;
    }

    #[derive(serde::Deserialize)]
    struct BandSearchResult { id: Option<String>, name: Option<String> }
    #[derive(serde::Deserialize)]
    struct BandSearchResponse(Vec<BandSearchResult>);

    let band = search_resp.json::<BandSearchResponse>().await.ok()?.0.into_iter().next()?;
    let band_id = band.id.as_ref()?;

    let band_url = format!("{}/bands/{}", METAL_API, band_id);
    let band_resp = client.get(&band_url).header("Accept", "application/json").send().await.ok()?;
    if !band_resp.status().is_success() { return None; }

    #[derive(serde::Deserialize)]
    struct Song { number: Option<String>, name: Option<String>, length: Option<String> }
    #[derive(serde::Deserialize)]
    struct Album { id: Option<String>, name: Option<String>, release_date: Option<String>, songs: Option<Vec<Song>> }
    #[derive(serde::Deserialize)]
    struct BandDetail { id: Option<String>, name: Option<String>, discography: Option<Vec<Album>> }

    let detail: BandDetail = band_resp.json().await.ok()?;
    let clean_title = util::norm_for_lfm(title);
    let album = detail.discography.as_ref().and_then(|albums| {
        albums.iter().find(|a| a.songs.as_ref().is_some_and(|s| s.iter().any(|s| s.name.as_deref().map_or(false, |n| util::norm_for_lfm(n) == clean_title))))
            .or_else(|| albums.iter().find(|a| a.name.as_deref().map_or(false, |n| n.to_lowercase().contains(&artist.to_lowercase()))))
    })?;

    let year = album.release_date.as_ref().and_then(|d| d.split(|c: char| !c.is_ascii_digit()).find(|p| p.len() == 4).map(String::from));
    let album_tracks: Vec<AlbumTrack> = album.songs.as_ref().map(|songs| songs.iter().filter_map(|s| {
        let t = s.name.as_ref()?;
        let dur = s.length.as_ref().and_then(|l| {
            let p: Vec<&str> = l.split(':').collect();
            if p.len() == 2 {
                Some(p[0].parse::<f64>().ok()? * 60.0 + p[1].parse::<f64>().ok()?)
            } else {
                p[0].parse::<f64>().ok()
            }
        });
        Some(AlbumTrack { title: t.clone(), duration_secs: dur.unwrap_or(0.0) })
    }).collect()).unwrap_or_default();

    tracing::info!("metal-api.dev resolved: album={:?}, year={:?}, tracks={}", album.name, year, album_tracks.len());
    Some(ValidatedMetadata {
        artist: detail.name.or_else(|| Some(artist.to_string())),
        album: album.name.clone(), year, track_no: None, album_tracks, genres: vec![], styles: vec![],
    })
}

/// Try connecting to a local Playwright proxy (scripts/metal-archives-proxy.py).
async fn try_local_proxy(artist: &str, title: &str, client: &reqwest::Client) -> Option<ValidatedMetadata> {
    // Check if proxy is running
    let ping = client.get(format!("{}/ping", LOCAL_PROXY)).timeout(std::time::Duration::from_secs(2)).send().await;
    if ping.is_err() {
        tracing::debug!("Local proxy not available at {}", LOCAL_PROXY);
        return None;
    }

    // Search albums via proxy
    let search_url = format!("{}/search?artist={}&album={}", LOCAL_PROXY, crate::util::urlencoding(artist), crate::util::urlencoding(title));
    let resp = client.get(&search_url).timeout(std::time::Duration::from_secs(30)).send().await.ok()?;
    let data: serde_json::Value = resp.json().await.ok()?;
    let results = data.get("results")?.as_array()?;
    let first = results.first()?;

    let album_url = first.get("url")?.as_str()?;
    let year = first.get("year").and_then(|y| y.as_str()).map(|s| s.to_string())
        .or_else(|| first.get("year").and_then(|y| y.as_str()).and_then(|s| s.split(|c: char| !c.is_ascii_digit()).find(|p| p.len() == 4).map(String::from)));

    if album_url.is_empty() { return None; }

    // Get album details
    let album_url_enc = crate::util::urlencoding(album_url);
    let album_resp = client.get(format!("{}/album?url={}", LOCAL_PROXY, album_url_enc)).timeout(std::time::Duration::from_secs(30)).send().await.ok()?;
    let album_data: serde_json::Value = album_resp.json().await.ok()?;

    let artist_name = album_data.get("artist").and_then(|a| a.as_str()).unwrap_or(artist);
    let album_name = album_data.get("album").and_then(|a| a.as_str()).map(String::from);
    let year = year.or_else(|| album_data.get("year").and_then(|y| y.as_str()).map(String::from));

    let album_tracks: Vec<AlbumTrack> = album_data.get("tracks")?.as_array()?.iter().filter_map(|t| {
        let name = t.get("title")?.as_str()?;
        let dur = t.get("length").and_then(|l| l.as_str()).and_then(|l| {
            let p: Vec<&str> = l.split(':').collect();
            if p.len() == 2 { Some(p[0].parse::<f64>().ok()? * 60.0 + p[1].parse::<f64>().ok()?) } else { None }
        }).unwrap_or(0.0);
        Some(AlbumTrack { title: name.to_string(), duration_secs: dur })
    }).collect();

    if album_tracks.is_empty() { return None; }

    tracing::info!("Local proxy resolved: album={:?}, year={:?}, tracks={}", album_name, year, album_tracks.len());
    Some(ValidatedMetadata {
        artist: Some(normalize_artist(artist_name)),
        album: album_name, year, track_no: None, album_tracks,
        genres: vec![], styles: vec![],
    })
}

fn normalize_artist(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() { return String::new(); }
    let mut chars = trimmed.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    first + chars.as_str()
}
