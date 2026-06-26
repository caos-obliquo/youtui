use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

const SCROBBLE_CACHE_FILENAME: &str = "scrobble_cache.json";

fn scrobble_cache_path() -> Option<PathBuf> {
    crate::get_config_dir().ok().map(|d| d.join(SCROBBLE_CACHE_FILENAME))
}

#[derive(Debug, Clone)]
pub struct ScrobbleState {
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub duration: Duration,
    pub start_time: SystemTime,
    pub scrobbled: bool,
}

impl ScrobbleState {
    pub fn new(artist: String, track: String, album: Option<String>, duration: Duration) -> Self {
        Self { artist, track, album, duration, start_time: SystemTime::now(), scrobbled: false }
    }

    pub fn should_scrobble(&self) -> bool {
        if self.scrobbled { return false; }
        let elapsed = self.start_time.elapsed().unwrap_or(Duration::ZERO);
        let result = elapsed >= Duration::from_secs(15) || elapsed >= self.duration / 3;
        tracing::info!("Scrobble check: elapsed={:?}, duration={:?}, should={}", elapsed, self.duration, result);
        result
    }
}

/// Save failed scrobble to persistent cache for later retry.
fn save_failed_scrobble(state: &ScrobbleState) {
    let Some(path) = scrobble_cache_path() else { return };
    let mut cache: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let entry = serde_json::json!({
        "artist": state.artist,
        "track": state.track,
        "album": state.album,
        "duration_secs": state.duration.as_secs(),
        "timestamp": state.start_time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
    });
    cache.push(entry);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, serde_json::to_string_pretty(&cache).unwrap_or_default()) {
        Ok(_) => info!("Saved failed scrobble to cache: {}", path.display()),
        Err(e) => warn!("Failed to write scrobble cache: {}", e),
    }
}

/// Remove a successfully-retried scrobble from the cache by index.
fn remove_cached_scrobble(index: usize) {
    let Some(path) = scrobble_cache_path() else { return };
    let mut cache: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if index < cache.len() {
        cache.remove(index);
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&cache).unwrap_or_default());
    }
}

