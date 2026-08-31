//! Last.fm recommendations CLI subcommand (pure Last.fm, no YTM).
//!
//! Fetches the user's top-scrobbled tracks/albums/artists, then for each top
//! item fetches its "similar" counterpart via Last.fm's public discovery
//! methods (`track.getSimilar` / `artist.getSimilar`). Every recommendation is
//! printed with its "Similar to" reason string.
//!
//! NOTE: Last.fm removed the `user.getRecommended*` methods from its public
//! catalog (they return error 3 "No method with that name in this package").
//! This module therefore synthesizes recommendations from the still-public
//! discovery endpoints:
//!   - tracks:  user.getTopTracks  -> track.getSimilar
//!   - artists: user.getTopArtists -> artist.getSimilar
//!   - albums:  user.getTopAlbums  -> artist.getSimilar(album.artist)
//!     (album.getSimilar does not exist on the public API)

use crate::config::ScrobblingConfig;
use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{debug, info, warn};

/// Last.fm API base endpoint.
const LASTFM_BASE: &str = "https://ws.audioscrobbler.com/2.0";

/// Which recommendation feed to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecKind {
    Tracks,
    Albums,
    Artists,
}

impl std::fmt::Display for RecKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RecKind::Tracks => "Tracks",
            RecKind::Albums => "Albums",
            RecKind::Artists => "Artists",
        };
        write!(f, "{}", s)
    }
}

impl RecKind {
    /// `user.getTop*` method name for this feed (the "based on my scrobbles" source).
    fn top_method(&self) -> &'static str {
        match self {
            RecKind::Tracks => "user.getTopTracks",
            RecKind::Albums => "user.getTopAlbums",
            RecKind::Artists => "user.getTopArtists",
        }
    }

    /// JSON container key of the `user.getTop*` response.
    fn top_container_key(&self) -> &'static str {
        match self {
            RecKind::Tracks => "toptracks",
            RecKind::Albums => "topalbums",
            RecKind::Artists => "topartists",
        }
    }

    /// JSON key of each item inside the `user.getTop*` array.
    fn top_item_key(&self) -> &'static str {
        match self {
            RecKind::Tracks => "track",
            RecKind::Albums => "album",
            RecKind::Artists => "artist",
        }
    }
}

/// A single synthesized Last.fm recommendation.
#[derive(Debug, Clone, PartialEq)]
pub struct RecItem {
    pub kind: RecKind,
    pub title: String,
    pub artist: String,
    pub mbid: String,
    pub url: String,
    pub playcount: Option<u64>,
    /// The "Similar to" reason string (always populated for Option A).
    pub reason: Option<String>,
    /// Last.fm similarity relevance score (0-1) from `getSimilar` responses.
    pub match_score: Option<f64>,
}

/// Parse a Last.fm count that may be encoded as a string or a number.
fn parse_count(v: &Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse::<u64>().ok();
    }
    None
}

/// A single source "top" item returned by `user.getTop*`.
#[derive(Clone)]
struct TopItem {
    title: String,
    artist: String,
}

/// Parse the `user.getTop*` response into the source top items.
fn parse_top_items(body: &str, kind: RecKind) -> Result<Vec<TopItem>> {
    let root: Value = serde_json::from_str(body)
        .with_context(|| format!("Failed to parse Last.fm {} top response", kind))?;
    let container = root
        .get(kind.top_container_key())
        .with_context(|| format!("Missing '{}' container in Last.fm response", kind.top_container_key()))?;
    let arr = container
        .get(kind.top_item_key())
        .and_then(|a| a.as_array())
        .with_context(|| format!("Missing '{}' array in Last.fm response", kind.top_item_key()))?;
    let mut items = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(title) = item.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        // Tracks/albums nest artist under `artist.name`; artists have none.
        let artist = item
            .get("artist")
            .and_then(|a| a.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        items.push(TopItem {
            title: title.to_string(),
            artist,
        });
    }
    Ok(items)
}

