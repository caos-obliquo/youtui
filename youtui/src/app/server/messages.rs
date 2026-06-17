use super::ArcServer;
use ytmapi_rs::parse::LibraryPlaylist;
use super::api::GetArtistSongsProgressUpdate;
use super::player::{DecodedInMemSong, Player};
use super::song_downloader::{DownloadProgressUpdate, InMemSong};
use super::song_thumbnail_downloader::SongThumbnail;
use crate::app::server::api::GetPlaylistSongsProgressUpdate;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use crate::app::AudioQuality;
use crate::app::structures::ListSongID;
use crate::async_rodio_sink::rodio::decoder::DecoderError;
use crate::async_rodio_sink::{
    AllStopped, AutoplayUpdate, PausePlayResponse, Paused, PlayUpdate, ProgressUpdate, QueueUpdate,
    Resumed, SeekDirection, Stopped, VolumeUpdate,
};
use anyhow::{Error, Result};
use async_callback_manager::{BackendStreamingTask, BackendTask, MapFn};
use futures::{Future, Stream};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use ytmapi_rs::common::{AlbumID, ArtistChannelID, PlaylistID, SearchSuggestion, VideoID, YoutubeID};
use musixmatch_inofficial::Musixmatch;
use reqwest;
use ytmapi_rs::parse::{SearchResultArtist, SearchResultPlaylist, SearchResultSong};

#[derive(PartialEq, Debug)]
pub enum TaskMetadata {
    PlayingSong,
    PlayPause,
}

#[derive(Debug)]
pub struct HandleApiError {
    pub error: Error,
    pub message: String,
}

#[derive(Debug, PartialEq)]
pub struct GetLyrics(pub String, pub String, pub String);
#[derive(Debug, PartialEq)]
pub struct GetAnnotations(pub String, pub String, pub String);
#[derive(Debug, PartialEq)]
pub struct ValidateMetadata(pub String, pub String, pub crate::app::structures::ListSongID, pub String);

#[derive(Debug, PartialEq)]
pub struct GetSearchSuggestions(pub String);
#[derive(Debug, PartialEq)]
pub struct SearchArtists(pub String);
#[derive(Debug, PartialEq)]
pub struct SearchSongs(pub String);
#[derive(Debug, PartialEq)]
pub struct SearchPlaylists(pub String);
#[derive(Debug, PartialEq)]
pub struct GetArtistSongs(pub ArtistChannelID<'static>);
#[derive(Debug, PartialEq)]
pub struct GetPlaylistSongs {
    pub playlist_id: PlaylistID<'static>,
    pub max_songs: usize,
}

#[derive(Debug, PartialEq)]
pub struct CreatePlaylistWithVideos {
    pub title: String,
    pub description: Option<String>,
    pub video_ids: Vec<VideoID<'static>>,
}

#[derive(Debug, PartialEq)]
pub struct AddSongsToPlaylist {
    pub playlist_id: PlaylistID<'static>,
    pub video_ids: Vec<VideoID<'static>>,
}

#[derive(Debug, PartialEq)]
pub struct GetAllLibraryPlaylists;

impl BackendTask<ArcServer> for GetAllLibraryPlaylists {
    type Output = Result<Vec<LibraryPlaylist>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            use ytmapi_rs::query::GetLibraryPlaylistsQuery;

            let pages: Vec<Vec<LibraryPlaylist>> = backend
                .api
                .get_api()
                .await?
                .read()
                .await
                .stream_browser_or_oauth(GetLibraryPlaylistsQuery, 10)
                .await?;

            Ok(pages.into_iter().flatten().collect())
        }
    }
}

impl BackendTask<ArcServer> for CreatePlaylistWithVideos {
    type Output = Result<PlaylistID<'static>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            backend.api.create_playlist_with_videos(
                self.title,
                self.description,
                self.video_ids,
            ).await
        }
    }
}

impl BackendTask<ArcServer> for AddSongsToPlaylist {
    type Output = Result<()>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            backend.api.add_playlist_items(self.playlist_id, self.video_ids).await
        }
    }
}

