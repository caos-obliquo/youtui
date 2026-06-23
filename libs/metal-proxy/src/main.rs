use anyhow::Result;
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

fn cookie_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config").join("youtui").join("ma_cookie")
}

async fn save_cookie(val: &str) {
    let path = cookie_path();
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(&path, val).await;
}

fn load_cookie() -> String {
    std::fs::read_to_string(cookie_path()).unwrap_or_default().trim().to_string()
}

struct AppState {
    cookie: Arc<Mutex<String>>,
    http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    if std::env::args().any(|a| a == "--get-cookie") {
        return cmd_get_cookie().await;
    }

    info!("Metal Archives proxy on port {}", PORT);
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
        .build()?;

    let cookie = get_or_refresh_cookie().await;
    if cookie.is_empty() {
        warn!("No MA cookie. Direct mode unavailable.");
    }

    let shared_cookie = Arc::new(Mutex::new(cookie));
    let bg_cookie = shared_cookie.clone();

    // Background: refresh cookie every 15 min from running Chromium
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(900)).await;
            if let Ok(c) = try_cdp_cookie().await {
                let mut guard = bg_cookie.lock().await;
                *guard = c.clone();
                save_cookie(&c).await;
                info!("Cookie refreshed");
            }
        }
    });

    let state = Arc::new(AppState { cookie: shared_cookie, http_client: client });
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    info!("Ready on http://0.0.0.0:{}", PORT);

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

async fn get_or_refresh_cookie() -> String {
    // Priority: env var → saved file → try CDP (no browser launch)
    if let Some(c) = std::env::var("MA_COOKIE").ok().filter(|c| !c.is_empty()) {
        save_cookie(&c).await;
        return c;
    }
    let file_cookie = load_cookie();
    if !file_cookie.is_empty() {
        return file_cookie;
    }
    // Try CDP if Chromium happens to be running with debug port
    if let Ok(c) = try_cdp_cookie().await {
        save_cookie(&c).await;
        return c;
    }
    String::new()
}

async fn try_cdp_cookie() -> Result<String> {
    let resp = reqwest::get(&format!("http://localhost:{}/json/version", CDP_PORT)).await?;
    let info: serde_json::Value = resp.json().await?;
    let ws_url = info.get("webSocketDebuggerUrl").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("no ws url"))?;

    let (browser, mut handler) = chromiumoxide::browser::Browser::connect(ws_url).await?;
    tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    let page = browser.new_page("about:blank").await?;
    page.goto("https://www.metal-archives.com/").await.ok();
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    for c in page.get_cookies().await.unwrap_or_default() {
        if c.name == "cf_clearance" {
            return Ok(format!("cf_clearance={}", c.value));
        }
    }
    Err(anyhow::anyhow!("no cf_clearance cookie"))
}

fn find_chrome() -> Option<std::path::PathBuf> {
    for p in &["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome", "/usr/bin/google-chrome-stable"] {
        if std::path::Path::new(p).exists() { return Some(std::path::PathBuf::from(p)); }
    }
    if let Ok(paths) = std::env::var("PATH") {
        for name in &["chromium", "chromium-browser", "google-chrome"] {
            for dir in paths.split(':') {
                let full = std::path::Path::new(dir).join(name);
                if full.exists() { return Some(full); }
            }
        }
    }
    None
}

// -- HTTP server --
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
        if let Some(v) = header.strip_prefix("Content-Length:") { content_length = v.trim().parse().unwrap_or(0); }
        else if let Some(v) = header.strip_prefix("content-length:") { content_length = v.trim().parse().unwrap_or(0); }
    }
    if content_length > 0 { let mut body = vec![0u8; content_length]; reader.read_exact(&mut body).await?; }
    let response = handle_request(path, &state).await;
    let mut writer = reader.into_inner();
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn handle_request(path: &str, state: &Arc<AppState>) -> String {
    let query = path.split('?').nth(1).unwrap_or("");
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
    let cookie = state.cookie.lock().await.clone();
    match path.split('?').next().unwrap_or("") {
        "/ping" => json_ok(&serde_json::json!({"status": "ok"})),
        "/search" => cmd_search_direct(&cookie, &state.http_client, &params).await,
        "/album" => cmd_album_direct(&cookie, &state.http_client, &params).await,
        _ => json_ok(&serde_json::json!({"error": "unknown"})),
    }
}

fn json_ok(data: &serde_json::Value) -> String {
    let body = serde_json::to_string(data).unwrap_or_default();
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}", body.len(), body)
}

