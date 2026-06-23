use anyhow::Result;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

const PORT: u16 = 5000;
const MA_BASE: &str = "https://www.metal-archives.com";

struct AppState {
    browser: Arc<Mutex<Option<Browser>>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Metal Archives Rust proxy on port {}", PORT);

    let browser_state: Arc<Mutex<Option<Browser>>> = match spawn_browser().await {
        Ok(b) => { info!("Chrome ready"); Arc::new(Mutex::new(Some(b))) }
        Err(e) => { warn!("Chrome unavailable: {}", e); Arc::new(Mutex::new(None)) }
    };

    let app_state = Arc::new(AppState { browser: browser_state });
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    info!("Listening on http://0.0.0.0:{}", PORT);

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = app_state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, state).await {
                error!("[{}] handler error: {}", addr, e);
            }
        });
    }
}

async fn spawn_browser() -> Result<Browser> {
    let chrome = find_chrome();
    let builder = BrowserConfig::builder().no_sandbox();
    let builder = if let Some(ref path) = chrome {
        info!("Using browser at: {}", path.display());
        builder.chrome_executable(path)
    } else {
        warn!("No Chrome/Chromium found, trying auto-detect");
        builder
    };
    let config = builder
        .build()
        .map_err(|e| anyhow::anyhow!("Browser config: {}", e))?;
    let (browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move { while let Some(_) = handler.next().await {} });
    Ok(browser)
}

fn find_chrome() -> Option<std::path::PathBuf> {
    for name in &["chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "chrome"] {
        if let Ok(path) = std::process::Command::new(name).arg("--version").output() {
            if path.status.success() {
                return Some(std::path::PathBuf::from(name));
            }
        }
    }
    None
}

async fn handle_client(stream: tokio::net::TcpStream, state: Arc<AppState>) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let path = parts[1];

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).await?;
        if header.trim().is_empty() { break; }
        if let Some(val) = header.strip_prefix("Content-Length:") {
            content_length = val.trim().parse().unwrap_or(0);
        } else if let Some(val) = header.strip_prefix("content-length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
    }

    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body).await?;
    }

    let response = handle_request(path, &state).await;
    let mut writer = reader.into_inner();
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn handle_request(path: &str, state: &Arc<AppState>) -> String {
    let query = path.split('?').nth(1).unwrap_or("");
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned().collect();

    match path.split('?').next().unwrap_or("") {
        "/ping" => json_ok(&serde_json::json!({"status": "ok"})),
        "/search" => cmd_search(state, &params).await,
        "/album" => cmd_album(state, &params).await,
        _ => json_ok(&serde_json::json!({"error": "unknown"})),
    }
}

fn json_ok(data: &serde_json::Value) -> String {
    let body = serde_json::to_string(data).unwrap_or_default();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
        body.len(), body
    )
}

fn json_err(msg: &str) -> String {
    json_ok(&serde_json::json!({"error": msg}))
}

async fn cmd_search(state: &Arc<AppState>, params: &HashMap<String, String>) -> String {
    let artist = params.get("artist").map(|s| s.as_str()).unwrap_or("");
    let album = params.get("album").map(|s| s.as_str()).unwrap_or("");
    if artist.is_empty() {
        return json_ok(&serde_json::json!({"results": []}));
    }

    let url = format!(
        "{}/search/ajax-advanced/searching/albums/?sEcho=1&iColumns=4&exactBandMatch=1&bandName={}{}",
        MA_BASE, urlencode(artist),
        if album.is_empty() { String::new() } else { format!("&releaseTitle={}", urlencode(album)) }
    );

    let html = match fetch_page(state, &url).await {
        Some(h) => h,
        None => return json_err("Failed to fetch search page"),
    };

    let results = parse_search_results(&html);
    json_ok(&serde_json::json!({"results": results}))
}