#[derive(Debug)]
pub struct DownloadSong(pub VideoID<'static>, pub ListSongID, pub Arc<CancellationToken>, pub AudioQuality);

impl PartialEq for DownloadSong {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

// Player Requests documentation:
// NOTE: I considered giving player more control of the playback than playlist,
// and increasing message size. However this seems to be more combinatorially
// difficult without a well defined data structure.

// XXX: This should be programmed to be unkillable.
// Case:
// Cur volume: 5
// Send IncreaseVolume(5)
// Send IncreaseVolume(5), killing previous task
// Volume will now be 10 - should be 15, should not allow caller to cause this.
// New note - 2025:
// SetVolume should be able to kill IncreaseVolume however...
#[derive(PartialEq, Debug)]
pub struct IncreaseVolume(pub i8);
#[derive(Debug, PartialEq)]
pub struct SetVolume(pub u8);
/// Seek forwards or backwards a duration in a song.
#[derive(Debug, PartialEq)]
pub struct Seek {
    pub duration: Duration,
    pub direction: SeekDirection,
}
/// Seek to a target position in a song.
#[derive(Debug, PartialEq)]
pub struct SeekTo {
    pub position: Duration,
    // Unlike seeking forward or back, it would be odd if user was expecting to seek to pos x in
    // song a but due to a race condition seek applied to song b.
    pub id: ListSongID,
}
/// Stop a song if it is still currently playing.
#[derive(Debug, PartialEq)]
pub struct Stop(pub ListSongID);
/// Stop the player, regardless of what song is playing.
#[derive(Debug, PartialEq)]
pub struct StopAll;
#[derive(Debug, PartialEq)]
pub struct PausePlay(pub ListSongID);
#[derive(Debug, PartialEq)]
pub struct Resume(pub ListSongID);
#[derive(Debug, PartialEq)]
pub struct Pause(pub ListSongID);
/// Decode a song into a format that can be played.
#[derive(PartialEq, Debug)]
pub struct DecodeSong(pub Arc<InMemSong>);
/// Play a song, starting from the start, regardless what's queued.
#[derive(Debug)]
pub struct PlaySong {
    pub song: DecodedInMemSong,
    pub id: ListSongID,
}
/// Play a song, unless it's already queued.
#[derive(Debug)]
pub struct AutoplaySong {
    pub song: DecodedInMemSong,
    pub id: ListSongID,
}
/// Queue a song to play next.
#[derive(Debug)]
pub struct QueueSong {
    pub song: DecodedInMemSong,
    pub id: ListSongID,
}
#[derive(Debug, PartialEq)]
pub struct GetSongThumbnail {
    pub thumbnail_url: String,
    pub thumbnail_id: SongThumbnailID<'static>,
}

impl BackendTask<ArcServer> for HandleApiError {
    // Infallible - assumption is that even if this task fails, caller won't care.
    type Output = ();
    // TODO: Review if TaskMetadata needs new enum cases.
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let Self { error, message } = self;
        let backend = backend.clone();
        async move {
            backend.api_error_handler.handle_error(error, message).await;
        }
    }
}

impl BackendTask<ArcServer> for GetLyrics {
    type Output = Result<String>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        _backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        async move {
            let artist = self.0;
            let title = self.1;
            let genius_token = self.2;

            // Try Genius API first if token available
            if !genius_token.is_empty() {
                let search_url = format!("https://api.genius.com/search?q={}+{}",
                    urlenc(&artist), urlenc(&title));
                match reqwest::Client::new()
                    .get(&search_url)
                    .header("Authorization", format!("Bearer {}", genius_token))
                    .send().await
                {
                    Ok(resp) => {
                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                            if let Some(hit) = data.pointer("/response/hits/0/result") {
                                if let Some(api_path) = hit.get("api_path").and_then(|p| p.as_str()) {
                                    let song_url = format!("https://api.genius.com{}", api_path);
                                    tracing::info!("Genius API: fetching song {}", song_url);
                                }
                                // Use the URL from Genius to scrape lyrics (API doesn't provide lyrics)
                                if let Some(url) = hit.get("url").and_then(|u| u.as_str()) {
                                    tracing::info!("Genius API found: {}", url);
                                    // Fall through to existing Genius scrape with this URL
                                }
                            }
                        }
                    }
                    Err(e) => tracing::warn!("Genius API search error: {}", e),
                }
            }

            // Try musixmatch-inofficial first
            match Musixmatch::builder().build() {
                Ok(client) => match client.matcher_lyrics(&title, &artist).await {
                    Ok(lyrics) => return Ok(lyrics.lyrics_body),
                    Err(musixmatch_inofficial::Error::NotFound) => {
                        tracing::info!("Musixmatch: lyrics not found, trying lyr fallback");
                    }
                    Err(e) => {
                        tracing::warn!("Musixmatch error: {}, trying lyr fallback", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to build Musixmatch client: {}", e);
                }
            }

            // Normalize title: lowercase, collapse whitespace, strip non-alphanumeric
            fn normalize(s: &str) -> String {
                s.to_lowercase()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            tracing::info!("Lyrics fallback: artist='{}', title='{}'", &artist, &title);

            // Try Genius search + scrape direct (most reliable, bypasses lyr matching)
            fn urlenc(s: &str) -> String {
                s.split_whitespace().collect::<Vec<_>>().join("+")
            }
            let search_url = format!(
                "https://genius.com/api/search/song?q={}+{}",
                urlenc(&title),
                urlenc(&artist)
            );
            match reqwest::get(&search_url).await {
                Ok(resp) => {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if let Some(hit) = data.pointer("/response/sections/0/hits/0/result") {
                            if let Some(url) = hit.get("url").and_then(|u| u.as_str()) {
                                tracing::info!("Genius found: {}", url);
                                // Fetch the Genius page and extract lyrics
                                if let Ok(page) = reqwest::get(url).await {
                                    if let Ok(html) = page.text().await {
                                        // Extract lyrics from Genius page data-lyrics-container divs
                                        let mut all_lyrics = String::new();
                                        for part in html.split("data-lyrics-container=\"true\"").skip(1) {
                                            if let Some(start) = part.find(">") {
                                                let inside = &part[start + 1..];
                                                // Find closing </div> of this container
                                                let content = inside.split("</div>").next().unwrap_or(inside);
                                                // Strip HTML tags, decode entities
                                                let mut in_tag = false;
                                                for ch in content.chars() {
                                                    match ch {
                                                        '<' => in_tag = true,
                                                        '>' if in_tag => { in_tag = false; all_lyrics.push('\n'); }
                                                        _ if !in_tag => all_lyrics.push(ch),
                                                        _ => {}
                                                    }
                                                }
                                                all_lyrics.push('\n');
                                            }
                                        }
                                        let cleaned: String = all_lyrics
                                            .replace("&quot;", "\"").replace("&#x27;", "'")
                                            .replace("&#x2019;", "'").replace("&amp;", "&")
                                            .replace("&lt;", "<").replace("&gt;", ">")
                                            .replace("&#x2014;", "--").replace("&#x2013;", "-");
                                        let raw_lines: Vec<&str> = cleaned.lines().collect();
                                        let mut merged: Vec<String> = Vec::new();
                                        for line in raw_lines {
                                            let t = line.trim();
                                            if t.is_empty() || t.contains("Contributors") || t.contains("You might also like") {
                                                continue;
                                            }
                                            if t == "(" || t == ")" || t.len() <= 2 && (t.contains('(') || t.contains(')')) {
                                                if let Some(last) = merged.last_mut() {
                                                    if t == "(" { last.push_str(" ("); }
                                                    else { last.push(')'); }
                                                }
                                            } else {
                                                merged.push(t.to_string());
                                            }
                                        }
                                        let cleaned: String = merged.join("\n");
                                        // Only accept Genius result if substantial (>50 chars) and not just the song title
                                        if cleaned.len() > 50 && cleaned.lines().count() > 2 {
                                            tracing::info!("Genius scrape: {} chars", cleaned.len());
                                            return Ok(cleaned);
                                        } else {
                                            tracing::info!("Genius scrape too short ({} chars), falling through", cleaned.len());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => tracing::warn!("Genius search failed: {}", e),
            }

            // Try bandcamp with constructed URL before lyr CLI fallback
            fn bc_slug(s: &str) -> (String, String) {
                let with = s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').collect::<String>()
                    .split_whitespace().collect::<Vec<_>>().join("-");
                let without = with.replace('-', "");
                (with, without)
            }
            let (bc_artist_hyphen, bc_artist_none) = bc_slug(&artist);
            let (bc_song_hyphen, _) = bc_slug(&title);
            let bc_artists = [bc_artist_hyphen.as_str(), bc_artist_none.as_str()];
            for artist_slug in &bc_artists {
                for suffix in &["", "-2", "-3", "-4", "-5"] {
                    let bc_url = format!("https://{}.bandcamp.com/track/{}{}", artist_slug, bc_song_hyphen, suffix);
                    tracing::info!("Trying bandcamp URL: {}", bc_url);
                    match tokio::process::Command::new("bandcamp-lyrics")
                        .arg(&bc_url)
                        .output()
                        .await
                    {
                        Ok(out) if out.status.success() => {
                            let lyrics = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            if !lyrics.is_empty() {
                                tracing::info!("bandcamp found lyrics ({} chars)", lyrics.len());
                                return Ok(lyrics);
                            }
                        }
                        _ => continue,
                    }
                }
            }

            // Fallback to lyr with original artist (try multiple variants)
            let norm_artist = normalize(&artist);
            let norm_title = normalize(&title);
            let first_artist = artist.split(',').next().unwrap_or(&artist).trim().to_string();
            let two_artists = artist.splitn(3, ',').take(2).collect::<Vec<_>>().join(" and ").trim().to_string();
            let variants: Vec<(&str, &str)> = vec![
                (&artist, &title),
                (&first_artist, &title),
                (&two_artists, &title),
                (&first_artist, &norm_title),
                (&norm_artist, &title),
                (&norm_artist, &norm_title),
            ];

            for (artist_name, song_title) in &variants {
                let output = tokio::process::Command::new("lyr")
                    .args(["--artist", artist_name, "--title", song_title])
                    .output()
                    .await;
                match output {
                    Ok(out) if out.status.success() => {
                        let raw = String::from_utf8_lossy(&out.stdout).to_string();
                        let lyrics = raw.lines().skip(1).collect::<Vec<_>>().join("\n");
                        let lyrics = lyrics.splitn(2, "Lyrics").nth(1).unwrap_or(&lyrics).trim().to_string();
                        if !lyrics.is_empty() {
                            return Ok(lyrics);
                        }
                    }
                    _ => continue,
                }
            }

            Err(anyhow::anyhow!("No lyrics found from any provider"))
        }
    }
}

impl BackendTask<ArcServer> for GetSearchSuggestions {
    // TODO: Consider alternative where the text isn't returned back to the caller.
    type Output = Result<(Vec<SearchSuggestion>, String)>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.api.get_search_suggestions(self.0).await }
    }
}
impl BackendTask<ArcServer> for SearchArtists {
    type Output = Result<Vec<SearchResultArtist>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.api.search_artists(self.0).await }
    }
}
impl BackendTask<ArcServer> for SearchSongs {
    type Output = Result<Vec<SearchResultSong>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let query = self.0;
        let backend = backend.clone();
        async move {
            // Try YTMusic first
            match backend.api.search_songs(query.clone()).await {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => tracing::info!("YTMusic no results, trying YouTube fallback for: {}", query),
                Err(e) => tracing::warn!("YTMusic search error: {}, trying YouTube fallback", e),
            }
            // Fallback: yt-dlp YouTube search
            let output = tokio::process::Command::new("yt-dlp")
                .args([
                    "--flat-playlist", "--dump-json", "--no-warnings",
                    &format!("ytsearch10:{}", query),
                ])
                .output()
                .await
                .map_err(|e| anyhow::anyhow!("yt-dlp search failed: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let results: Vec<SearchResultSong> = stdout.lines()
                .filter_map(|line| {
                    let v: serde_json::Value = serde_json::from_str(line).ok()?;
                    let title = v.get("title")?.as_str()?;
                    let uploader = v.get("uploader").and_then(|u| u.as_str()).unwrap_or("Unknown");
                    let id = v.get("id")?.as_str()?;
                    let duration = v.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0) as u64;
                    let vid: VideoID<'static> = VideoID::from_raw(id.to_string());
                    let album_id: AlbumID<'static> = AlbumID::from_raw(id.to_string());
                    Some(ytmapi_rs::parse::SearchResultSong::from_yt_dlp(
                        title.to_string(),
                        uploader.to_string(),
                        vid,
                        Some(ytmapi_rs::parse::ParsedSongAlbum {
                            name: format!("YouTube: {}", uploader),
                            id: album_id,
                        }),
                        format!("{}", duration as u64),
                    ))
                })
                .collect();
            Ok(results)
        }
    }
}
impl BackendTask<ArcServer> for GetAnnotations {
    type Output = Result<Vec<(String, String)>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        _backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        async move {
            let artist = self.0;
            let title = self.1;
            let token = self.2;

            // Search Genius API for song
            let search_url = format!("https://api.genius.com/search?q={}+{}",
                artist.split_whitespace().collect::<Vec<_>>().join("+"),
                title.split_whitespace().collect::<Vec<_>>().join("+"));
            let client = reqwest::Client::new();
            let resp = client.get(&search_url)
                .header("Authorization", format!("Bearer {}", token))
                .send().await.map_err(|e| anyhow::anyhow!("Genius API error: {}", e))?;
            let data: serde_json::Value = resp.json().await.map_err(|e| anyhow::anyhow!("JSON error: {}", e))?;
            let song_id = data.pointer("/response/hits/0/result/id")
                .and_then(|id| id.as_u64())
                .ok_or_else(|| anyhow::anyhow!("No Genius results"))?;

            // Fetch referents (annotations) for this song
            let ref_url = format!("https://api.genius.com/referents?song_id={}", song_id);
            let ref_resp = client.get(&ref_url)
                .header("Authorization", format!("Bearer {}", token))
                .send().await.map_err(|e| anyhow::anyhow!("Referents error: {}", e))?;
            let ref_data: serde_json::Value = ref_resp.json().await.map_err(|e| anyhow::anyhow!("JSON error: {}", e))?;

            let mut annotations = Vec::new();
            if let Some(refs) = ref_data.pointer("/response/referents").and_then(|r| r.as_array()) {
                for referent in refs {
                    let fragment = referent.get("fragment").and_then(|f| f.as_str()).unwrap_or("").to_string();
                    let body = referent.pointer("/annotations/0/body/dom")
                        .and_then(|d| extract_text_from_dom(d));
                    if !fragment.is_empty() && !body.as_deref().unwrap_or("").is_empty() {
                        annotations.push((fragment, body.unwrap_or_default()));
                    }
                }
            }
            tracing::info!("Fetched {} annotations for song {}", annotations.len(), song_id);
            Ok(annotations)
        }
    }
}
fn extract_text_from_dom(dom: &serde_json::Value) -> Option<String> {
    match dom {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(m) => {
            if let Some(children) = m.get("children").and_then(|c| c.as_array()) {
                let mut texts = Vec::new();
                for child in children {
                    if let Some(t) = extract_text_from_dom(child) {
                        texts.push(t);
                    }
                }
                if texts.is_empty() { None } else { Some(texts.join(" ")) }
            } else {
                None
            }
        }
        _ => None,
    }
}
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ValidatedMetadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub track_no: Option<usize>,
}

impl BackendTask<ArcServer> for ValidateMetadata {
    type Output = Result<ValidatedMetadata>;
    type MetadataType = TaskMetadata;
    fn into_future(self, _backend: &ArcServer) -> impl Future<Output = Self::Output> + Send + 'static {
        async move {
            let artist = self.0;
            let title = self.1;
            let _song_id = self.2;
            let lastfm_key = self.3;
            let client = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (music-player)")
                .build()?;

            // Last.fm: track.getInfo first, fallback to track.search
            if !lastfm_key.is_empty() {
                // Try exact match first
                let lfm_url = format!(
                    "https://ws.audioscrobbler.com/2.0/?method=track.getInfo&api_key={}&artist={}&track={}&format=json",
                    lastfm_key, urlencoding(&artist), urlencoding(&title)
                );
                let mut found = None;
                if let Ok(resp) = client.get(&lfm_url).send().await {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if let Some(track) = data.get("track") {
                            let album = track.get("album").and_then(|a| a.get("title")).and_then(|t| t.as_str()).map(|s| s.to_string());
                            let artist_name = track.get("artist").and_then(|a| a.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string());
                            let year = track.get("wiki")
                                .and_then(|w| w.get("published"))
                                .and_then(|p| p.as_str())
                                .and_then(|d| d.get(..4))
                                .map(|s| s.to_string());
                                                        let track_no = track.get("album").and_then(|a| a.get("@attr")).and_then(|a| a.get("rank")).and_then(|r| r.as_str()).and_then(|s| s.parse::<usize>().ok());
found = Some(ValidatedMetadata { artist: artist_name, album, year, track_no });
                        }
                    }
                }
                if let Some(ref meta) = found {
                    if meta.album.is_some() || meta.year.is_some() {
                        return Ok(meta.clone());
                    }
                }

                // Fallback: search by track name only, then fetch best match's full info
                let search_url = format!(
                    "https://ws.audioscrobbler.com/2.0/?method=track.search&api_key={}&track={}&format=json&limit=5",
                    lastfm_key, urlencoding(&title)
                );
                if let Ok(resp) = client.get(&search_url).send().await {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if let Some(results) = data.get("results").and_then(|r| r.get("trackmatches")).and_then(|m| m.get("track")).and_then(|t| t.as_array()) {
                            for result in results {
                                let result_artist = result.get("artist").and_then(|a| a.as_str()).unwrap_or("");
                                let result_name = result.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                // Re-fetch with exact artist+name to get album/year
                                let info_url = format!(
                                    "https://ws.audioscrobbler.com/2.0/?method=track.getInfo&api_key={}&artist={}&track={}&format=json",
                                    lastfm_key, urlencoding(result_artist), urlencoding(result_name)
                                );
                                if let Ok(resp) = client.get(&info_url).send().await {
                                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                                        if let Some(track) = data.get("track") {
                                            let album = track.get("album").and_then(|a| a.get("title")).and_then(|t| t.as_str()).map(|s| s.to_string());
                                            let artist_name = track.get("artist").and_then(|a| a.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string());
                                            let year = track.get("wiki")
                                                .and_then(|w| w.get("published"))
                                                .and_then(|p| p.as_str())
                                                .and_then(|d| d.get(..4))
                                                .map(|s| s.to_string());
                                            if album.is_some() || year.is_some() {
                                                let track_no = track.get("album").and_then(|a| a.get("@attr")).and_then(|a| a.get("rank")).and_then(|r| r.as_str()).and_then(|s| s.parse::<usize>().ok());
                                                return Ok(ValidatedMetadata { artist: artist_name, album, year, track_no });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // MusicBrainz fallback: 1 req/s, no auth
            tokio::time::sleep(Duration::from_millis(1200)).await;
            let mb_url = format!(
                "https://musicbrainz.org/ws/2/recording?query=artist:%22{}%22+AND+recording:%22{}%22&fmt=json",
                urlencoding(&artist), urlencoding(&title)
            );
            if let Ok(resp) = client.get(&mb_url).header("Accept", "application/json").send().await {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    let mb_meta = {
                        let rec = match data.get("recordings").and_then(|a| a.as_array()).and_then(|a| a.first()) {
                            Some(r) => r,
                            None => { return Ok(ValidatedMetadata::default()); }
                        };
                        let artist_name = rec.get("artist-credit").and_then(|a| a.as_array()).and_then(|a| a.first())
                            .and_then(|c| c.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string());
                        let year = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
                            .and_then(|r| r.get("date")).and_then(|d| d.as_str()).and_then(|d| d.get(..4)).map(|s| s.to_string());
                        let album = rec.get("releases").and_then(|a| a.as_array()).and_then(|a| a.first())
                            .and_then(|r| r.get("title")).and_then(|t| t.as_str()).map(|s| s.to_string());
                        ValidatedMetadata { artist: artist_name, album, year, track_no: None }
                    };
                    if mb_meta.album.is_some() || mb_meta.year.is_some() {
                        return Ok(mb_meta);
                    }
                }
            }

            Ok(ValidatedMetadata::default())
        }
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            ' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", c as u8));
            }
        }
    }
    out
}

impl BackendTask<ArcServer> for SearchPlaylists {
    type Output = Result<Vec<SearchResultPlaylist>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.api.search_playlists(self.0).await }
    }
}
impl BackendStreamingTask<ArcServer> for GetArtistSongs {
    type Output = GetArtistSongsProgressUpdate;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl futures::Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.api.get_artist_songs(self.0)
    }
}
impl BackendStreamingTask<ArcServer> for GetPlaylistSongs {
    type Output = GetPlaylistSongsProgressUpdate;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl futures::Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend
            .api
            .get_playlist_songs(self.playlist_id, self.max_songs)
    }
}