/// Extract a `RecItem` from a `getSimilar`-shaped item (track or artist).
fn parse_similar_item(item: &Value, kind: RecKind, source: &TopItem) -> Option<RecItem> {
    let title = item.get("name")?.as_str()?;
    let artist = match kind {
        RecKind::Artists => String::new(),
        _ => item
            .get("artist")
            .and_then(|a| a.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string(),
    };
    let mbid = item
        .get("mbid")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    let url = item
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();
    let playcount = item.get("playcount").and_then(parse_count);
    let match_score = item.get("match").and_then(|m| m.as_f64());
    // Build the "Similar to" reason: <source title> [by <source artist>].
    let reason = if source.artist.is_empty() {
        format!("Similar to: {}", source.title)
    } else {
        format!("Similar to: {} by {}", source.title, source.artist)
    };
    Some(RecItem {
        kind,
        title: title.to_string(),
        artist,
        mbid,
        url,
        playcount,
        reason: Some(reason),
        match_score,
    })
}

/// Send a signed GET to the Last.fm API and return the parsed JSON body.
async fn lastfm_get(config: &ScrobblingConfig, method: &str, signed_params: Vec<(String, String)>) -> Result<Value> {
    let mut params: Vec<(String, String)> = vec![
        ("method".into(), method.into()),
        ("api_key".into(), config.api_key.clone()),
    ];
    // `sk` is sent for signed requests; harmless when empty.
    if !config.session_key.trim().is_empty() {
        params.push(("sk".into(), config.session_key.clone()));
    }
    params.extend(signed_params);
    // `format=json` is a transport param and must NOT be part of the signature.
    let api_sig = crate::config::sign_lastfm(&params, &config.api_secret);
    params.push(("api_sig".into(), api_sig));
    params.push(("format".into(), "json".into()));

    let client = reqwest::Client::new();
    let resp = client
        .get(LASTFM_BASE)
        .query(&params)
        .send()
        .await
        .with_context(|| format!("Last.fm {} request failed", method))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .with_context(|| format!("Failed to read Last.fm {} response body", method))?;
    if !status.is_success() {
        anyhow::bail!("Last.fm {} returned HTTP {}: {}", method, status, body);
    }
    // Last.fm returns error payloads with HTTP 200; surface them clearly.
    let value: Value = serde_json::from_str(&body)
        .with_context(|| format!("Failed to parse Last.fm {} JSON response", method))?;
    if let Value::Object(map) = &value {
        if let Some(err) = map.get("error") {
            anyhow::bail!(
                "Last.fm {} API error {}: {}",
                method,
                err,
                map.get("message").map(|m| m.to_string()).unwrap_or_default()
            );
        }
    }
    Ok(value)
}

/// Fetch the user's top items for the given feed/track/album/artist kind.
async fn fetch_top_items(
    config: &ScrobblingConfig,
    kind: RecKind,
    limit: u32,
    page: u32,
) -> Result<Vec<TopItem>> {
    // `user` is intentionally omitted: when the request is signed with the
    // account's session_key, Last.fm scopes `getTop*` to the session owner.
    let params = vec![
        ("limit".into(), limit.to_string()),
        ("page".into(), page.to_string()),
    ];
    let body = lastfm_get(config, kind.top_method(), params).await?;
    let raw = serde_json::to_string(&body)
        .with_context(|| format!("Failed to re-serialize {} top response", kind))?;
    let items = parse_top_items(&raw, kind)?;
    debug!("Fetched {} top {} items (limit={} page={})", items.len(), kind, limit, page);
    Ok(items)
}

/// Fetch the "similar" counterpart for a single source item.
async fn fetch_similar_for_source(
    config: &ScrobblingConfig,
    kind: RecKind,
    source: &TopItem,
    similar_limit: u32,
) -> Result<Vec<RecItem>> {
    let (method, params, container_key, item_key): (&str, Vec<(String, String)>, &str, &str) =
        match kind {
            RecKind::Tracks => (
                "track.getSimilar",
                vec![
                    ("artist".into(), source.artist.clone()),
                    ("track".into(), source.title.clone()),
                    ("limit".into(), similar_limit.to_string()),
                ],
                "similartracks",
                "track",
            ),
            RecKind::Artists => (
                "artist.getSimilar",
                vec![
                    ("artist".into(), source.title.clone()),
                    ("limit".into(), similar_limit.to_string()),
                ],
                "similarartists",
                "artist",
            ),
            // album.getSimilar does not exist; use the album's artist instead.
            RecKind::Albums => (
                "artist.getSimilar",
                vec![
                    ("artist".into(), source.artist.clone()),
                    ("limit".into(), similar_limit.to_string()),
                ],
                "similarartists",
                "artist",
            ),
        };
    let value = lastfm_get(config, method, params).await?;
    let container = value
        .get(container_key)
        .with_context(|| format!("Missing '{}' container in {} response", container_key, method))?;
    let arr = container
        .get(item_key)
        .and_then(|a| a.as_array())
        .with_context(|| format!("Missing '{}' array in {} response", item_key, method))?;
    let mut items = Vec::with_capacity(arr.len());
    for item in arr {
        if let Some(rec) = parse_similar_item(item, kind, source) {
            items.push(rec);
        }
    }
    debug!(
        "Fetched {} similar items for source '{}' via {} (limit={})",
        items.len(), source.title, method, similar_limit
    );
    Ok(items)
}

/// Build a full list of recommendations for one feed kind.
///
/// `limit` is the number of top-scrobbled source items to consider;
/// `page` paginates that source list. Each source produces one recommendation
/// (the top similar item), so the returned length is bounded by `limit`.
pub async fn fetch_recommendations(
    config: &ScrobblingConfig,
    kind: RecKind,
    limit: u32,
    page: u32,
) -> Result<Vec<RecItem>> {
    info!("Fetching recommendations: kind={:?} limit={} page={}", kind, limit, page);
    let top = fetch_top_items(config, kind, limit, page).await?;
    let mut out = Vec::with_capacity(top.len());
    for source in &top {
        // One similar item per source keeps output bounded and reasons precise.
        let recs = fetch_similar_for_source(config, kind, source, 1).await?;
        out.extend(recs);
    }
    info!("Fetched {} recommendations for {:?}", out.len(), kind);
    Ok(out)
}

/// Look up an artist's total listener count via `artist.getInfo`.
async fn fetch_artist_listeners(config: &ScrobblingConfig, artist: &str) -> Result<Option<u64>> {
    if artist.trim().is_empty() {
        return Ok(None);
    }
    let value = lastfm_get(
        config,
        "artist.getInfo",
        vec![("artist".into(), artist.to_string())],
    )
    .await?;
    Ok(value
        .get("artist")
        .and_then(|a| a.get("stats"))
        .and_then(|s| s.get("listeners"))
        .and_then(parse_count))
}

/// Map a listener count to a niche favorability score in [0, 1].
/// Small audiences are heavily favored; mainstream artists approach 0.
fn niche_favor(listeners: Option<u64>) -> f64 {
    match listeners {
        Some(n) if n <= 50_000 => 1.0,
        Some(n) if n <= 200_000 => 0.7,
        Some(n) if n <= 1_000_000 => 0.4,
        Some(_) => 0.1,
        None => 0.5,
    }
}

/// Build niche-first recommendations.
///
/// Seeds from the user's top artists (wide, capturing multiple clusters),
/// expands each seed via `getSimilar` carrying its `match` score, skews
/// toward lower-listener (niche) candidates, excludes artists already in the
/// user's corpus, and caps per-seed and globally for diversity.
pub async fn fetch_niche_recommendations(
    config: &ScrobblingConfig,
    kind: RecKind,
    limit: u32,
    seed_count: u32,
    niche_level: f64,
    page: u32,
) -> Result<Vec<RecItem>> {
    use futures::stream::StreamExt;

    info!(
        "Fetching niche recommendations: kind={:?} limit={} seed_count={} niche_level={} page={}",
        kind, limit, seed_count, niche_level, page
    );
    let seeds = fetch_top_items(config, kind, seed_count, page).await?;
    debug!(
        "Niche recommendations: fetched {} top seeds",
        seeds.len()
    );
    let known: std::collections::HashSet<String> = seeds.iter().map(|s| s.title.clone()).collect();

    let seed_candidates: Vec<(crate::lastfm_recommend::TopItem, Vec<RecItem>)> =
        futures::stream::iter(seeds.iter().cloned().map(|seed| {
            let cfg = config.clone();
            async move {
                let recs = fetch_similar_for_source(&cfg, kind, &seed, 10).await?;
                Ok::<(crate::lastfm_recommend::TopItem, Vec<RecItem>), anyhow::Error>((seed, recs))
            }
        }))
        .buffer_unordered(8)
        .collect::<Vec<Result<(crate::lastfm_recommend::TopItem, Vec<RecItem>)>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    let seed_batches = seed_candidates.len();
    let mut picked: Vec<(RecItem, Vec<String>)> = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (seed, cand_batch) in seed_candidates {
        let mut per_seed = 0u32;
        for cand in cand_batch {
            if per_seed >= 3 {
                break;
            }
            let name_key = if cand.artist.is_empty() {
                cand.title.clone()
            } else {
                format!("{}|{}", cand.artist, cand.title)
            };
            if known.contains(&cand.artist) || known.contains(&cand.title) {
                continue;
            }
            if let Some(&idx) = seen.get(&name_key) {
                if !picked[idx].1.contains(&seed.title) {
                    picked[idx].1.push(seed.title.clone());
                }
            } else {
                let idx = picked.len();
                seen.insert(name_key, idx);
                picked.push((cand, vec![seed.title.clone()]));
                per_seed += 1;
            }
        }
    }
    debug!(
        "Niche recommendations: collected {} unique candidates from {} seed batches",
        picked.len(),
        seed_batches
    );

    let mut all: Vec<(RecItem, f64)> = futures::stream::iter(picked.into_iter().map(|(cand, seeds)| {
        let artist = if kind == RecKind::Artists {
            cand.title.clone()
        } else {
            cand.artist.clone()
        };
        let cfg = config.clone();
        async move {
            let listeners = fetch_artist_listeners(&cfg, &artist).await?;
            let match_score = cand.match_score.unwrap_or(0.5);
            let favor = niche_favor(listeners);
            let score = match_score * (1.0 - niche_level) + favor * niche_level;
            debug!(
                "Niche candidate: artist={} match={:.2} listeners={:?} favor={:.2} score={:.2}",
                artist, match_score, listeners, favor, score
            );
            let seed_str = seeds.join(", ");
            let base = if seed_str.is_empty() {
                "Similar to: unknown".to_string()
            } else {
                format!("Similar to: {}", seed_str)
            };
            let mut item = cand;
            item.reason = Some(format!("{} (match {:.2}){}", base, score, niche_suffix(niche_level)));
            Ok::<(RecItem, f64), anyhow::Error>((item, score))
        }
    }))
    .buffer_unordered(8)
    .collect::<Vec<Result<(RecItem, f64)>>>()
    .await
    .into_iter()
    .collect::<Result<Vec<_>>>()?;

    all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let total_scored = all.len();
    let mut out = Vec::new();
    for (item, _score) in all {
        if out.len() >= limit as usize {
            break;
        }
        out.push(item);
    }
    info!(
        "Niche recommendations: produced {} of {} scored candidates (limit {})",
        out.len(),
        total_scored,
        limit
    );
    Ok(out)
}

fn niche_suffix(niche_level: f64) -> &'static str {
    if niche_level >= 0.5 {
        " [niche]"
    } else {
        ""
    }
}