async fn cmd_album(state: &Arc<AppState>, params: &HashMap<String, String>) -> String {
    let album_url = match params.get("url") {
        Some(u) => u,
        None => return json_ok(&serde_json::json!({"error": "no url"})),
    };
    let html = match fetch_page(state, album_url).await {
        Some(h) => h,
        None => return json_err("Failed to fetch album page"),
    };

    let doc = Html::parse_document(&html);
    let sel_h1 = Selector::parse("h1.album_name").unwrap();
    let sel_h2 = Selector::parse("h2.band_name a").unwrap();

    let album_name = doc.select(&sel_h1).next()
        .map(|e| e.text().collect::<String>().trim().to_string()).unwrap_or_default();
    let artist_name = doc.select(&sel_h2).next()
        .map(|e| e.text().collect::<String>().trim().to_string()).unwrap_or_default();

    let year = Regex::new(r"(?i)Release date:.*?(\d{4})").unwrap()
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    let album_type = extract_dd_after_dt(&doc, "Type:");
    let tracks = extract_tracks(&doc);

    json_ok(&serde_json::json!({
        "album": album_name,
        "artist": artist_name,
        "year": year,
        "metal_archives_type": album_type,
        "tracks": tracks,
    }))
}

fn extract_dd_after_dt(doc: &Html, label: &str) -> String {
    // Use regex on the HTML to find dt/dd pairs — avoids NodeRef API issues
    let re = Regex::new(&format!(
        r"(?i)<dt[^>]*>[^<]*{}[^<]*</dt>\s*<dd[^>]*>(.*?)</dd>",
        regex::escape(label)
    )).unwrap();
    if let Some(caps) = re.captures(&doc.html()) {
        let inner = caps.get(1).unwrap().as_str();
        return Regex::new(r"<[^>]*>").unwrap().replace_all(inner, "")
            .trim().to_string();
    }
    String::new()
}

fn extract_tracks(doc: &Html) -> Vec<serde_json::Value> {
    let mut tracks = Vec::new();
    let row_sel = Selector::parse("table.table_lyrics tr").unwrap();
    let td_sel = Selector::parse("td").unwrap();

    for row in doc.select(&row_sel) {
        let inner = row.inner_html();
        if !inner.contains("wrapWords") { continue; }

        let cells: Vec<_> = row.select(&td_sel).collect();
        if cells.len() >= 3 {
            let title = cells[1].text().collect::<String>().trim().to_string();
            let length = cells[2].text().collect::<String>().trim().to_string();
            if !title.is_empty() {
                tracks.push(serde_json::json!({"title": title, "length": length}));
            }
        }
    }
    tracks
}

async fn fetch_page(state: &Arc<AppState>, url: &str) -> Option<String> {
    let mut guard = state.browser.lock().await;
    let browser = guard.as_mut()?;
    let page = browser.new_page("about:blank").await.ok()?;
    page.goto(url).await.ok()?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    page.content().await.ok()
}

fn parse_search_results(html: &str) -> Vec<serde_json::Value> {
    let re = Regex::new(r#""aaData":(\[.*?\]),""#).unwrap();
    if let Some(caps) = re.captures(html) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(caps.get(1).unwrap().as_str()) {
            if let Some(arr) = data.as_array() {
                return arr.iter().filter_map(|row| {
                    let r = row.as_array()?;
                    if r.len() < 4 { return None; }
                    let artist = strip_html(r[0].as_str().unwrap_or(""));
                    let album_html = r[1].as_str().unwrap_or("");
                    let album = strip_html(album_html);
                    let url_re = Regex::new(r#"href="([^"]+)""#).unwrap();
                    let album_url = url_re.captures(album_html)
                        .and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("");
                    let date_raw = r[3].as_str().unwrap_or("");
                    let year = Regex::new(r"\d{4}").unwrap().find(date_raw)
                        .map(|m| m.as_str()).unwrap_or("");
                    Some(serde_json::json!({"artist": artist, "album": album, "url": album_url, "year": year}))
                }).collect();
            }
        }
    }
    vec![]
}

fn strip_html(s: &str) -> String {
    Regex::new(r"<[^>]*>").unwrap().replace_all(s, "").trim().to_string()
}

fn urlencode(s: &str) -> String {
    s.as_bytes().iter().map(|&c| match c {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (c as char).to_string(),
        b' ' => "+".to_string(),
        _ => format!("%{:02X}", c),
    }).collect()
}