async fn cmd_search_direct(cookie: &str, client: &reqwest::Client, params: &HashMap<String, String>) -> String {
    let artist = params.get("artist").map(|s| s.as_str()).unwrap_or("");
    let album = params.get("album").map(|s| s.as_str()).unwrap_or("");
    if artist.is_empty() || cookie.is_empty() { return json_ok(&serde_json::json!({"results": []})); }

    let url = format!("{}/search/ajax-advanced/searching/albums/?sEcho=1&iColumns=4&exactBandMatch=1&bandName={}{}",
        MA_BASE, urlencode(artist), if album.is_empty() { String::new() } else { format!("&releaseTitle={}", urlencode(album)) });

    let resp = match client.get(&url).header("Cookie", cookie).send().await {
        Ok(r) => r, _ => return json_err("request failed")
    };
    let text = match resp.text().await { Ok(t) => t, _ => return json_err("read failed") };
    json_ok(&serde_json::json!({"results": parse_search_results(&text)}))
}

async fn cmd_album_direct(cookie: &str, client: &reqwest::Client, params: &HashMap<String, String>) -> String {
    let url = match params.get("url") { Some(u) => u, None => return json_ok(&serde_json::json!({"error": "no url"})) };
    if cookie.is_empty() { return json_err("no cookie"); }

    let resp = match client.get(url).header("Cookie", cookie).send().await {
        Ok(r) => r, _ => return json_err("request failed")
    };
    let html = match resp.text().await { Ok(h) => h, _ => return json_err("read failed") };

    let doc = Html::parse_document(&html);
    let album_name = extract_text(&doc, "h1.album_name");
    let artist_name = extract_text(&doc, "h2.band_name a");
    let year = Regex::new(r"(?i)Release date:.*?(\d{4})").unwrap()
        .captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default();
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

fn parse_search_results(html: &str) -> Vec<serde_json::Value> {
    let re = Regex::new(r#""aaData":(\[.*?\]),""#).unwrap();
    if let Some(caps) = re.captures(html) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(caps.get(1).unwrap().as_str()) {
            if let Some(arr) = data.as_array() {
                return arr.iter().filter_map(|row| {
                    let r = row.as_array()?;
                    (r.len() >= 4).then(|| {
                        let ah = r[1].as_str().unwrap_or("");
                        serde_json::json!({
                            "artist": strip_html(r[0].as_str().unwrap_or("")),
                            "album": strip_html(ah),
                            "url": Regex::new(r#"href="([^"]+)""#).unwrap().captures(ah).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or(""),
                            "year": Regex::new(r"\d{4}").unwrap().find(r[3].as_str().unwrap_or("")).map(|m| m.as_str()).unwrap_or(""),
                        })
                    })
                }).collect();
            }
        }
    }
    vec![]
}

fn json_err(msg: &str) -> String { json_ok(&serde_json::json!({"error": msg})) }
fn strip_html(s: &str) -> String { Regex::new(r"<[^>]*>").unwrap().replace_all(s, "").trim().to_string() }
fn urlencode(s: &str) -> String { s.as_bytes().iter().map(|&c| match c { b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (c as char).to_string(), b' ' => "+".to_string(), _ => format!("%{:02X}", c) }).collect() }

// -- --get-cookie flag (manual mode) --
async fn cmd_get_cookie() -> Result<()> {
    let chrome = find_chrome().ok_or_else(|| anyhow::anyhow!("No Chromium found"))?;

    // Try headless=new first (background, no window)
    println!("Trying headless mode (background)...");
    let _ = tokio::process::Command::new(&chrome)
        .arg(format!("--remote-debugging-port={}", CDP_PORT))
        .arg("--headless=new").arg("--no-first-run")
        .arg("--no-default-browser-check").arg("--mute-audio")
        .arg("--disable-blink-features=AutomationControlled")
        .arg(format!("--user-data-dir=/tmp/metal-proxy-{}", std::process::id()))
        .arg("https://www.metal-archives.com/")
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .spawn()?;

    for _ in 0..30 {
        if let Ok(c) = try_cdp_cookie().await {
            save_cookie(&c).await;
            println!("✅ Cookie saved (headless mode)");
            return Ok(());
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    // Headless failed, try visible window
    println!("Headless blocked. Opening visible Chromium...");
    println!("Complete the Cloudflare challenge in the window.");
    let _ = tokio::process::Command::new(&chrome)
        .arg(format!("--remote-debugging-port={}", CDP_PORT))
        .arg("--no-first-run").arg("--no-default-browser-check")
        .arg(format!("--user-data-dir=/tmp/metal-proxy-vis-{}", std::process::id()))
        .arg("https://www.metal-archives.com/")
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .spawn()?;

    for _ in 0..60 {
        if let Ok(c) = try_cdp_cookie().await {
            save_cookie(&c).await;
            println!("✅ Cookie saved!");
            return Ok(());
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    Err(anyhow::anyhow!("Timed out waiting for cookie"))
}