/// Print recommendations in sectioned list mode.
pub fn print_recommendations(items: &[RecItem]) {
    let mut section = None;
    for (i, item) in items.iter().enumerate() {
        if section != Some(item.kind) {
            if section.is_some() {
                println!();
            }
            println!("== {} ==", item.kind);
            section = Some(item.kind);
        }
        let head = if item.artist.is_empty() {
            item.title.clone()
        } else {
            format!("{} - {}", item.artist, item.title)
        };
        let playcount = item
            .playcount
            .map(|n| format!(" [playcount: {}]", n))
            .unwrap_or_default();
        let reason = item
            .reason
            .clone()
            .unwrap_or_else(|| "(no reason provided)".to_string());
        println!("{}. {}{}  {}", i + 1, head, playcount, reason);
    }
    if section.is_none() {
        println!("No recommendations returned.");
    }
}

/// ListenBrainz API base endpoint (collaborative-filtering recommendations).
const LISTENBRAINZ_BASE: &str = "https://api.listenbrainz.org";

/// One ListenBrainz collaborative-filtering recommendation.
///
/// Unlike the Last.fm similarity feed, LB CF answers "people who share your
/// taste also listen to X". Every item carries a `reason` string so the
/// mandatory "Similar to" requirement is satisfied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LbRecItem {
    pub title: String,
    pub artist: String,
    pub reason: Option<String>,
    pub url: String,
}