impl BackendStreamingTask<ArcServer> for DownloadSong {
    type Output = DownloadProgressUpdate;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl futures::Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.song_downloader.download_song(self.0, self.1, Some(self.2), self.3)
    }
}
impl BackendTask<ArcServer> for Seek {
    type Output = Option<ProgressUpdate<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.seek(self.duration, self.direction).await }
    }
}
impl BackendTask<ArcServer> for SeekTo {
    type Output = Option<ProgressUpdate<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.seek_to(self.position, self.id).await }
    }
}
impl BackendTask<ArcServer> for DecodeSong {
    type Output = std::result::Result<DecodedInMemSong, DecoderError>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        _backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        Player::try_decode(self.0)
    }
}
impl BackendTask<ArcServer> for IncreaseVolume {
    type Output = Option<VolumeUpdate>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.increase_volume(self.0).await }
    }
}
impl BackendTask<ArcServer> for SetVolume {
    type Output = Option<VolumeUpdate>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.set_volume(self.0).await }
    }
}
impl BackendTask<ArcServer> for Stop {
    type Output = Option<Stopped<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.stop(self.0).await }
    }
}
impl BackendTask<ArcServer> for StopAll {
    type Output = Option<AllStopped>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.stop_all().await }
    }
}
impl BackendTask<ArcServer> for PausePlay {
    type Output = Option<PausePlayResponse<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.pause_play(self.0).await }
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayPause]
    }
}
impl BackendTask<ArcServer> for Resume {
    type Output = Option<Resumed<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.resume(self.0).await }
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayPause]
    }
}
impl BackendTask<ArcServer> for Pause {
    type Output = Option<Paused<ListSongID>>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move { backend.player.pause(self.0).await }
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayPause]
    }
}

