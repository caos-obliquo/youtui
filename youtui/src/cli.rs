use crate::{Cli, OAUTH_FILENAME, RuntimeInfo, get_api, get_config_dir, get_data_dir};
use anyhow::{Context, Result};
use tracing::info;
use metadata_cache_sqlite::SqliteCache;
use metadata_provider::MetadataProvider;
use futures::future::try_join_all;
use querybuilder::{CliQuery, QueryType, command_to_query};
use std::io::BufRead;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::Semaphore;
use indicatif::{ProgressBar, ProgressStyle};
use ytmapi_rs::{generate_oauth_code_and_url, generate_oauth_token};

mod querybuilder;

/// Default network timeout for any single async CLI operation.
const CLI_TIMEOUT: Duration = Duration::from_secs(30);

/// Generous budget for fan-out operations that perform many sequential
/// upstream calls (e.g. the niche recommendations scan which fetches one or
/// more `getSimilar`/`getInfo` responses per seed/candidate). A single 30s
/// window is too small for ~140 network round-trips.
const NICHE_TIMEOUT: Duration = Duration::from_secs(600);

/// Wrap a Result-returning async operation in a 30s timeout.
async fn with_timeout<F, T>(ctx: &str, fut: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    with_timeout_dur(CLI_TIMEOUT, ctx, fut).await
}

async fn with_timeout_dur<F, T>(dur: Duration, ctx: &str, fut: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    match tokio::time::timeout(dur, fut).await {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!(
            "[ERROR] {}: operation timed out after {}s (network failure or unresponsive server)",
            ctx,
            dur.as_secs()
        )),
    }
}

/// Wrap an Option-returning async operation in a 30s timeout. On timeout the
/// operation is treated as having returned no data (None) with a logged error.
async fn with_timeout_opt<F, T>(ctx: &str, fut: F) -> Option<T>
where
    F: std::future::Future<Output = Option<T>>,
{
    match tokio::time::timeout(CLI_TIMEOUT, fut).await {
        Ok(Some(res)) => Some(res),
        Ok(None) => None,
        Err(_) => {
            eprintln!("[ERROR] {}: operation timed out after {}s", ctx, CLI_TIMEOUT.as_secs());
            None
        }
    }
}

fn make_progress_bar(total: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(msg.to_string());
    pb
}

/// Validate that a required string argument is non-empty.
fn require_non_empty(arg: &str, name: &str) -> Result<()> {
    if arg.trim().is_empty() {
        Err(anyhow::anyhow!(
            "[ERROR] argument validation: '{}' must not be empty",
            name
        ))
    } else {
        Ok(())
    }
}

/// Validate a UUID v4-shaped string (used for release-group IDs etc).
fn validate_uuid(arg: &str, name: &str) -> Result<()> {
    let s = arg.trim();
    let parts: Vec<&str> = s.split('-').collect();
    let lens = [8, 4, 4, 4, 12];
    if parts.len() != 5 || !parts.iter().zip(lens.iter()).all(|(p, l)| p.len() == *l) {
        return Err(anyhow::anyhow!(
            "[ERROR] argument validation: '{}' is not a valid UUID (expected xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)",
            name
        ));
    }
    if !s.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(anyhow::anyhow!(
            "[ERROR] argument validation: '{}' contains non-hex characters",
            name
        ));
    }
    Ok(())
}

