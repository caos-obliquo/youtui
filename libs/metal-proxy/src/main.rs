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
const CDP_PORT: u16 = 9222;
const COOKIE_FILE: &str = "ma_cookie";

fn cookie_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config").join("youtui").join(COOKIE_FILE)
}

struct AppState {
    browser: Arc<Mutex<Option<Browser>>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // If --get-cookie flag, extract from Chromium and exit
    if std::env::args().any(|a| a == "--get-cookie") {
        return cmd_get_cookie().await;
    }

    info!("Metal Archives Rust proxy on port {}", PORT);
    let browser_state = match spawn_browser().await {
        Ok(b) => { info!("Chrome ready"); Arc::new(Mutex::new(Some(b))) }
        Err(e) => { warn!("Chrome unavailable: {}", e); Arc::new(Mutex::new(None)) }
    };

    let state = Arc::new(AppState { browser: browser_state });
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    info!("Listening on http://0.0.0.0:{}", PORT);

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, state).await {
                error!("[{}] handler error: {}", addr, e);
            }
        });
    }
}

async fn spawn_browser() -> Result<Browser> {
    let tmp_dir = std::env::temp_dir().join("chromiumoxide-runner");
    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;

    let chrome = find_chrome();
    let mut builder = BrowserConfig::builder()
        .no_sandbox()
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--disable-web-security")
        .arg("--allow-running-insecure-content");
    if let Some(ref path) = chrome {
        info!("Using browser at: {}", path.display());
        builder = builder.chrome_executable(path);
    } else {
        warn!("No Chrome/Chromium found, trying auto-detect");
    }
    let config = builder
        .build()
        .map_err(|e| anyhow::anyhow!("Browser config: {}", e))?;
    let (browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move { while let Some(_) = handler.next().await {} });
    Ok(browser)
}

