use super::{AuthToken, RawResult, fallback_client_version};
use crate::client::Client;
use crate::error::{Error, Result};
use crate::parse::ProcessedResult;
use crate::utils;
use crate::utils::constants::{USER_AGENT, YTM_URL};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub struct BrowserToken {
    sapisid: String,
    client_version: String,
    cookies: String,
    visitor_id: Option<String>,
}

impl AuthToken for BrowserToken {
    fn client_version(&self) -> Cow<'_, str> {
        (&self.client_version).into()
    }
    fn deserialize_response<Q>(
        raw: RawResult<Q, Self>,
    ) -> Result<crate::parse::ProcessedResult<Q>> {
        let processed = ProcessedResult::try_from(raw)?;
        if let Some(error) = processed.get_json().pointer("/error") {
            let Some(code) = error.pointer("/code").and_then(|v| v.as_u64()) else {
                return Err(Error::response("API reported an error but no code"));
            };
            let message = error
                .pointer("/message")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            return Err(Error::other_code(code, message));
        }
        Ok(processed)
    }
    fn headers(&self) -> Result<impl IntoIterator<Item = (&str, Cow<'_, str>)>> {
        let hash = utils::hash_sapisid(&self.sapisid);
        let mut headers: Vec<(&str, Cow<'_, str>)> = vec![
            ("User-Agent", USER_AGENT.into()),
            ("Content-Type", "application/json".into()),
            ("X-Goog-Api-Format-Version", "1".into()),
            ("X-YouTube-Client-Name", "67".into()),
            ("X-YouTube-Client-Version", self.client_version.as_str().into()),
            ("X-Origin", YTM_URL.into()),
            ("Origin", YTM_URL.into()),
            ("Referer", format!("{YTM_URL}/").into()),
            ("Authorization", format!("SAPISIDHASH {hash}").into()),
            ("Cookie", self.cookies.as_str().into()),
        ];
        if let Some(ref vid) = self.visitor_id {
            headers.push(("X-Goog-Visitor-Id", vid.as_str().into()));
        }
        Ok(headers)
    }
}

impl BrowserToken {
    pub async fn from_str(cookie_str: &str, _client: &Client) -> Result<Self> {
        let trimmed = cookie_str.trim();
        let cookies = if trimmed.contains('\t') {
            parse_netscape_cookies(trimmed)
        } else {
            trimmed.to_string()
        };
        let sapisid = cookies
            .split_once("SAPISID=")
            .ok_or(Error::header())?
            .1
            .split_once(';')
            .ok_or(Error::header())?
            .0
            .to_string();
        let client_version = fallback_client_version(&Utc::now());
        Ok(Self {
            sapisid,
            client_version,
            cookies,
            visitor_id: None,
        })
    }
    pub async fn from_cookie_file<P>(path: P, client: &Client) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let contents = tokio::fs::read_to_string(path).await?;
        BrowserToken::from_str(&contents, client).await
    }
}

fn parse_netscape_cookies(input: &str) -> String {
    // Deduplicate cookies: last occurrence wins (matches Python dict behavior).
    // yt-dlp auto-refresh appends cookies without removing old ones,
    // producing duplicates with DIFFERENT values for critical auth cookies
    // (OSID, __Secure-3PSIDCC, HSID, auth_token, etc.). Sending expired
    // cookie values alongside fresh ones causes YouTube to reject requests.
    // Netscape format: domain\tflag\tpath\tsecure\texpiry\tname\tvalue
    let mut seen = std::collections::BTreeMap::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.splitn(7, '\t');
        let _ = fields.nth(4); // skip domain, flag, path, secure, expiry
        let name = match fields.next() {
            Some(n) => n,
            None => continue,
        };
        let value = match fields.next() {
            Some(v) => v,
            None => continue,
        };
        seen.insert(name.to_string(), value.to_string());
    }
    seen.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

// Don't use default Debug implementation for BrowserToken - contents are
// private
impl Debug for BrowserToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Private BrowserToken")
    }
}