impl BackendStreamingTask<ArcServer> for PlaySong {
    type Output = PlayUpdate<ListSongID>;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.player.play_song(self.song, self.id)
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayingSong]
    }
}
impl BackendStreamingTask<ArcServer> for AutoplaySong {
    type Output = AutoplayUpdate<ListSongID>;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.player.autoplay_song(self.song, self.id)
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayingSong]
    }
}
impl BackendStreamingTask<ArcServer> for QueueSong {
    type Output = QueueUpdate<ListSongID>;
    type MetadataType = TaskMetadata;
    fn into_stream(
        self,
        backend: &ArcServer,
    ) -> impl Stream<Item = Self::Output> + Send + Unpin + 'static {
        let backend = backend.clone();
        backend.player.queue_song(self.song, self.id)
    }
    fn metadata() -> Vec<Self::MetadataType> {
        vec![TaskMetadata::PlayingSong]
    }
}
impl BackendTask<ArcServer> for GetSongThumbnail {
    type Output = anyhow::Result<SongThumbnail>;
    type MetadataType = TaskMetadata;
    fn into_future(
        self,
        backend: &ArcServer,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let backend = backend.clone();
        async move {
            backend
                .song_thumbnail_downloader
                .download_song_thumbnail(self.thumbnail_id, self.thumbnail_url)
                .await
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct PlayDecodedSong(pub ListSongID);
impl MapFn<DecodedInMemSong> for PlayDecodedSong {
    type Output = PlaySong;
    fn apply(self, input: DecodedInMemSong) -> Self::Output {
        tracing::info!("Song decoded succesfully. {:?}", self.0);
        PlaySong {
            song: input,
            id: self.0,
        }
    }
}
#[derive(PartialEq, Debug)]
pub struct AutoplayDecodedSong(pub ListSongID);
impl MapFn<DecodedInMemSong> for AutoplayDecodedSong {
    type Output = AutoplaySong;
    fn apply(self, input: DecodedInMemSong) -> Self::Output {
        tracing::info!("Song decoded succesfully. {:?}", self.0);
        AutoplaySong {
            song: input,
            id: self.0,
        }
    }
}
#[derive(PartialEq, Debug)]
pub struct QueueDecodedSong(pub ListSongID);
impl MapFn<DecodedInMemSong> for QueueDecodedSong {
    type Output = QueueSong;
    fn apply(self, input: DecodedInMemSong) -> Self::Output {
        tracing::info!("Song decoded succesfully. {:?}", self.0);
        QueueSong {
            song: input,
            id: self.0,
        }
    }
}

/// It's not possible to compare some of these Tasks type due to the underlying
/// type, but because tests and some ci run with async_callback_manager's
/// "task-equality" enabled, a PartialEq impl is required. It's acceptable to
/// panic as running .eq() on these types is a logic error AND should only occur
/// during testing.
#[cfg(any(test, clippy))]
#[allow(unexpected_cfgs)]
mod test_config {
    use crate::app::server::{AutoplaySong, HandleApiError, PlaySong, QueueSong};

    impl PartialEq for HandleApiError {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare HandleApiError");
        }
    }
    impl PartialEq for PlaySong {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare PlaySong");
        }
    }
    impl PartialEq for AutoplaySong {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare AutoplaySong");
        }
    }
    impl PartialEq for QueueSong {
        fn eq(&self, _: &Self) -> bool {
            panic!("Unable to compare QueueSong");
        }
    }
}