fn find_chrome() -> Option<std::path::PathBuf> {
    for path in &[
        "/usr/bin/chromium", "/usr/bin/chromium-browser",
        "/usr/bin/google-chrome", "/usr/bin/google-chrome-stable",
        "/usr/bin/chrome",
    ] {
        if std::path::Path::new(path).exists() {
            return Some(std::path::PathBuf::from(path));
        }
    }
    if let Ok(paths) = std::env::var("PATH") {
        for name in &["chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "chrome"] {
            for dir in paths.split(':') {
                let full = std::path::Path::new(dir).join(name);
                if full.exists() { return Some(full); }
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
    if parts.len() < 2 { return Ok(()); }
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
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}", body.len(), body)
}

fn json_err(msg: &str) -> String {
    json_ok(&serde_json::json!({"error": msg}))
}

async fn cmd_search(state: &Arc<AppState>, params: &HashMap<String, String>) -> String {
    let artist = params.get("artist").map(|s| s.as_str()).unwrap_or("");
    let album = params.get("album").map(|s| s.as_str()).unwrap_or("");
    if artist.is_empty() { return json_ok(&serde_json::json!({"results": []})); }
    let url = format!("{}/search/ajax-advanced/searching/albums/?sEcho=1&iColumns=4&exactBandMatch=1&bandName={}{}", MA_BASE, urlencode(artist),
        if album.is_empty() { String::new() } else { format!("&releaseTitle={}", urlencode(album)) });
    let html = match fetch_page(state, &url).await { Some(h) => h, None => return json_err("Failed to fetch page") };
    json_ok(&serde_json::json!({"results": parse_search_results(&html)}))
}

async fn cmd_album(state: &Arc<AppState>, params: &HashMap<String, String>) -> String {
    let album_url = match params.get("url") { Some(u) => u, None => return json_ok(&serde_json::json!({"error": "no url"})) };
    let html = match fetch_page(state, album_url).await { Some(h) => h, None => return json_err("Failed to fetch album") };
    let doc = Html::parse_document(&html);
    let album_name = extract_text(&doc, "h1.album_name");
    let artist_name = extract_text(&doc, "h2.band_name a");
    let year = Regex::new(r"(?i)Release date:.*?(\d{4})").unwrap().captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default();
    json_ok(&serde_json::json!({"album": album_name, "artist": artist_name, "year": year, "tracks": extract_tracks(&doc)}))
}

fn extract_text(doc: &Html, sel: &str) -> String {
    Selector::parse(sel).ok().and_then(|s| doc.select(&s).next()).map(|e| e.text().collect::<String>().trim().to_string()).unwrap_or_default()
}

fn extract_tracks(doc: &Html) -> Vec<serde_json::Value> {
    let row_sel = Selector::parse("table.table_lyrics tr").unwrap();
    let td_sel = Selector::parse("td").unwrap();
    doc.select(&row_sel).filter(|r| r.inner_html().contains("wrapWords")).filter_map(|row| {
        let cells: Vec<_> = row.select(&td_sel).collect();
        (cells.len() >= 3).then(|| serde_json::json!({"title": cells[1].text().collect::<String>().trim().to_string(), "length": cells[2].text().collect::<String>().trim().to_string()}))
    }).collect()
}

async fn fetch_page(state: &Arc<AppState>, url: &str) -> Option<String> {
    let mut guard = state.browser.lock().await;
    let browser = guard.as_mut()?;
    let page = browser.new_page("about:blank").await.ok()?;
    page.goto("https://www.metal-archives.com/").await.ok()?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    page.goto(url).await.ok()?;
    for _ in 0..12 {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        if let Ok(html) = page.content().await {
            if html.len() > 2000 && !html.contains("Just a moment") && !html.contains("challenge-form") {
                return Some(html);
            }
        }
    }
    page.content().await.ok()
}

fn parse_search_results(html: &str) -> Vec<serde_json::Value> {
    let re = Regex::new(r#""aaData":(\[.*?\]),""#).unwrap();
    if let Some(caps) = re.captures(html) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(caps.get(1).unwrap().as_str()) {
            if let Some(arr) = data.as_array() {
                return arr.iter().filter_map(|row| {
                    let r = row.as_array()?;
                    (r.len() >= 4).then(|| {
                        let album_html = r[1].as_str().unwrap_or("");
                        let url_re = Regex::new(r#"href="([^"]+)""#).unwrap();
                        serde_json::json!({
                            "artist": strip_html(r[0].as_str().unwrap_or("")),
                            "album": strip_html(album_html),
                            "url": url_re.captures(album_html).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or(""),
                            "year": Regex::new(r"\d{4}").unwrap().find(r[3].as_str().unwrap_or("")).map(|m| m.as_str()).unwrap_or(""),
                        })
                    })
                }).collect();
            }
        }
    }
    vec![]
}

fn strip_html(s: &str) -> String { Regex::new(r"<[^>]*>").unwrap().replace_all(s, "").trim().to_string() }
fn urlencode(s: &str) -> String { s.as_bytes().iter().map(|&c| match c { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (c as char).to_string(), b' ' => "+".to_string(), _ => format!("%{:02X}", c) }).collect() }

/// --get-cookie: extract cf_clearance from running Chromium via CDP.
/// Start Chromium with: chromium --remote-debugging-port=9222
/// Then open metal-archives.com and complete the Cloudflare challenge.
/// Run this tool to extract and save the cookie.
async fn cmd_get_cookie() -> Result<()> {
    println!("Connecting to Chromium on port {}...", CDP_PORT);
    println!("Make sure Chromium is running with: chromium --remote-debugging-port={}", CDP_PORT);
    println!("And you have metal-archives.com open with the challenge completed.");
    println!();

    // Try to connect to Chromium via CDP
    let ws_url = format!("http://localhost:{}/json/version", CDP_PORT);
    let resp = reqwest::get(&ws_url).await.map_err(|e| {
        anyhow::anyhow!("Cannot connect to Chromium.\n\
            Start Chromium with:\n  chromium --remote-debugging-port={}\n\
            Then open metal-archives.com.\nError: {}", CDP_PORT, e)
    })?;

    let info: serde_json::Value = resp.json().await?;
    let debug_url = info.get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No WebSocket URL from Chromium"))?;

    let (browser, mut handler) = Browser::connect(debug_url).await?;
    tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    // Navigate to MA and get cookies
    let page = browser.new_page("about:blank").await?;
    page.goto("https://www.metal-archives.com/").await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let cookies = page.get_cookies().await.unwrap_or_default();
    for c in &cookies {
        if c.name == "cf_clearance" {
            let val = format!("cf_clearance={}", c.value);
            let path = cookie_path();
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            tokio::fs::write(&path, &val).await?;
            println!("✅ Cookie saved to {:?}", path);
            println!("   Expires: {:?}", c.expires);
            println!();
            println!("The cookie persists across youtui restarts.");
            println!("Refresh it with: cargo run --release -p metal-proxy -- --get-cookie");
            return Ok(());
        }
    }

    println!("❌ No cf_clearance cookie found.");
    println!("   Make sure metal-archives.com is open in Chromium");
    println!("   and the Cloudflare challenge has been completed.");
    Ok(())
}
