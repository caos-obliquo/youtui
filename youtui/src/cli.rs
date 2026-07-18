use crate::{Cli, OAUTH_FILENAME, RuntimeInfo, get_api, get_config_dir, get_data_dir};
use anyhow::Result;
use metadata_provider::MetadataProvider;
use futures::future::try_join_all;
use querybuilder::{CliQuery, QueryType, command_to_query};
use std::io::BufRead;
use std::path::PathBuf;
use tokio::sync::Semaphore;
use ytmapi_rs::{generate_oauth_code_and_url, generate_oauth_token};

mod querybuilder;

pub async fn handle_cli_command(cli: Cli, rt: RuntimeInfo) -> Result<()> {
    let config = rt.config;
    // Handle TestScrobble - not a YTM API command
    match &cli.command {
        Some(crate::Command::TestScrobble { artist, track, album, duration }) => {
            use crate::app::scrobbler::{ScrobbleState, submit_scrobble_inner};
            use std::time::Duration;
            let state = ScrobbleState::new(
                artist.clone(),
                track.clone(),
                album.clone(),
                None,
                Duration::from_secs(*duration),
            );
            println!("ARTIST={}", state.artist);
            println!("TRACK={}", state.track);
            println!("ALBUM={:?}", state.album);
            println!("DURATION={}s", state.duration.as_secs());
            println!("API_KEY={}", config.scrobbling.api_key);
            println!("API_SECRET_PRESENT={}", !config.scrobbling.api_secret.is_empty());
            println!("SESSION_KEY={}", config.scrobbling.session_key);
            eprintln!("--- Sending scrobble request ---");
            match submit_scrobble_inner(&config.scrobbling, &state).await {
                crate::app::scrobbler::ScrobbleResult::Success => {
                    println!("RESULT=OK (scrobble accepted)");
                }
                crate::app::scrobbler::ScrobbleResult::RateLimited => {
                    println!("RESULT=RATE_LIMITED (wait and try again)");
                }
                crate::app::scrobbler::ScrobbleResult::Failure => {
                    println!("RESULT=FAILED (check stderr for API response)");
                }
            }
            return Ok(());
        }
        Some(crate::Command::ScrobbleCache { show: _show, clear, retry }) => {
            use crate::app::scrobbler::{read_scrobble_cache_entries, clear_scrobble_cache};
            if *clear {
                clear_scrobble_cache();
                println!("Scrobble cache cleared.");
                return Ok(());
            }
            if *retry {
                println!("Retrying cached scrobbles...");
                crate::app::scrobbler::retry_failed_scrobbles(&config.scrobbling).await;
                println!("Retry complete.");
                return Ok(());
            }
            match read_scrobble_cache_entries() {
                Some(entries) if !entries.is_empty() => {
                    println!("Scrobble cache ({} entries):", entries.len());
                    for (i, e) in entries.iter().enumerate() {
                        let artist = e["artist"].as_str().unwrap_or("?");
                        let track = e["track"].as_str().unwrap_or("?");
                        let album = e["album"].as_str().unwrap_or("");
                        let retries = e["retry_count"].as_u64().unwrap_or(0);
                        println!("  {}. {} - {} ({}) retries={}", i + 1, artist, track, album, retries);
                    }
                }
                _ => println!("Scrobble cache is empty."),
            }
            return Ok(());
        }
        Some(crate::Command::TestValidateMetadata { artist, title, album }) => {
            use crate::app::server::MetadataRegistry;
            let http_client = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (metadata-test)")
                .build()?;
            let registry = MetadataRegistry::new(
                http_client,
                Some(config.scrobbling.api_key.clone()).filter(|s| !s.is_empty()),
                Some(config.scrobbling.discogs_token.clone()).filter(|s| !s.is_empty()),
                Some(config.scrobbling.genius_token.clone()).filter(|s| !s.is_empty()),
                Some(config.scrobbling.listenbrainz_token.clone()).filter(|s| !s.is_empty()),
                Some(config.scrobbling.musicbrainz_bearer_token.clone()).filter(|s| !s.is_empty()),
                Some(config.scrobbling.api_key.clone()).filter(|s| !s.is_empty()), // librefm_key
                None,
                None,
                None, // sqlite_path
            );
            println!("Resolving: {} - {}", artist, title);
            if let Some(a) = album {
                println!("Album hint: {}", a);
            }
            match registry.resolve(artist, title, album.as_deref()).await {
                Ok(meta) => {
                    println!("--- RESULT ---");
                    println!("Artist:    {:?}", meta.artist);
                    println!("Album:     {:?}", meta.album);
                    println!("Year:      {:?}", meta.year);
                    println!("Track no:  {:?}", meta.track_no);
                    println!("Tracks:    {}", meta.album_tracks.len());
                    println!("Genres:    {:?}", meta.genres);
                    println!("Styles:    {:?}", meta.styles);
                    for (i, t) in meta.album_tracks.iter().enumerate() {
                        println!("  {}. {} ({:.0}s) {:?}", i + 1, t.title, t.duration_secs, t.artist);
                    }
                }
                Err(e) => println!("ERROR: {}", e),
            }
            return Ok(());
        }
        Some(crate::Command::TestListenbrainz { artist, title }) => {
            use metadata_provider::listenbrainz::ListenBrainzProvider;
            let token = config.scrobbling.listenbrainz_token.clone();
            if token.is_empty() {
                println!("ERROR: listenbrainz_token not configured in [scrobbling] section");
                return Ok(());
            }
            let provider = ListenBrainzProvider::new(token);
            let client = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (listenbrainz-test)")
                .build()?;
            println!("Querying ListenBrainz: {} - {}", artist, title);
            match provider.lookup(artist, title, None, &client).await {
                Some(meta) => {
                    println!("Artist: {:?}", meta.artist);
                    println!("Album:  {:?}", meta.album);
                    println!("Year:   {:?}", meta.year);
                    println!("Genres: {:?}", meta.genres);
                    println!("Styles: {:?}", meta.styles);
                    println!("MBID:   {:?}", meta.musicbrainz_release_group_id);
                }
                None => println!("No result from ListenBrainz"),
            }
            return Ok(());
        }
        Some(crate::Command::TestMusicbrainz { artist, title }) => {
            use metadata_provider::musicbrainz::MusicBrainzProvider;
            let client = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (musicbrainz-test)")
                .build()?;
            let client_id = Some(config.scrobbling.musicbrainz_client_id.clone())
                .filter(|s| !s.is_empty());
            let client_secret = Some(config.scrobbling.musicbrainz_client_secret.clone())
                .filter(|s| !s.is_empty());
            let bearer = Some(config.scrobbling.musicbrainz_bearer_token.clone())
                .filter(|s| !s.is_empty());
            let provider = MusicBrainzProvider::new(client_id, client_secret, bearer);
            println!("Querying MusicBrainz: {} - {}", artist, title);
            match provider.lookup(artist, title, None, &client).await {
                Some(meta) => {
                    println!("Artist: {:?}", meta.artist);
                    println!("Album:  {:?}", meta.album);
                    println!("Year:   {:?}", meta.year);
                    println!("Tracks: {}", meta.album_tracks.len());
                    println!("Genres: {:?}", meta.genres);
                    println!("Styles: {:?}", meta.styles);
                    println!("MBID:   {:?}", meta.musicbrainz_release_group_id);
                    for (i, t) in meta.album_tracks.iter().enumerate() {
                        println!("  {}. {} ({:.0}s) {:?}", i + 1, t.title, t.duration_secs, t.artist);
                    }
                }
                None => println!("No result from MusicBrainz"),
            }
            return Ok(());
        }
        Some(crate::Command::TestCaa { release_group_id, artist, title }) => {
            let http = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (caa-test)")
                .build()?;
            let mbid = if let Some(rgid) = release_group_id {
                rgid.clone()
            } else if let (Some(a), Some(t)) = (artist, title) {
                println!("Resolving MBID for {} - {}...", a, t);
                use metadata_provider::musicbrainz::MusicBrainzProvider;
                let client_id = Some(config.scrobbling.musicbrainz_client_id.clone())
                    .filter(|s| !s.is_empty());
                let client_secret = Some(config.scrobbling.musicbrainz_client_secret.clone())
                    .filter(|s| !s.is_empty());
                let bearer = Some(config.scrobbling.musicbrainz_bearer_token.clone())
                    .filter(|s| !s.is_empty());
                let provider = MusicBrainzProvider::new(client_id, client_secret, bearer);
                match provider.lookup(&a, &t, None, &http).await {
                    Some(meta) => match meta.musicbrainz_release_group_id {
                        Some(id) => id,
                        None => { println!("No MBID found"); return Ok(()); }
                    },
                    None => { println!("MusicBrainz lookup failed"); return Ok(()); }
                }
            } else {
                println!("Provide --release-group-id OR --artist + --title");
                return Ok(());
            };
            println!("Fetching CAA for release-group: {}", mbid);
            let url = format!("https://coverartarchive.org/release-group/{}/front", mbid);
            match http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let bytes = resp.bytes().await.unwrap_or_default();
                    let len = bytes.len();
                    let path = std::env::temp_dir().join(format!("caa_{}.jpg", mbid));
                    if let Err(e) = std::fs::write(&path, &bytes) {
                        println!("Failed to write image: {}", e);
                    } else {
                        println!("OK: {} bytes -> {}", len, path.display());
                    }
                }
                Ok(resp) => println!("CAA returned HTTP {}", resp.status()),
                Err(e) => println!("CAA request failed: {}", e),
            }
            return Ok(());
        }
        Some(crate::Command::EnrichCache { file }) => {
            return handle_enrich_cache(&config, file).await;
        }
        _ => {}
    }
    match cli {
        // TODO: Block this action using type system.
        Cli {
            command: None,
            show_source: true,
            ..
        } => println!("Show source requires an associated API command"),
        Cli {
            command: None,
            input_json: Some(_),
            ..
        } => println!("API command must be provided when providing an input json file"),
        Cli {
            command: None,
            input_json: None,
            show_source: false,
        } => println!("No command provided"),
        Cli {
            command: Some(command),
            input_json: Some(input_array),
            show_source,
        } => {
            let source_futures = input_array.into_iter().map(tokio::fs::read_to_string);
            let sources = try_join_all(source_futures).await?;
            let cli_query = CliQuery {
                query_type: QueryType::FromSourceFiles(sources),
                show_source,
            };
            let api = get_api(&config).await?;
            let res = command_to_query(command, cli_query, api).await?;
            println!("{res}");
        }
        Cli {
            command: Some(command),
            input_json: None,
            show_source,
        } => {
            let cli_query = CliQuery {
                query_type: QueryType::FromApi,
                show_source,
            };
            let api = get_api(&config).await?;
            let res = command_to_query(command, cli_query, api).await?;
            println!("{res}");
        }
    }
    Ok(())
}
pub async fn get_and_output_oauth_token(
    file_name: Option<PathBuf>,
    write_to_stdout: bool,
    client_id: String,
    client_secret: String,
) -> Result<()> {
    let token_str = get_oauth_token(client_id, client_secret).await?;
    match (file_name, write_to_stdout) {
        (Some(file_name), _) => {
            tokio::fs::write(&file_name, &token_str).await?;
            println!("Wrote Oauth token to {}", file_name.display());
        }
        (None, false) => {
            let mut path = get_config_dir()?;
            path.push(OAUTH_FILENAME);
            tokio::fs::write(&path, &token_str).await?;
            println!("Wrote Oauth token to {}", path.display());
        }
        (None, true) => (),
    };
    if write_to_stdout {
        println!("{token_str}");
    }
    Ok(())
}
async fn handle_enrich_cache(config: &crate::config::Config, file: &Option<String>) -> Result<()> {
    use crate::app::server::MetadataRegistry;

    let overrides_path = get_config_dir().ok().map(|d| d.join("metadata_overrides.json"));
    let cache_dir = get_data_dir().ok();
    let cache_path = cache_dir.clone();
    let sqlite_path = cache_dir.map(|d| d.join("metadata_cache.db"));

    let http_client = reqwest::Client::builder()
        .user_agent("Youtui/0.1 (enrich-cache)")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let registry = MetadataRegistry::new(
        http_client,
        Some(config.scrobbling.api_key.clone()).filter(|s| !s.is_empty()),
        Some(config.scrobbling.discogs_token.clone()).filter(|s| !s.is_empty()),
        Some(config.scrobbling.genius_token.clone()).filter(|s| !s.is_empty()),
        Some(config.scrobbling.listenbrainz_token.clone()).filter(|s| !s.is_empty()),
        Some(config.scrobbling.musicbrainz_bearer_token.clone()).filter(|s| !s.is_empty()),
        Some(config.scrobbling.api_key.clone()).filter(|s| !s.is_empty()), // librefm_key
        overrides_path,
        cache_path,
        sqlite_path,
    );

    let lines: Vec<String> = if let Some(path) = file {
        let f = std::fs::File::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open file '{}': {}", path, e))?;
        let reader = std::io::BufReader::new(f);
        reader.lines().filter_map(|l| l.ok()).collect()
    } else {
        let stdin = std::io::stdin();
        stdin.lock().lines().filter_map(|l| l.ok()).collect()
    };

    let pairs: Vec<(String, String)> = lines
        .iter()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .filter_map(|l| {
            let trimmed = l.trim();
            let mut parts = trimmed.splitn(2, '|');
            let artist = parts.next()?.trim();
            let title = parts.next()?.trim();
            if artist.is_empty() || title.is_empty() {
                None
            } else {
                Some((artist.to_string(), title.to_string()))
            }
        })
        .collect();

    if pairs.is_empty() {
        tracing::warn!("enrich-cache: no input lines");
        println!("No input lines found. Provide 'Artist | Title' per line.");
        return Ok(());
    }

    let total = pairs.len();
    let semaphore = Semaphore::new(10);
    let mut completed = 0usize;
    let mut errors = 0usize;

    tracing::info!("enrich-cache: starting batch enrichment for {} songs", total);
    for (artist, title) in &pairs {
        let _permit = semaphore.acquire().await;
        tracing::debug!("enrich-cache: resolving {} - {}", artist, title);
        match registry.resolve_fast(artist, title, None).await {
            Some(meta) => {
                completed += 1;
                let year_str = meta.year.as_deref().unwrap_or("None");
                let genre_str = if meta.genres.is_empty() { String::new() } else { format!(" [{}]", meta.genres.join(", ")) };
                tracing::info!("enrich-cache: {} - {} -> year={:?}, genres={}, styles={}", artist, title, meta.year, meta.genres.len(), meta.styles.len());
                print!("\r[ {completed}/{total} ] {artist} - {title} -> {year_str}{genre_str}");
            }
            None => {
                errors += 1;
                completed += 1;
                tracing::debug!("enrich-cache: {} - {} -> no data", artist, title);
                print!("\r[ {completed}/{total} ] {artist} - {title} -> None");
            }
        }
        use std::io::Write;
        std::io::stdout().flush().ok();
    }

    println!();
    tracing::info!("enrich-cache: done - {} enriched, {} errors", completed, errors);
    println!("Done. {completed} enriched, {errors} errors");
    Ok(())
}

async fn get_oauth_token(client_id: String, client_secret: String) -> Result<String> {
    let client = ytmapi_rs::client::Client::new()?;
    let (code, url) = generate_oauth_code_and_url(&client, &client_id).await?;
    // Hack to wait for input
    println!("Go to {url}, finish the login flow, and press enter when done");
    let mut _buf = String::new();
    let _ = std::io::stdin().read_line(&mut _buf);
    let token = generate_oauth_token(&client, code, client_id, client_secret).await?;
    Ok(serde_json::to_string_pretty(&token)?)
}