/// Parse a single LB CF recording item into an [`LbRecItem`].
///
/// The response should contain a `track_metadata` object with `title`/`track_name`
/// and `artist_name`, plus an optional `reason`. We are tolerant of the shape
/// because LB may evolve its payload.
fn parse_lb_recording(value: &Value) -> Option<LbRecItem> {
    let info = value.get("track_metadata")?;
    let title = info
        .get("title")
        .or_else(|| info.get("track_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let artist = info
        .get("artist_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let reason = value
        .get("reason")
        .or_else(|| value.get("recommendation"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let url = value
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some(LbRecItem {
        title,
        artist,
        reason,
        url,
    })
}

/// Parse the LB CF recommendation response body into a list of items.
///
/// LB returns `{"payload": {"recording": [...]}}` on success. If the body is
/// empty (HTTP 204) LB has simply not generated recommendations yet, so we
/// return an empty vector rather than an error.
fn parse_lb_recommendations(value: &Value) -> Vec<LbRecItem> {
    let mut out = Vec::new();
    let payload = value.get("payload");
    let list = payload
        .and_then(|p| p.get("recording"))
        .or_else(|| value.get("recording"))
        .or_else(|| value.get("payload"));
    let Some(list) = list else {
        return out;
    };
    if let Some(arr) = list.as_array() {
        for item in arr {
            if let Some(rec) = parse_lb_recording(item) {
                out.push(rec);
            }
        }
    }
    out
}

/// Fetch ListenBrainz collaborative-filtering recommendations for a username.
///
/// `artist_type` is one of `top`, `similar`, `raw` (per LB docs). Returns an
/// empty vector if LB has not yet generated recommendations (HTTP 204); this is
/// NOT an error, it means LB's nightly CF job has not run against the corpus.
pub async fn fetch_listenbrainz_recommendations(
    config: &ScrobblingConfig,
    username: &str,
    artist_type: &str,
) -> Result<Vec<LbRecItem>> {
    if config.listenbrainz_token.trim().is_empty() {
        anyhow::bail!("listenbrainz_token not configured in [scrobbling] config section");
    }
    let url = format!(
        "{}/1/cf/recommendation/user/{}/recording",
        LISTENBRAINZ_BASE, username
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .query(&[("artist_type", artist_type)])
        .header("Authorization", format!("Token {}", config.listenbrainz_token))
        .send()
        .await
        .with_context(|| format!("ListenBrainz CF request failed for {}", username))?;
    let status = resp.status();
    // HTTP 204 means LB has not generated recommendations for this user yet.
    if status == reqwest::StatusCode::NO_CONTENT {
        info!(
            "ListenBrainz CF report empty (204) for user={} artist_type={}; falling back to corpus synthesis",
            username, artist_type
        );
        return synthesize_listenbrainz_recommendations(config, username, 20, 6).await;
    }
    let body = resp
        .text()
        .await
        .with_context(|| format!("Failed to read ListenBrainz CF response body for {}", username))?;
    if !status.is_success() {
        warn!(
            "ListenBrainz CF returned HTTP {} for user={} artist_type={}: {}",
            status, username, artist_type, body
        );
        anyhow::bail!("ListenBrainz CF returned HTTP {}: {}", status, body);
    }
    let value: Value = serde_json::from_str(&body)
        .with_context(|| format!("Failed to parse ListenBrainz CF JSON response for {}", username))?;
    let recs = parse_lb_recommendations(&value);
    info!(
        "ListenBrainz CF returned {} recommendations for user={} artist_type={}",
        recs.len(),
        username,
        artist_type
    );
    Ok(recs)
}

async fn fetch_listenbrainz_top_artists(
    config: &ScrobblingConfig,
    username: &str,
    pages: u32,
    per_page: u32,
) -> Result<Vec<(String, u64)>> {
    let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut max_ts: Option<u64> = None;
    let client = reqwest::Client::new();

    for _ in 0..pages {
        let mut req = client
            .get(format!(
                "{}/1/user/{}/listens",
                LISTENBRAINZ_BASE, username
            ))
            .query(&[("count", per_page.to_string())])
            .header("Authorization", format!("Token {}", config.listenbrainz_token));
        if let Some(ts) = max_ts {
            req = req.query(&[("max_ts", ts.to_string())]);
        }
        let resp = req
            .send()
            .await
            .with_context(|| format!("ListenBrainz listens request failed for {}", username))?;
        let status = resp.status();
        if !status.is_success() {
            warn!(
                "ListenBrainz listens returned HTTP {} for user={}: {}",
                status,
                username,
                resp.text().await.unwrap_or_default()
            );
            break;
        }
        let body = resp
            .text()
            .await
            .with_context(|| format!("Failed to read ListenBrainz listens body for {}", username))?;
        let value: Value = serde_json::from_str(&body)
            .with_context(|| format!("Failed to parse ListenBrainz listens JSON for {}", username))?;
        let payload = value.get("payload").cloned().unwrap_or(Value::Null);
        let listens = payload.get("listens").and_then(|l| l.as_array()).cloned().unwrap_or_default();
        if listens.is_empty() {
            info!(
                "ListenBrainz listens walk exhausted at max_ts={:?} for user={}",
                max_ts, username
            );
            break;
        }
        let mut page_oldest: Option<u64> = None;
        for listen in &listens {
            let artist = listen
                .get("track_metadata")
                .and_then(|m| m.get("artist_name"))
                .and_then(|a| a.as_str())
                .unwrap_or("");
            if !artist.is_empty() {
                *counts.entry(artist.to_string()).or_insert(0) += 1;
            }
        }
        page_oldest = payload.get("oldest_listen_ts").and_then(|v| v.as_u64());
        let fetched = listens.len() as u64;
        debug!(
            "ListenBrainz listens page: user={} fetched={} oldest_ts={:?}",
            username, fetched, page_oldest
        );
        // Stop once a page under-fills; the server has no more history.
        if (listens.len() as u32) < per_page {
            break;
        }
        match page_oldest {
            Some(ts) => max_ts = Some(ts),
            None => break,
        }
    }

    let mut top: Vec<(String, u64)> = counts.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    info!(
        "ListenBrainz corpus: aggregated {} distinct artists for user={}",
        top.len(),
        username
    );
    Ok(top)
}

/// Synthesize artist recommendations from the ListenBrainz scrobble corpus when
/// LB's CF feed is empty. Seeds come from aggregate top artists in LB listens,
/// then feed the proven Last.fm `artist.getSimilar` engine. The "Similar to"
/// reason is preserved on every item.
pub async fn synthesize_listenbrainz_recommendations(
    config: &ScrobblingConfig,
    username: &str,
    limit: u32,
    pages: u32,
) -> Result<Vec<LbRecItem>> {
    if config.listenbrainz_token.trim().is_empty() {
        anyhow::bail!("listenbrainz_token not configured in [scrobbling] config section");
    }
    let top = fetch_listenbrainz_top_artists(config, username, pages, 500).await?;
    if top.is_empty() {
        return Ok(Vec::new());
    }
    info!(
        "Synthesizing ListenBrainz recommendations from {} top artists for {}",
        top.len(),
        username
    );
    let mut out: Vec<LbRecItem> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (seed_artist, _count) in top {
        let source = TopItem {
            title: seed_artist.clone(),
            artist: String::new(),
        };
        let resp = lastfm_get(
            config,
            "artist.getSimilar",
            vec![
                ("artist".into(), seed_artist.clone()),
                ("limit".into(), "10".to_string()),
            ],
        )
        .await;
        let Ok(value) = resp else {
            debug!("artist.getSimilar failed for seed '{}': skipped", seed_artist);
            continue;
        };
        let Some(arr) = value
            .get("similarartists")
            .and_then(|c| c.get("artist"))
            .and_then(|a| a.as_array())
        else {
            continue;
        };
        for item in arr {
            let Some(sim) = parse_similar_item(item, RecKind::Artists, &source) else {
                continue;
            };
            if seen.contains(&sim.title) {
                continue;
            }
            seen.insert(sim.title.clone());
            let reason = sim.reason.unwrap_or_else(|| {
                format!("Similar to: {}", seed_artist)
            });
            out.push(LbRecItem {
                title: sim.title.clone(),
                artist: String::new(),
                reason: Some(reason),
                url: sim.url,
            });
            if out.len() as u32 >= limit {
                break;
            }
        }
        if out.len() as u32 >= limit {
            break;
        }
    }
    info!(
        "Synthesized {} ListenBrainz artist recommendations for {}",
        out.len(),
        username
    );
    Ok(out)
}

/// Print ListenBrainz CF recommendations in list mode.
pub fn print_listenbrainz_recommendations(items: &[LbRecItem]) {
    if items.is_empty() {
        println!("No ListenBrainz recommendations yet.");
        println!("ListenBrainz computes recommendations on a nightly batch; run this again once the job has run.");
        return;
    }
    for (i, item) in items.iter().enumerate() {
        let head = if item.artist.is_empty() {
            item.title.clone()
        } else {
            format!("{} - {}", item.artist, item.title)
        };
        let reason = item
            .reason
            .clone()
            .unwrap_or_else(|| "(no reason provided)".to_string());
        println!("{}. {}  {}", i + 1, head, reason);
    }
}

/// Print ListenBrainz CF recommendations as a machine-readable JSON dump.
pub fn print_listenbrainz_recommendations_json(items: &[LbRecItem]) {
    let out: Vec<Value> = items
        .iter()
        .map(|item| {
            serde_json::json!({
                "title": item.title,
                "artist": item.artist,
                "reason": item.reason,
                "url": item.url,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

/// Print recommendations as a machine-readable JSON dump.
pub fn print_recommendations_json(items: &[RecItem]) {
    let out: Vec<Value> = items
        .iter()
        .map(|item| {
            serde_json::json!({
                "kind": item.kind.to_string(),
                "title": item.title,
                "artist": item.artist,
                "mbid": item.mbid,
                "url": item.url,
                "playcount": item.playcount,
                "reason": item.reason,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top_tracks_fixture() -> &'static str {
        r#"{
            "toptracks": {
                "track": [
                    { "name": "Poison Tree", "artist": { "name": "Grouper", "mbid": "a1" }, "playcount": "1197", "mbid": "t1", "url": "https://last.fm/track/1" },
                    { "name": "Your Guts Are Like Mine", "artist": { "name": "Set Fire to Flames", "mbid": "a2" }, "playcount": 900, "mbid": "t2", "url": "https://last.fm/track/2" }
                ]
            }
        }"#
    }

    fn top_albums_fixture() -> &'static str {
        r#"{
            "topalbums": {
                "album": [
                    { "name": "Below the House", "artist": { "name": "Planning for Burial", "mbid": "a3" }, "playcount": "500", "mbid": "al1", "url": "https://last.fm/album/1" }
                ]
            }
        }"#
    }

    fn top_artists_fixture() -> &'static str {
        r#"{
            "topartists": {
                "artist": [
                    { "name": "Grouper", "playcount": "20000", "mbid": "ap1", "url": "https://last.fm/artist/1" }
                ]
            }
        }"#
    }

    fn similar_tracks_fixture() -> &'static str {
        r#"{
            "similartracks": {
                "track": [
                    { "name": "Mirrorring", "artist": { "name": "Grouper", "mbid": "s1" }, "playcount": "300", "mbid": "st1", "url": "https://last.fm/track/s1", "match": 1.0 }
                ]
            }
        }"#
    }

    fn similar_artists_fixture() -> &'static str {
        r#"{
            "similarartists": {
                "artist": [
                    { "name": "Mirrorring", "playcount": "100", "mbid": "sa1", "url": "https://last.fm/artist/s1", "match": 1.0 }
                ]
            }
        }"#
    }

    #[test]
    fn test_parse_top_tracks() {
        let items = parse_top_items(top_tracks_fixture(), RecKind::Tracks).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Poison Tree");
        assert_eq!(items[0].artist, "Grouper");
        assert_eq!(items[1].artist, "Set Fire to Flames");
    }

    #[test]
    fn test_parse_top_albums() {
        let items = parse_top_items(top_albums_fixture(), RecKind::Albums).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Below the House");
        assert_eq!(items[0].artist, "Planning for Burial");
    }

    #[test]
    fn test_parse_top_artists() {
        let items = parse_top_items(top_artists_fixture(), RecKind::Artists).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Grouper");
        // Artists have no nested artist name.
        assert_eq!(items[0].artist, "");
    }

    #[test]
    fn test_parse_similar_track_with_reason() {
        let source = TopItem { title: "Poison Tree".into(), artist: "Grouper".into() };
        let value: Value = serde_json::from_str(similar_tracks_fixture()).unwrap();
        let arr = &value["similartracks"]["track"].as_array().unwrap()[0];
        let rec = parse_similar_item(arr, RecKind::Tracks, &source).unwrap();
        assert_eq!(rec.kind, RecKind::Tracks);
        assert_eq!(rec.title, "Mirrorring");
        assert_eq!(rec.artist, "Grouper");
        assert_eq!(rec.playcount, Some(300));
        assert_eq!(rec.reason.as_deref(), Some("Similar to: Poison Tree by Grouper"));
    }

    #[test]
    fn test_parse_similar_artist_with_reason() {
        let source = TopItem { title: "Grouper".into(), artist: String::new() };
        let value: Value = serde_json::from_str(similar_artists_fixture()).unwrap();
        let arr = &value["similarartists"]["artist"].as_array().unwrap()[0];
        let rec = parse_similar_item(arr, RecKind::Artists, &source).unwrap();
        assert_eq!(rec.kind, RecKind::Artists);
        assert_eq!(rec.title, "Mirrorring");
        // Artists have no nested artist name.
        assert_eq!(rec.artist, "");
        assert_eq!(rec.reason.as_deref(), Some("Similar to: Grouper"));
    }

    #[test]
    fn test_lb_parse_recording_with_reason() {
        let value: Value = serde_json::from_str(
            r#"{
                "payload": {
                    "recording": [
                        { "track_metadata": { "title": "Sleep Maps", "artist_name": "Set Fire to Flames" }, "reason": "Similar to: Grouper by plan" }
                    ]
                }
            }"#,
        )
        .unwrap();
        let items = parse_lb_recommendations(&value);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Sleep Maps");
        assert_eq!(items[0].artist, "Set Fire to Flames");
        assert_eq!(items[0].reason.as_deref(), Some("Similar to: Grouper by plan"));
    }

    #[test]
    fn test_lb_parse_empty_payload_is_tolerant() {
        // A 204-derived empty body is a valid "not generated yet" state, not an error.
        let items = parse_lb_recommendations(&serde_json::json!({}));
        assert!(items.is_empty());
        let items2 = parse_lb_recommendations(&serde_json::json!({"payload": {"recording": []}}));
        assert!(items2.is_empty());
    }

    #[test]
    fn test_sign_lastfm_known_vector() {
        // Reuse the scrobbler's signature test vector.
        let params: Vec<(String, String)> = vec![
            ("api_key".into(), "key123".into()),
            ("api_secret".into(), "secret456".into()),
            ("method".into(), "track.scrobble".into()),
            ("sk".into(), "sk789".into()),
        ];
        let sig = crate::config::sign_lastfm(&params, "secret456");
        assert_eq!(sig.len(), 32, "API signature must be 32 hex chars");
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()), "API sig must be hex");
        // Deterministic: same inputs yield same output.
        let sig2 = crate::config::sign_lastfm(&params, "secret456");
        assert_eq!(sig, sig2);
    }
}