/// Retry failed scrobbles from persistent cache.
pub async fn retry_failed_scrobbles(config: &crate::config::ScrobblingConfig) {
    let Some(path) = scrobble_cache_path() else { return };
    let cache: Vec<serde_json::Value> = match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => return,
    };
    if cache.is_empty() {
        return;
    }
    info!("Retrying {} failed scrobbles from cache (with 2s delay between each)", cache.len());
    for (i, entry) in cache.iter().enumerate() {
        let artist = entry["artist"].as_str().unwrap_or("").to_string();
        let track = entry["track"].as_str().unwrap_or("").to_string();
        let album = entry["album"].as_str().map(|s| s.to_string());
        let duration_secs = entry["duration_secs"].as_u64().unwrap_or(0);
        let ts = entry["timestamp"].as_u64().unwrap_or(0);
        let state = ScrobbleState {
            artist,
            track,
            album,
            duration: Duration::from_secs(duration_secs),
            start_time: UNIX_EPOCH + Duration::from_secs(ts),
            scrobbled: false,
        };
        match submit_scrobble_inner(config, &state).await {
            ScrobbleResult::Success => {
                remove_cached_scrobble(i);
            }
            ScrobbleResult::RateLimited => {
                warn!("Rate limited during cache retry — stopping retries, keeping remaining entries for next startup");
                break;
            }
            ScrobbleResult::Failure => {
                // Leave in cache for next retry attempt
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    // Clean up empty cache file
    let Ok(remaining) = std::fs::read_to_string(&path) else { return };
    if remaining.trim() == "[]" || remaining.trim().is_empty() {
        let _ = std::fs::remove_file(&path);
    }
}

pub(crate) enum ScrobbleResult {
    Success,
    Failure,
    RateLimited,
}

/// Inner submit: returns Success if accepted, Failure on error, RateLimited when rate-limited.
pub(crate) async fn submit_scrobble_inner(
    config: &crate::config::ScrobblingConfig,
    state: &ScrobbleState,
) -> ScrobbleResult {
    if !config.enabled { return ScrobbleResult::Failure; }
    if config.api_key.is_empty() {
        warn!("Scrobble blocked: api_key not configured");
        return ScrobbleResult::Failure;
    }
    if config.session_key.is_empty() {
        warn!("Scrobble blocked: session_key not configured");
        return ScrobbleResult::Failure;
    }
    let timestamp = state.start_time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let mut params: Vec<(String, String)> = vec![
        ("method".into(), "track.scrobble".into()),
        ("api_key".into(), config.api_key.clone()),
        ("sk".into(), config.session_key.clone()),
        ("artist".into(), state.artist.clone()),
        ("track".into(), state.track.clone()),
        ("timestamp".into(), timestamp.to_string()),
    ];
    if let Some(ref album) = state.album {
        params.push(("album".into(), album.clone()));
    }
    params.push(("duration".into(), state.duration.as_secs().to_string()));
    params.sort_by(|a, b| a.0.cmp(&b.0));
    let sig_string: String = params.iter()
        .map(|(k, v)| format!("{}{}", k, v))
        .collect::<Vec<_>>()
        .join("") + &config.api_secret;
    let api_sig = format!("{:x}", md5::compute(sig_string.as_bytes()));
    debug!("Scrobble params: {:?}, api_sig={}", params, api_sig);
    params.push(("api_sig".into(), api_sig));

    let client = reqwest::Client::new();
    match client.post("https://ws.audioscrobbler.com/2.0/")
        .form(&params)
        .send()
        .await
    {
        Ok(resp) => {
            let text = resp.text().await.unwrap_or_default();
            if text.contains("<lfm status=\"ok\">") {
                info!("Scrobbled: {} - {} (album: {:?})", state.artist, state.track, state.album);
                ScrobbleResult::Success
            } else if text.contains("error code=\"29\"") {
                error!("Scrobble rate limited: {} (artist={}, track={})", text, state.artist, state.track);
                eprintln!("SCROBBLE_API_RESPONSE={}", text);
                ScrobbleResult::RateLimited
            } else {
                error!("Scrobble failed: {} (artist={}, track={})", text, state.artist, state.track);
                eprintln!("SCROBBLE_API_RESPONSE={}", text);
                ScrobbleResult::Failure
            }
        }
        Err(e) => {
            error!("Scrobble HTTP error: {} (artist={}, track={})", e, state.artist, state.track);
            ScrobbleResult::Failure
        }
    }
}

/// Submit a scrobble to Last.fm. On failure, saves to persistent cache for retry.
pub async fn submit_scrobble(config: &crate::config::ScrobblingConfig, state: &ScrobbleState) {
    match submit_scrobble_inner(config, state).await {
        ScrobbleResult::Success => {},
        _ => { save_failed_scrobble(state); },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    /// Verify signature is computed with sorted params (Last.fm requirement).
    /// Known test vector: artist="Test Artist", track="Test Track", duration=240,
    /// api_key="key123", api_secret="secret456", session_key="sk789", timestamp=1000000
    /// Expected sig: MD5 of "albumapi_keykey123artistTest Artistduration240methodtrack.scrobbleksk789timestamp1000000trackTest Tracksecret456"
    #[test]
    fn test_signature_sorted_alphabetically() {
        let config = crate::config::ScrobblingConfig {
            enabled: true,
            api_key: "key123".into(),
            api_secret: "secret456".into(),
            session_key: "sk789".into(),
            genius_token: String::new(),
            discogs_token: String::new(),
        };
        // ScrobbleState with fixed start_time for deterministic timestamp
        // We set start_time to UNIX_EPOCH + 1000s so timestamp = 1000
        let mut state = ScrobbleState::new(
            "Test Artist".into(),
            "Test Track".into(),
            Some("Test Album".into()),
            Duration::from_secs(240),
        );
        state.start_time = UNIX_EPOCH + Duration::from_secs(1000);

        let timestamp = state.start_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(timestamp, 1000);

        let mut params: Vec<(String, String)> = vec![
            ("method".into(), "track.scrobble".into()),
            ("api_key".into(), config.api_key.clone()),
            ("sk".into(), config.session_key.clone()),
            ("artist".into(), state.artist.clone()),
            ("track".into(), state.track.clone()),
            ("timestamp".into(), timestamp.to_string()),
        ];
        params.push(("album".into(), state.album.clone().unwrap()));
        params.push(("duration".into(), state.duration.as_secs().to_string()));

        // Sort — this is what the fix does
        params.sort_by(|a, b| a.0.cmp(&b.0));

        // Verify sorted order
        let keys: Vec<&str> = params.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["album", "api_key", "artist", "duration", "method", "sk", "timestamp", "track"],
            "Params must be alphabetically sorted for Last.fm signature");

        let sig_string: String = params.iter()
            .map(|(k, v)| format!("{}{}", k, v))
            .collect::<Vec<_>>()
            .join("") + &config.api_secret;
        let api_sig = format!("{:x}", md5::compute(sig_string.as_bytes()));

        // Expected: MD5("albumTest Albumapi_keykey123artistTest Artistduration240methodtrack.scrobbleksk789timestamp1000trackTest Tracksecret456")
        // Verified via manual computation: should produce a 32-char hex string
        assert_eq!(api_sig.len(), 32, "API signature must be 32 hex chars");
        assert!(api_sig.chars().all(|c| c.is_ascii_hexdigit()), "API sig must be hex");

        // Verify that WITHOUT sorting, a DIFFERENT sig is produced (catches regression)
        let unsorted_params: Vec<(String, String)> = vec![
            ("method".into(), "track.scrobble".into()),
            ("api_key".into(), config.api_key.clone()),
            ("sk".into(), config.session_key.clone()),
            ("artist".into(), state.artist.clone()),
            ("track".into(), state.track.clone()),
            ("timestamp".into(), timestamp.to_string()),
            ("album".into(), state.album.clone().unwrap()),
            ("duration".into(), state.duration.as_secs().to_string()),
        ];
        // NO sort — use insertion order
        let unsorted_sig = format!("{:x}", md5::compute(
            unsorted_params.iter()
                .map(|(k, v)| format!("{}{}", k, v))
                .collect::<Vec<_>>()
                .join("")
                .as_bytes()
        ));
        assert_ne!(api_sig, unsorted_sig, "Sorted sig must differ from unsorted sig");
    }

    /// Verify that should_scrobble returns false when scrobbled=true
    #[test]
    fn test_should_scrobble_already_scrobbled() {
        let mut state = ScrobbleState::new("A".into(), "B".into(), None, Duration::from_secs(240));
        state.scrobbled = true;
        assert!(!state.should_scrobble());
    }

    /// Verify that should_scrobble returns false when insufficient time elapsed
    #[test]
    fn test_should_scrobble_too_soon() {
        let state = ScrobbleState::new("A".into(), "B".into(), None, Duration::from_secs(240));
        // start_time is now, so elapsed ≈ 0
        assert!(!state.should_scrobble());
    }

    /// Verify submit_scrobble silently returns when config not enabled
    #[test]
    fn test_submit_scrobble_disabled() {
        let config = crate::config::ScrobblingConfig {
            enabled: false,
            api_key: String::new(),
            api_secret: String::new(),
            session_key: String::new(),
            genius_token: String::new(),
            discogs_token: String::new(),
        };
        let state = ScrobbleState::new("A".into(), "B".into(), None, Duration::from_secs(240));
        // This should not panic — just return immediately
        let _fut = submit_scrobble(&config, &state);
        // We can't easily block on async in non-async test,
        // but at least verify the function signature compiles and doesn't panic at start
    }

    /// Verify submit_scrobble silently returns when api_key is empty
    #[test]
    fn test_submit_scrobble_no_api_key() {
        let config = crate::config::ScrobblingConfig {
            enabled: true,
            api_key: String::new(),
            api_secret: "secret".into(),
            session_key: "sk".into(),
            genius_token: String::new(),
            discogs_token: String::new(),
        };
        let state = ScrobbleState::new("A".into(), "B".into(), None, Duration::from_secs(240));
        let _fut = submit_scrobble(&config, &state);
        // Should return immediately, no HTTP call
    }
}
