use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info};

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

pub async fn submit_scrobble(config: &crate::config::ScrobblingConfig, state: &ScrobbleState) {
    if !config.enabled || config.api_key.is_empty() || config.session_key.is_empty() {
        return;
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

    // Last.fm requires params sorted alphabetically before signing
    params.sort_by(|a, b| a.0.cmp(&b.0));
    let sig_string: String = params.iter()
        .map(|(k, v)| format!("{}{}", k, v))
        .collect::<Vec<_>>()
        .join("") + &config.api_secret;
    let api_sig = format!("{:x}", md5::compute(sig_string.as_bytes()));
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
            } else {
                error!("Scrobble failed: {} (artist={}, track={})", text, state.artist, state.track);
            }
        }
        Err(e) => error!("Scrobble HTTP error: {} (artist={}, track={})", e, state.artist, state.track),
    }
}
