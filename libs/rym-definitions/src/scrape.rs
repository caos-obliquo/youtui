use serde::Serialize;

/// A single RYM genre with its description.
#[derive(Debug, Clone, Serialize)]
pub struct GenreEntry {
    pub name: String,
    pub description: String,
}

/// A single RYM descriptor with its explanation.
#[derive(Debug, Clone, Serialize)]
pub struct DescriptorEntry {
    pub name: String,
    pub explanation: String,
}

/// RateYourMusic scraping client with Cloudflare cookie support.
pub struct RymClient {
    inner: reqwest::Client,
    cookies: String,
}

impl RymClient {
    pub fn new(cookies: String) -> Self {
        let inner = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36")
            .cookie_store(true)
            .build()
            .expect("Failed to build reqwest client");
        Self { inner, cookies }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER};
        let mut h = HeaderMap::new();
        h.insert(
            COOKIE,
            HeaderValue::from_str(&self.cookies)
                .expect("Invalid cookie value"),
        );
        h.insert(REFERER, HeaderValue::from_static("https://rateyourmusic.com/"));
        h
    }

    /// Test the cookie by fetching the RYM genres page.
    pub async fn test_connection(&self) -> Result<(u16, String), String> {
        let resp = self
            .inner
            .get("https://rateyourmusic.com/genres/")
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(|e| format!("Body read failed: {}", e))?;
        Ok((status, body))
    }

    /// Fetch all genre entries from `/genres/`.
    pub async fn fetch_genres(&self) -> Result<Vec<GenreEntry>, String> {
        let body = self.get_text("https://rateyourmusic.com/genres/").await?;
        super::parse::parse_genres(&body)
    }

    /// Fetch all descriptor entries from `/descriptors/`.
    pub async fn fetch_descriptors(&self) -> Result<Vec<DescriptorEntry>, String> {
        let body = self.get_text("https://rateyourmusic.com/descriptors/").await?;
        super::parse::parse_descriptors(&body)
    }

    async fn get_text(&self, url: &str) -> Result<String, String> {
        let resp = self
            .inner
            .get(url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(format!("Status {} for {}", status, url));
        }
        resp.text().await.map_err(|e| format!("Body read failed: {}", e))
    }
}
