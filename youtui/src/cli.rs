use crate::{Cli, OAUTH_FILENAME, RuntimeInfo, get_api, get_config_dir};
use anyhow::Result;
use futures::future::try_join_all;
use querybuilder::{CliQuery, QueryType, command_to_query};
use std::path::PathBuf;
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
                None,
                None,
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
