use crate::{util, AlbumTrack, MetadataProvider, ValidatedMetadata};
use futures::future::BoxFuture;
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static HTML_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static HREF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"href="([^"]+)""#).unwrap());
static YEAR_COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!--\s*(\d{4})").unwrap());
static YEAR_DIGITS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{4}").unwrap());
static GENRE_DT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<dt[^>]*>Genre:</dt>\s*<dd[^>]*>(.*?)</dd>").unwrap());

/// Provider for Encyclopaedia Metallum (Metal Archives) data.
/// Priority 5 (highest) - catches metal bands before any other provider.
///
/// Tries sources in order:
///   1. https://metal-api.dev/ - approved community REST API (primary)
///   2. http://localhost:5000/ - optional Rust proxy (bypasses Cloudflare via
///      Chromium)
///
/// The Rust proxy is in libs/metal-proxy/ - run with:
///   cargo run --release -p metal-proxy
/// (requires Chromium installed)
pub struct MetalApiProvider;

impl Default for MetalApiProvider {
    fn default() -> Self {
        Self
    }
}

impl MetalApiProvider {
    pub fn new() -> Self {
        Self
    }
}

const METAL_API: &str = "https://metal-api.dev";
const LOCAL_PROXY: &str = "http://localhost:5000";

impl MetadataProvider for MetalApiProvider {
    fn priority(&self) -> u8 {
        5
    }

    fn lookup<'a>(
        &'a self,
        artist: &'a str,
        title: &'a str,
        _album: Option<&'a str>,
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
    let band = artist.trim();
    if band.is_empty() {
        return None;
    }

    // 1. Try direct MA access with cf_clearance cookie (env or file). Fastest &
    //    most reliable - metal-api.dev is down, proxy requires setup.
    let result = try_direct_ma(band, title).await;
    if result.is_some() {
        return result;
    }

    tracing::debug!("direct MA access unavailable, trying local proxy");

    // 2. Try local Chromium proxy (bypasses Cloudflare via headless browser)
    let result = try_local_proxy(band, title, client).await;
    if result.is_some() {
        return result;
    }

    tracing::debug!("local proxy unavailable, trying metal-api.dev");

    // 3. Try metal-api.dev (approved community REST API - currently returning 500)
    try_metal_api(band, title, client).await
}

async fn try_metal_api(
    artist: &str,
    title: &str,
    client: &reqwest::Client,
) -> Option<ValidatedMetadata> {
    let search_url = format!(
        "{}/search/bands/name/{}",
        METAL_API,
        crate::util::urlencoding(artist)
    );
    let search_resp = client
        .get(&search_url)
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;
    if !search_resp.status().is_success() {
        tracing::debug!("metal-api.dev returned {}", search_resp.status());
        return None;
    }

    #[derive(serde::Deserialize)]
    struct BandSearchResult {
        _id: Option<String>,
        _name: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct BandSearchResponse(Vec<BandSearchResult>);

    let band = search_resp
        .json::<BandSearchResponse>()
        .await
        .ok()?
        .0
        .into_iter()
        .next()?;
    let _band_id = band._id.as_ref()?;

    let band_url = format!("{}/bands/{}", METAL_API, _band_id);
    let band_resp = client
        .get(&band_url)
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;
    if !band_resp.status().is_success() {
        return None;
    }

    #[derive(serde::Deserialize)]
    struct Song {
        _number: Option<String>,
        name: Option<String>,
        length: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Album {
        _id: Option<String>,
        name: Option<String>,
        release_date: Option<String>,
        songs: Option<Vec<Song>>,
    }
    #[derive(serde::Deserialize)]
    struct BandDetail {
        _id: Option<String>,
        name: Option<String>,
        discography: Option<Vec<Album>>,
    }

    let detail: BandDetail = band_resp.json().await.ok()?;
    let clean_title = util::norm_for_lfm(title);
    let album = detail.discography.as_ref().and_then(|albums| {
        albums
            .iter()
            .find(|a| {
                a.songs.as_ref().is_some_and(|s| {
                    s.iter().any(|s| {
                        s.name
                            .as_deref()
                            .is_some_and(|n| util::norm_for_lfm(n) == clean_title)
                    })
                })
            })
            .or_else(|| {
                albums.iter().find(|a| {
                    a.name
                        .as_deref()
                        .is_some_and(|n| n.to_lowercase().contains(&artist.to_lowercase()))
                })
            })
    })?;

    let year = album.release_date.as_ref().and_then(|d| {
        d.split(|c: char| !c.is_ascii_digit())
            .find(|p| p.len() == 4)
            .map(String::from)
    });
    let album_tracks: Vec<AlbumTrack> = album
        .songs
        .as_ref()
        .map(|songs| {
            songs
                .iter()
                .filter_map(|s| {
                    let t = s.name.as_ref()?;
                    let dur = s.length.as_ref().and_then(|l| {
                        let p: Vec<&str> = l.split(':').collect();
                        if p.len() == 2 {
                            Some(p[0].parse::<f64>().ok()? * 60.0 + p[1].parse::<f64>().ok()?)
                        } else {
                            p[0].parse::<f64>().ok()
                        }
                    });
                    Some(AlbumTrack {
                        title: t.clone(),
                        duration_secs: dur.unwrap_or(0.0),
                        artist: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    tracing::info!(
        "metal-api.dev resolved: album={:?}, year={:?}, tracks={}",
        album.name,
        year,
        album_tracks.len()
    );
    Some(ValidatedMetadata {
        artist: detail.name.or_else(|| Some(artist.to_string())),
        album: album.name.clone(),
        year,
        track_no: None,
        album_tracks,
        genres: vec![],
        styles: vec![],
    })
}

/// Try connecting to a local Playwright proxy
/// (scripts/metal-archives-proxy.py).
async fn try_local_proxy(
    artist: &str,
    title: &str,
    client: &reqwest::Client,
) -> Option<ValidatedMetadata> {
    // Check if proxy is running
    let ping = client
        .get(format!("{}/ping", LOCAL_PROXY))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await;
    if ping.is_err() {
        tracing::debug!("Local proxy not available at {}", LOCAL_PROXY);
        return None;
    }

    // Search albums via proxy
    let search_url = format!(
        "{}/search?artist={}&album={}",
        LOCAL_PROXY,
        crate::util::urlencoding(artist),
        crate::util::urlencoding(title)
    );
    let resp = client
        .get(&search_url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .ok()?;
    let data: serde_json::Value = resp.json().await.ok()?;
    let results = data.get("results")?.as_array()?;
    let first = results.first()?;

    let album_url = first.get("url")?.as_str()?;
    let year = first
        .get("year")
        .and_then(|y| y.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            first.get("year").and_then(|y| y.as_str()).and_then(|s| {
                s.split(|c: char| !c.is_ascii_digit())
                    .find(|p| p.len() == 4)
                    .map(String::from)
            })
        });

    if album_url.is_empty() {
        return None;
    }

    // Get album details
    let album_url_enc = crate::util::urlencoding(album_url);
    let album_resp = client
        .get(format!("{}/album?url={}", LOCAL_PROXY, album_url_enc))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .ok()?;
    let album_data: serde_json::Value = album_resp.json().await.ok()?;

    let artist_name = album_data
        .get("artist")
        .and_then(|a| a.as_str())
        .unwrap_or(artist);
    let album_name = album_data
        .get("album")
        .and_then(|a| a.as_str())
        .map(String::from);
    let year = year.or_else(|| {
        album_data
            .get("year")
            .and_then(|y| y.as_str())
            .map(String::from)
    });

    let album_tracks: Vec<AlbumTrack> = album_data
        .get("tracks")?
        .as_array()?
        .iter()
        .filter_map(|t| {
            let name = t.get("title")?.as_str()?;
            let dur = t
                .get("length")
                .and_then(|l| l.as_str())
                .and_then(|l| {
                    let p: Vec<&str> = l.split(':').collect();
                    if p.len() == 2 {
                        Some(p[0].parse::<f64>().ok()? * 60.0 + p[1].parse::<f64>().ok()?)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            Some(AlbumTrack {
                title: name.to_string(),
                duration_secs: dur,
                artist: None,
            })
        })
        .collect();

    if album_tracks.is_empty() {
        return None;
    }

    tracing::info!(
        "Local proxy resolved: album={:?}, year={:?}, tracks={}",
        album_name,
        year,
        album_tracks.len()
    );
    Some(ValidatedMetadata {
        artist: Some(normalize_artist(artist_name)),
        album: album_name,
        year,
        track_no: None,
        album_tracks,
        genres: vec![],
        styles: vec![],
    })
}

fn normalize_artist(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut chars = trimmed.chars();
    let first = chars.next().unwrap();
    // Preserve intentional lowercase (e.g. "data da morte"), only capitalize if not
    // already lowercase
    if first.is_lowercase() {
        return trimmed.to_string();
    }
    first.to_uppercase().to_string() + chars.as_str()
}

/// Try direct Metal Archives access using a cf_clearance cookie.
/// Checks MA_COOKIE env var first, then ~/.config/youtui/ma_cookie file.
async fn try_direct_ma(artist: &str, title: &str) -> Option<ValidatedMetadata> {
    let cookie = std::env::var("MA_COOKIE")
        .ok()
        .filter(|c| !c.is_empty())
        .or_else(load_cookie_file);

    let cookie_val = cookie?;
    save_cookie(&cookie_val);
    tracing::info!("Trying direct MA access with cookie");

    // Build a reqwest client that mimics a real browser
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36")
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie_val).ok()?);
            h.insert(reqwest::header::ACCEPT, "application/json, text/plain, */*".parse().ok()?);
            h.insert(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9".parse().ok()?);
            h
        })
        .build()
        .ok()?;

    // Search for ALL albums by this artist (no album filter, get all matches)
    let search_url = format!(
        "https://www.metal-archives.com/search/ajax-advanced/searching/albums/?sEcho=1&iColumns=4&exactBandMatch=1&bandName={}",
        crate::util::urlencoding(artist),
    );

    let resp = client.get(&search_url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::debug!("Direct MA search returned {}", resp.status());
        return None;
    }

    let text = resp.text().await.ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;
    let rows = data.get("aaData")?.as_array()?;
    if rows.is_empty() {
        return None;
    }

    // Find the best matching album: prefer exact album title match, else first
    // result
    let clean_title = util::norm_for_lfm(title);
    let matching_row = {
        let mut best = None;
        for row in rows {
            if let Some(r) = row.as_array() {
                if let Some(album_html) = r.get(1).and_then(|v| v.as_str()) {
                    let album_name = HTML_TAG_RE.replace_all(album_html, "");
                    let album_name = album_name.trim();
                    let nl = util::norm_for_lfm(album_name);
                    if nl.contains(&clean_title) || clean_title.contains(&nl) {
                        best = Some(row);
                        break;
                    }
                    if best.is_none() {
                        best = Some(row);
                    }
                }
            }
        }
        best
    };

    let row_arr = matching_row?.as_array()?;
    if row_arr.len() < 4 {
        return None;
    }

    let album_html = row_arr[1].as_str().unwrap_or("");
    let album_url = HREF_RE
        .captures(album_html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())?;

    let date_raw = row_arr[3].as_str().unwrap_or("");
    // MA dates come as: "<!-- 2024-01-15 -->January 15th, 2024" or "<!-- 2024
    // -->April 1st, 2024"
    let year = YEAR_COMMENT_RE
        .captures(date_raw)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            YEAR_DIGITS_RE
                .find(date_raw)
                .map(|m| m.as_str().to_string())
        });

    // Extract band URL from search result (row[0]) for genre info
    let band_html = row_arr[0].as_str().unwrap_or("");
    let band_url = HREF_RE
        .captures(band_html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    // Get album details and extract tracks (in a block so doc drops before next
    // async)
    let (album_name, artist_name, tracks) = {
        let album_resp = client.get(album_url).send().await.ok()?;
        let album_html = album_resp.text().await.ok()?;
        let doc = Html::parse_document(&album_html);

        let sel_h1 = Selector::parse("h1.album_name").ok()?;
        let sel_h2 = Selector::parse("h2.band_name a").ok()?;
        let album_name = doc
            .select(&sel_h1)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())?;
        let artist_name = doc
            .select(&sel_h2)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| artist.to_string());

        let row_sel = Selector::parse("table.table_lyrics tr").ok()?;
        let td_sel = Selector::parse("td").ok()?;
        let mut tracks = Vec::new();
        for row in doc.select(&row_sel) {
            if !row.inner_html().contains("wrapWords") {
                continue;
            }
            let cells: Vec<_> = row.select(&td_sel).collect();
            if cells.len() >= 3 {
                let title = cells[1].text().collect::<String>().trim().to_string();
                let length = cells[2].text().collect::<String>().trim().to_string();
                if !title.is_empty() {
                    let dur = if length.contains(':') {
                        let p: Vec<&str> = length.split(':').collect();
                        if p.len() == 2 {
                            Some(p[0].parse::<f64>().ok()? * 60.0 + p[1].parse::<f64>().ok()?)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    tracks.push(AlbumTrack {
                        title,
                        duration_secs: dur.unwrap_or(0.0),
                        artist: None,
                    });
                }
            }
        }
        if tracks.is_empty() {
            return None;
        }
        (album_name, artist_name, tracks)
    };

    // Now fetch band page for genres
    let mut genres: Vec<String> = Vec::new();
    if let Some(ref band_url) = band_url {
        if let Ok(band_resp) = client.get(band_url).send().await {
            if let Ok(band_html) = band_resp.text().await {
                if let Some(caps) = GENRE_DT_RE.captures(&band_html) {
                    let genre_str = HTML_TAG_RE.replace_all(caps.get(1).unwrap().as_str(), "");
                    for g in genre_str
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                    {
                        genres.push(g.to_string());
                    }
                }
            }
        }
    }

    tracing::info!(
        "Direct MA access resolved: album={:?}, year={:?}, tracks={}, genres={:?}",
        album_name,
        year,
        tracks.len(),
        genres
    );
    Some(ValidatedMetadata {
        artist: Some(normalize_artist(&artist_name)),
        album: Some(album_name),
        year,
        track_no: None,
        album_tracks: tracks,
        genres,
        styles: vec![],
    })
}

fn cookie_file_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())?;
    Some(
        std::path::PathBuf::from(home)
            .join(".config")
            .join("youtui")
            .join("ma_cookie"),
    )
}

fn load_cookie_file() -> Option<String> {
    let path = cookie_file_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Save a successful cookie to the config file for future use.
pub fn save_cookie(cookie: &str) {
    if let Some(path) = cookie_file_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&path, cookie) {
            Ok(_) => tracing::info!("MA cookie saved to {:?}", path),
            Err(e) => tracing::warn!("Failed to save MA cookie: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_artist_capitalizes_first_letter() {
        // All-lowercase preserved (intentional naming)
        assert_eq!(normalize_artist("metallica"), "metallica");
        // Already-uppercase first char stays unchanged
        assert_eq!(normalize_artist("MEGADETH"), "MEGADETH");
        // Mixed case with uppercase first char stays
        assert_eq!(normalize_artist("Iron maiden"), "Iron maiden");
    }

    #[test]
    fn test_normalize_intentional_lowercase() {
        assert_eq!(normalize_artist("data da morte"), "data da morte");
    }

    #[test]
    fn test_normalize_artist_empty_string() {
        assert_eq!(normalize_artist(""), "");
        assert_eq!(normalize_artist("  "), "");
    }

    #[test]
    fn test_html_tag_re_strips_tags() {
        let result = HTML_TAG_RE.replace_all("<b>Album Name</b>", "");
        assert_eq!(result, "Album Name");

        let result = HTML_TAG_RE.replace_all("<a href='/bands/Metallica'>Metallica</a>", "");
        assert_eq!(result, "Metallica");
    }

    #[test]
    fn test_href_re_extracts_url() {
        let input = r#"<a href="/bands/123">Link</a>"#;
        let cap = HREF_RE
            .captures(input)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str());
        assert_eq!(cap, Some("/bands/123"));

        let no_match = "no href here";
        assert!(HREF_RE.captures(no_match).is_none());
    }

    #[test]
    fn test_year_comment_re_extracts_year() {
        let input = "<!-- 2024-01-15 -->January 15th, 2024";
        let cap = YEAR_COMMENT_RE
            .captures(input)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str());
        assert_eq!(cap, Some("2024"));

        let modern = "<!-- 2025 -->April 1st, 2025";
        let cap2 = YEAR_COMMENT_RE
            .captures(modern)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str());
        assert_eq!(cap2, Some("2025"));

        let no_comment = "no year here";
        assert!(YEAR_COMMENT_RE.captures(no_comment).is_none());
    }

    #[test]
    fn test_genre_dt_re_extracts_genre() {
        let input = "<dt>Genre:</dt><dd>Thrash Metal, Death Metal</dd>";
        let cap = GENRE_DT_RE.captures(input);
        assert!(cap.is_some());
        let genre_str = HTML_TAG_RE.replace_all(cap.unwrap().get(1).unwrap().as_str(), "");
        assert_eq!(genre_str, "Thrash Metal, Death Metal");

        let no_genre = "<dt>Country:</dt><dd>USA</dd>";
        assert!(GENRE_DT_RE.captures(no_genre).is_none());
    }
}