pub async fn handle_cli_command(cli: Cli, rt: RuntimeInfo) -> Result<()> {
    let config = rt.config;
    // Handle TestScrobble - not a YTM API command
    match &cli.command {
        Some(crate::Command::TestScrobble { artist, track, album, duration }) => {
            use crate::app::scrobbler::{ScrobbleState, submit_scrobble_inner};
            use std::time::Duration;
            require_non_empty(artist, "artist")?;
            require_non_empty(track, "track")?;
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
            let res = match tokio::time::timeout(CLI_TIMEOUT, submit_scrobble_inner(&config.scrobbling, &state)).await {
                Ok(r) => r,
                Err(_) => {
                    eprintln!("[ERROR] TestScrobble: operation timed out after {}s", CLI_TIMEOUT.as_secs());
                    crate::app::scrobbler::ScrobbleResult::Failure
                }
            };
            match res {
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
                if tokio::time::timeout(CLI_TIMEOUT, crate::app::scrobbler::retry_failed_scrobbles(&config.scrobbling)).await.is_err() {
                    eprintln!("[ERROR] ScrobbleCache retry timed out after {}s", CLI_TIMEOUT.as_secs());
                }
                println!("Retry complete.");
                return Ok(());
            }
            match read_scrobble_cache_entries() {
                Some(entries) if !entries.is_empty() => {
                    let pb = make_progress_bar(entries.len() as u64, "listing entries");
                    for (i, e) in entries.iter().enumerate() {
                        let artist = e["artist"].as_str().unwrap_or("?");
                        let track = e["track"].as_str().unwrap_or("?");
                        let album = e["album"].as_str().unwrap_or("");
                        let retries = e["retry_count"].as_u64().unwrap_or(0);
                        pb.println(format!("  {}. {} - {} ({}) retries={}", i + 1, artist, track, album, retries));
                        pb.inc(1);
                    }
                    pb.finish_with_message(format!("Scrobble cache: {} entries", entries.len()));
                }
                _ => println!("Scrobble cache is empty."),
            }
            return Ok(());
        }
        Some(crate::Command::Recommendations { type_filter, limit, page, niche_level, seed_count, seed, similar_limit, json }) => {
            use crate::lastfm_recommend::{
                RecKind, fetch_niche_recommendations, fetch_recommendations,
                fetch_recommendations_for_seed, print_recommendations,
                print_recommendations_json,
            };
            if let Some(seed_query) = seed {
                require_non_empty(seed_query, "seed")?;
                let kind = match type_filter.as_str() {
                    "tracks" => RecKind::Tracks,
                    "albums" => RecKind::Albums,
                    "artists" => RecKind::Artists,
                    _ => RecKind::Artists,
                };
                info!(
                    "Fetching seed recommendations: kind={} seed='{}' limit={} similar_limit={}",
                    kind, seed_query, limit, similar_limit
                );
                let items = with_timeout_dur(
                    NICHE_TIMEOUT,
                    "recommendations",
                    fetch_recommendations_for_seed(
                        &config.scrobbling,
                        kind,
                        seed_query,
                        *limit,
                        *similar_limit,
                    ),
                )
                .await
                .with_context(|| {
                    format!("Failed to fetch seed recommendations for '{}'", seed_query)
                })?;
                info!("Fetched {} seed recommendations", items.len());
                if *json {
                    print_recommendations_json(&items);
                } else {
                    print_recommendations(&items);
                }
                return Ok(());
            }
            let valid = ["all", "tracks", "albums", "artists"];
            if !valid.contains(&type_filter.as_str()) {
                return Err(anyhow::anyhow!(
                    "--type must be one of: all, tracks, albums, artists"
                ));
            }
            if !(0.0..=1.0).contains(niche_level) {
                return Err(anyhow::anyhow!(
                    "--niche-level must be between 0.0 and 1.0"
                ));
            }
            if config.scrobbling.session_key.trim().is_empty() {
                eprintln!("[ERROR] recommendations: session_key not configured in [scrobbling] config section");
                return Err(anyhow::anyhow!("recommendations: session_key missing"));
            }
            let kinds: Vec<RecKind> = if type_filter == "all" {
                vec![RecKind::Tracks, RecKind::Albums, RecKind::Artists]
            } else {
                vec![match type_filter.as_str() {
                    "tracks" => RecKind::Tracks,
                    "albums" => RecKind::Albums,
                    "artists" => RecKind::Artists,
                    _ => unreachable!(),
                }]
            };
            let mut all = Vec::new();
            for kind in &kinds {
                if *niche_level == 0.0 {
                    info!("Fetching Last.fm recommendations: kind={} limit={} page={}", kind, limit, page);
                    let items = with_timeout(
                        "recommendations",
                        fetch_recommendations(&config.scrobbling, *kind, *limit, *page),
                    )
                    .await
                    .with_context(|| format!("Failed to fetch Last.fm {} recommendations", kind))?;
                    info!("Fetched {} Last.fm {} recommendations", items.len(), kind);
                    all.extend(items);
                } else {
                    info!(
                        "Fetching niche Last.fm recommendations: kind={} limit={} niche_level={} seeds={}",
                        kind, limit, niche_level, seed_count
                    );
                    let items = with_timeout_dur(
                        NICHE_TIMEOUT,
                        "recommendations",
                        fetch_niche_recommendations(
                            &config.scrobbling,
                            *kind,
                            *limit,
                            *seed_count,
                            *niche_level,
                            *page,
                        ),
                    )
                    .await
                    .with_context(|| format!("Failed to fetch niche Last.fm {} recommendations", kind))?;
                    info!("Fetched {} niche Last.fm {} recommendations", items.len(), kind);
                    all.extend(items);
                }
            }
            if *json {
                print_recommendations_json(&all);
            } else {
                print_recommendations(&all);
            }
            return Ok(());
        }
        Some(crate::Command::ListenbrainzRecommendations { artist_type, json }) => {
            use crate::lastfm_recommend::{
                fetch_listenbrainz_recommendations, print_listenbrainz_recommendations,
                print_listenbrainz_recommendations_json,
            };
            let valid = ["top", "similar", "raw"];
            if !valid.contains(&artist_type.as_str()) {
                return Err(anyhow::anyhow!(
                    "--artist-type must be one of: top, similar, raw"
                ));
            }
            if config.scrobbling.listenbrainz_token.trim().is_empty() {
                eprintln!("[ERROR] listenbrainz-recommendations: listenbrainz_token not configured in [scrobbling] config section");
                return Err(anyhow::anyhow!("listenbrainz-recommendations: listenbrainz_token missing"));
            }
            let username = "caos_obliquo";
            info!("Fetching ListenBrainz CF recommendations: user={} artist_type={}", username, artist_type);
            let items = with_timeout(
                "listenbrainz-recommendations",
                fetch_listenbrainz_recommendations(&config.scrobbling, username, artist_type),
            )
            .await
            .with_context(|| format!("Failed to fetch ListenBrainz CF recommendations for {}", username))?;
            info!("Fetched {} ListenBrainz CF recommendations", items.len());
            if *json {
                print_listenbrainz_recommendations_json(&items);
            } else {
                print_listenbrainz_recommendations(&items);
            }
            return Ok(());
        }
        Some(crate::Command::TestValidateMetadata {
            artist,
            title,
            album,
            rym,
        }) => {
            use crate::app::server::MetadataRegistry;
            require_non_empty(artist, "artist")?;
            require_non_empty(title, "title")?;
            let http_client = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (metadata-test)")
                .timeout(CLI_TIMEOUT)
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
            let pb = make_progress_bar(1, "resolving metadata");
            pb.set_message("querying all providers...");
            let meta = with_timeout("TestValidateMetadata", registry.resolve(artist, title, album.as_deref()))
                .await
                .context("metadata resolution failed")?;
            pb.finish_and_clear();
            println!("--- RESULT ---");
            println!("Artist:    {:?}", meta.artist);
            println!("Album:     {:?}", meta.album);
            println!("Year:      {:?}", meta.year);
            println!("Track no:  {:?}", meta.track_no);
            println!("Tracks:    {}", meta.album_tracks.len());
            println!("Genres:    {:?}", meta.genres);
            println!("Styles:    {:?}", meta.styles);
            if *rym {
                for g in &meta.genres {
                    match rym_genre_data::find_genre(g) {
                        Some(ge) => match &ge.description {
                            Some(desc) => println!("  [RYM] {} - {}", g, desc),
                            None => println!("  [RYM] {} - (no description)", g),
                        },
                        None => println!("  [RYM] {} - (not in RYM data)", g),
                    }
                }
            }
            for (i, t) in meta.album_tracks.iter().enumerate() {
                println!("  {}. {} ({:.0}s) {:?}", i + 1, t.title, t.duration_secs, t.artist);
            }
            return Ok(());
        }
        Some(crate::Command::TestListenbrainz { artist, title }) => {
            use metadata_provider::listenbrainz::ListenBrainzProvider;
            require_non_empty(artist, "artist")?;
            require_non_empty(title, "title")?;
            let token = config.scrobbling.listenbrainz_token.clone();
            if token.is_empty() {
                println!("[ERROR] TestListenbrainz: listenbrainz_token not configured in [scrobbling] section");
                return Ok(());
            }
            let provider = ListenBrainzProvider::new(token);
            let client = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (listenbrainz-test)")
                .timeout(CLI_TIMEOUT)
                .build()?;
            println!("Querying ListenBrainz: {} - {}", artist, title);
            let meta = with_timeout_opt("TestListenbrainz", provider.lookup(artist, title, None, &client)).await;
            match meta {
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
            require_non_empty(artist, "artist")?;
            require_non_empty(title, "title")?;
            let client = reqwest::Client::builder()
                .user_agent("Youtui/0.1 (musicbrainz-test)")
                .timeout(CLI_TIMEOUT)
                .build()?;
            let client_id = Some(config.scrobbling.musicbrainz_client_id.clone())
                .filter(|s| !s.is_empty());
            let client_secret = Some(config.scrobbling.musicbrainz_client_secret.clone())
                .filter(|s| !s.is_empty());
            let bearer = Some(config.scrobbling.musicbrainz_bearer_token.clone())
                .filter(|s| !s.is_empty());
            let provider = MusicBrainzProvider::new(client_id, client_secret, bearer);
            println!("Querying MusicBrainz: {} - {}", artist, title);
            let meta = with_timeout_opt("TestMusicbrainz", provider.lookup(artist, title, None, &client)).await;
            match meta {
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
                .timeout(CLI_TIMEOUT)
                .build()?;
            let mbid = if let Some(rgid) = release_group_id {
                validate_uuid(rgid, "release_group_id")?;
                rgid.clone()
            } else if let (Some(a), Some(t)) = (artist, title) {
                require_non_empty(a, "artist")?;
                require_non_empty(t, "title")?;
                println!("Resolving MBID for {} - {}...", a, t);
                use metadata_provider::musicbrainz::MusicBrainzProvider;
                let client_id = Some(config.scrobbling.musicbrainz_client_id.clone())
                    .filter(|s| !s.is_empty());
                let client_secret = Some(config.scrobbling.musicbrainz_client_secret.clone())
                    .filter(|s| !s.is_empty());
                let bearer = Some(config.scrobbling.musicbrainz_bearer_token.clone())
                    .filter(|s| !s.is_empty());
                let provider = MusicBrainzProvider::new(client_id, client_secret, bearer);
                match with_timeout_opt("TestCaa MBID resolve", provider.lookup(&a, &t, None, &http)).await {
                    Some(meta) => match meta.musicbrainz_release_group_id {
                        Some(id) => id,
                        None => { println!("[ERROR] TestCaa: no MBID found for {} - {}", a, t); return Ok(()); }
                    },
                    None => { println!("[ERROR] TestCaa: MusicBrainz lookup failed for {} - {}", a, t); return Ok(()); }
                }
            } else {
                println!("[ERROR] TestCaa: provide --release-group-id OR --artist + --title");
                return Ok(());
            };
            println!("Fetching CAA for release-group: {}", mbid);
            let url = format!("https://coverartarchive.org/release-group/{}/front", mbid);
            let resp = with_timeout("TestCaa fetch", async { http.get(&url).send().await.map_err(anyhow::Error::from) }).await?;
            if resp.status().is_success() {
                let bytes = with_timeout("TestCaa read body", async { resp.bytes().await.map_err(anyhow::Error::from) }).await?;
                let len = bytes.len();
                let path = std::env::temp_dir().join(format!("caa_{}.jpg", mbid));
                if let Err(e) = std::fs::write(&path, &bytes) {
                    println!("[ERROR] TestCaa: failed to write image: {}", e);
                } else {
                    println!("OK: {} bytes -> {}", len, path.display());
                }
            } else {
                println!("[ERROR] TestCaa: CAA returned HTTP {}", resp.status());
            }
            return Ok(());
        }
        Some(crate::Command::EnrichCache { file }) => {
            return handle_enrich_cache(&config, file).await;
        }
        Some(crate::Command::MetadataCache { show: _show, stats, clear }) => {
            let sqlite_path = crate::get_data_dir()?.join("metadata_cache.db");
            let cache = SqliteCache::open(&sqlite_path)
                .with_context(|| format!("[ERROR] MetadataCache: failed to open cache at {}", sqlite_path.display()))?;
            if *clear {
                cache.clear().with_context(|| "[ERROR] MetadataCache: failed to clear cache")?;
                println!("Metadata cache cleared.");
                return Ok(());
            }
            if *stats {
                let count = cache.len().with_context(|| "[ERROR] MetadataCache: failed to count entries")?;
                let file_size = std::fs::metadata(&sqlite_path)?.len();
                println!("Metadata cache entries: {}", count);
                println!("Database file size: {} bytes", file_size);
                return Ok(());
            }
            // Default (--show or no flags): list entries
            let entries = cache.iter().with_context(|| "[ERROR] MetadataCache: failed to iterate entries")?;
            if entries.is_empty() {
                println!("Metadata cache is empty.");
            } else {
                let pb = make_progress_bar(entries.len() as u64, "listing entries");
                for (key, meta) in &entries {
                    let year = meta.year.as_deref().unwrap_or("None");
                    let artist = meta.artist.as_deref().unwrap_or("?");
                    let album = meta.album.as_deref().unwrap_or("?");
                    pb.println(format!("  {}: year={}, artist={}, album={}, genres={}, styles={}",
                        key, year, artist, album, meta.genres.len(), meta.styles.len()));
                    if !meta.subgenres.is_empty() {
                        pb.println(format!("    subgenres: {}", meta.subgenres.join(", ")));
                    }
                    if !meta.genre_paths.is_empty() {
                        let paths: Vec<String> = meta.genre_paths
                            .iter()
                            .map(|(t, p)| format!("{} -> {}", t, p))
                            .collect();
                        pb.println(format!("    genre_paths: {}", paths.join("; ")));
                    }
                    if !meta.descriptors.is_empty() {
                        pb.println(format!("    descriptors: {}", meta.descriptors.join(", ")));
                    }
                    pb.inc(1);
                }
                pb.finish_with_message(format!("Metadata cache: {} entries", entries.len()));
            }
            return Ok(());
        }
        Some(crate::Command::GenreDb { list, lookup }) => {
            let db_path = crate::get_data_dir()?.join("genre_db.db");
            let db = genre_db_sqlite::GenreDb::open_persistent(&db_path)
                .with_context(|| format!("[ERROR] GenreDb: failed to open database at {}", db_path.display()))?;
            if let Some(name) = lookup {
                let info = db.find_genre(name);
                let subgenres = db.get_subgenres_with_descriptions(name);
                match info {
                    Some(g) => {
                        println!("{} (source: {})", g.name, g.source);
                        if let Some(ref desc) = g.description {
                            println!("  Description: {}", desc);
                        }
                        if let Some(ref parent) = g.parent_name {
                            println!("  Parent: {}", parent);
                        }
                        if !subgenres.is_empty() {
                            println!("  Subgenres ({}):", subgenres.len());
                            for (sub, desc) in &subgenres {
                                print!("    - {}", sub);
                                if let Some(d) = desc {
                                    print!(" — {}", d);
                                }
                                println!();
                            }
                        } else {
                            println!("  (no subgenres)");
                        }
                    }
                    None => {
                        println!("Genre '{}' not found.", name);
                        // Try fuzzy suggestions
                        let all = db.all_genres();
                        let lowered = name.to_lowercase();
                        let suggestions: Vec<&String> = all.iter()
                            .filter(|g| g.to_lowercase().contains(&lowered))
                            .take(5)
                            .collect();
                        if !suggestions.is_empty() {
                            println!("  Did you mean:");
                            for s in suggestions {
                                println!("    - {}", s);
                            }
                        }
                    }
                }
                return Ok(());
            }
            if *list {
                let all = db.all_genres();
                let pb = make_progress_bar(all.len() as u64, "listing genres");
                for genre in &all {
                    let subgenres = db.get_subgenres_with_descriptions(genre);
                    if subgenres.is_empty() {
                        pb.println(format!("  {} (leaf)", genre));
                    } else {
                        pb.println(format!("  {} ({} subgenres):", genre, subgenres.len()));
                        for (sub, desc) in &subgenres {
                            let mut line = format!("    - {}", sub);
                            if let Some(d) = desc {
                                line.push_str(&format!(" — {}", d));
                            }
                            pb.println(line);
                        }
                    }
                    pb.inc(1);
                }
                pb.finish_with_message(format!("Genres: {} total", all.len()));
                return Ok(());
            }
            // Default: stats
            let all = db.all_genres();
            let descriptors = db.all_descriptors();
            println!("Genre database: {} genres, {} descriptors", all.len(), descriptors.len());
            println!("Database path: {}", db_path.display());
            return Ok(());
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
            let res = with_timeout("API command", command_to_query(command, cli_query, api))
                .await
                .context("YTM API command failed")?;
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
            let res = with_timeout("API command", command_to_query(command, cli_query, api))
                .await
                .context("YTM API command failed")?;
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
        .timeout(CLI_TIMEOUT)
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
    let pb = make_progress_bar(total as u64, "enriching");

    tracing::info!("enrich-cache: starting batch enrichment for {} songs", total);
    for (artist, title) in &pairs {
        let _permit = semaphore.acquire().await;
        tracing::debug!("enrich-cache: resolving {} - {}", artist, title);
        let meta = match with_timeout_opt(
            &format!("EnrichCache resolve {} - {}", artist, title),
            registry.resolve_fast(artist, title, None),
        )
        .await
        {
            Some(m) => m,
            None => {
                errors += 1;
                completed += 1;
                pb.set_message(format!("{artist} - {title} -> ERROR"));
                pb.inc(1);
                continue;
            }
        };
        completed += 1;
        let year_str = meta.year.as_deref().unwrap_or("None");
        let genre_str = if meta.genres.is_empty() { String::new() } else { format!(" [{}]", meta.genres.join(", ")) };
        tracing::info!("enrich-cache: {} - {} -> year={:?}, genres={}, styles={}", artist, title, meta.year, meta.genres.len(), meta.styles.len());
        pb.set_message(format!("{artist} - {title} -> {year_str}{genre_str}"));
        pb.inc(1);
    }
    pb.finish_with_message(format!("Done. {completed} enriched, {errors} errors"));
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
