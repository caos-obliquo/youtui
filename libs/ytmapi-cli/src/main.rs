use ytmapi_rs::{
    YtMusic,
    common::{AlbumID, ArtistChannelID, LikeStatus, PlaylistID, SetVideoID, VideoID, YoutubeID},
    process_json,
    query::{
        EditPlaylistQuery, GetAlbumQuery, GetPlaylistTracksQuery,
        SearchQuery, search::{SongsFilter},
        playlist::PrivacyStatus,
    },
};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    // Parse global options
    let mut cookie_file: Option<String> = None;
    let mut json_output = false;
    let mut cmd_args: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cookie" => {
                i += 1;
                cookie_file = Some(args.get(i).cloned().unwrap_or_default());
            }
            "--json" => json_output = true,
            _ => cmd_args.push(args[i].clone()),
        }
        i += 1;
    }

    if cmd_args.is_empty() {
        print_usage();
        return;
    }

    // Fall back to env var for cookie
    if cookie_file.is_none() {
        cookie_file = std::env::var("YTMAPI_COOKIE").ok();
    }

    let command = &cmd_args[0];
    let rest = &cmd_args[1..];

    match command.as_str() {
        "fixture" => cmd_fixture(rest, json_output).await,
        _ => cmd_live(command, rest, cookie_file.as_deref(), json_output).await,
    }
}

fn print_usage() {
    eprintln!("ytmapi - YouTube Music API CLI");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  ytmapi [options] <command> [args...]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --cookie <file>     Cookie file for auth (or YTMAPI_COOKIE env)");
    eprintln!("  --json              Machine-readable JSON output");
    eprintln!();
    eprintln!("COMMANDS (live, requires auth):");
    eprintln!("  search <query>                Search songs");
    eprintln!("  search-artists <query>        Search artists");
    eprintln!("  search-albums <query>         Search albums");
    eprintln!("  search-playlists <query>      Search playlists");
    eprintln!("  playlist <id>                 Get playlist tracks");
    eprintln!("  playlist-songs <id>           Get playlist tracks (streaming debug)");
    eprintln!("  album <id>                    Get album details");
    eprintln!("  artist <id>                   Get artist");
    eprintln!("  watch-playlist <video_id>     Get related/watch playlist for a video");
    eprintln!("  library playlists             List library playlists");
    eprintln!("  library songs                 List library songs");
    eprintln!("  delete-playlist <id>          Delete a playlist");
    eprintln!("  edit-playlist <id> [opts]     Edit playlist (--title/--description/--privacy)");
    eprintln!("  rate-playlist <id> <rating>   Rate playlist (like/indifferent/dislike)");
    eprintln!("  remove-items <id> <vid...>    Remove items from playlist");
    eprintln!("  add-to-playlist <id> <vid...> Add items to playlist");
    eprintln!();
    eprintln!("COMMANDS (offline, no auth):");
    eprintln!("  fixture <file> [--type search|playlist|album]");
    eprintln!();
    eprintln!("AUTH:");
    eprintln!("  Export cookies from https://music.youtube.com to a file:");
    eprintln!("    1. Install 'Get cookies.txt' browser extension");
    eprintln!("    2. Export cookies for music.youtube.com");
    eprintln!("    3. Run: ytmapi search \"Beatles\" --cookie ~/Downloads/cookies.txt");
}

async fn cmd_live(command: &str, args: &[String], cookie: Option<&str>, json: bool) {
    let cookie_path = match cookie {
        Some(c) => c.to_string(),
        None => {
            eprintln!("Error: --cookie <file> required for live queries");
            eprintln!("Set YTMAPI_COOKIE env var or pass --cookie flag");
            return;
        }
    };

    let yt = match YtMusic::from_cookie_file(&cookie_path).await {
        Ok(yt) => yt,
        Err(e) => {
            eprintln!("Error loading cookie file '{}': {}", cookie_path, e);
            eprintln!("Make sure the file contains valid YouTube Music cookies.");
            return;
        }
    };

    match command {
        "search" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search <query>"); return; }
            match yt.search_songs(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-artists" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-artists <query>"); return; }
            match yt.search_artists(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-albums" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-albums <query>"); return; }
            match yt.search_albums(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-playlists" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-playlists <query>"); return; }
            match yt.search_playlists(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "playlist" => {
            if args.is_empty() { eprintln!("Usage: ytmapi playlist <id>"); return; }
            let id = PlaylistID::from_raw(&args[0]);
            match yt.get_playlist_tracks(id).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Playlist error: {}", e),
            }
        }
        "playlist-songs" => {
            if args.is_empty() { eprintln!("Usage: ytmapi playlist-songs <id>"); return; }
            let id = PlaylistID::from_raw(&args[0]);
            eprintln!("Fetching tracks (streaming)...");
            match yt.get_playlist_tracks(id).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Playlist error: {}", e),
            }
        }
        "album" => {
            if args.is_empty() { eprintln!("Usage: ytmapi album <id>"); return; }
            let id = AlbumID::from_raw(&args[0]);
            match yt.get_album(id).await {
                Ok(result) => print_results(&result, json),
                Err(e) => eprintln!("Album error: {}", e),
            }
        }
        "artist" => {
            if args.is_empty() { eprintln!("Usage: ytmapi artist <channel_id>"); return; }
            let id = ArtistChannelID::from_raw(&args[0]);
            match yt.get_artist(id).await {
                Ok(result) => print_results(&result, json),
                Err(e) => eprintln!("Artist error: {}", e),
            }
        }
        "watch-playlist" => {
            if args.is_empty() { eprintln!("Usage: ytmapi watch-playlist <video_id>"); return; }
            let id = VideoID::from_raw(&args[0]);
            match yt.get_watch_playlist_from_video_id(id).await {
                Ok(result) => print_results(&result, json),
                Err(e) => eprintln!("Watch playlist error: {}", e),
            }
        }
        "library" => {
            if args.is_empty() { eprintln!("Usage: ytmapi library <playlists|songs>"); return; }
            match args[0].as_str() {
                "playlists" => match yt.get_library_playlists().await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "songs" => match yt.get_library_songs().await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                _ => eprintln!("Unknown library subcommand: {}. Use 'playlists' or 'songs'", args[0]),
            }
        }
        "delete-playlist" => {
            if args.is_empty() { eprintln!("Usage: ytmapi delete-playlist <playlist_id>"); return; }
            let id = PlaylistID::from_raw(&args[0]);
            match yt.delete_playlist(id).await {
                Ok(_) => println!("Playlist deleted successfully"),
                Err(e) => eprintln!("Delete error: {}", e),
            }
        }
        "rate-playlist" => {
            if args.len() < 2 { eprintln!("Usage: ytmapi rate-playlist <playlist_id> <like|indifferent|dislike>"); return; }
            let id = PlaylistID::from_raw(&args[0]);
            let rating = match args[1].as_str() {
                "like" => LikeStatus::Liked,
                "indifferent" => LikeStatus::Indifferent,
                "dislike" => LikeStatus::Disliked,
                _ => { eprintln!("Invalid rating. Use: like, indifferent, or dislike"); return; }
            };
            match yt.rate_playlist(id, rating).await {
                Ok(_) => println!("Playlist rated successfully"),
                Err(e) => eprintln!("Rate error: {}", e),
            }
        }
        "edit-playlist" => {
            if args.is_empty() { eprintln!("Usage: ytmapi edit-playlist <playlist_id> [--title <t>] [--description <d>] [--privacy <private|public|unlisted>]"); return; }
            let id = PlaylistID::from_raw(&args[0]);
            let mut title: Option<String> = None;
            let mut desc: Option<String> = None;
            let mut privacy: Option<PrivacyStatus> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--title" => { i += 1; title = Some(args.get(i).cloned().unwrap_or_default()); }
                    "--description" => { i += 1; desc = Some(args.get(i).cloned().unwrap_or_default()); }
                    "--privacy" => {
                        i += 1;
                        privacy = Some(match args.get(i).map(|s| s.as_str()) {
                            Some("private") => PrivacyStatus::Private,
                            Some("public") => PrivacyStatus::Public,
                            Some("unlisted") => PrivacyStatus::Unlisted,
                            _ => { eprintln!("Invalid privacy. Use: private, public, or unlisted"); return; }
                        });
                    }
                    _ => { eprintln!("Unknown flag: {}", args[i]); return; }
                }
                i += 1;
            }
            let mut query = if let Some(t) = title {
                EditPlaylistQuery::new_title(&id, t)
            } else {
                EditPlaylistQuery::new_title(&id, "")
            };
            if let Some(d) = desc { query = query.with_new_description(d); }
            if let Some(p) = privacy { query = query.with_new_privacy_status(p); }
            match yt.edit_playlist(query).await {
                Ok(_) => println!("Playlist edited successfully"),
                Err(e) => eprintln!("Edit error: {}", e),
            }
        }
        "remove-items" => {
            if args.len() < 2 { eprintln!("Usage: ytmapi remove-items <playlist_id> <video_id>..."); return; }
            let id = PlaylistID::from_raw(&args[0]);
            let set_ids: Vec<SetVideoID<'_>> = args[1..].iter().map(|v| SetVideoID::from_raw(v.clone())).collect();
            match yt.remove_playlist_items(id, set_ids).await {
                Ok(_) => println!("Items removed successfully"),
                Err(e) => eprintln!("Remove error: {}", e),
            }
        }
        "add-to-playlist" => {
            if args.len() < 2 { eprintln!("Usage: ytmapi add-to-playlist <playlist_id> <video_id>..."); return; }
            let id = PlaylistID::from_raw(&args[0]);
            let video_ids: Vec<VideoID<'_>> = args[1..].iter().map(|v| VideoID::from_raw(v.clone())).collect();
            match yt.add_video_items_to_playlist(id, video_ids).await {
                Ok(results) => println!("Added {} items", results.len()),
                Err(e) => eprintln!("Add error: {}", e),
            }
        }
        _ => {
            eprintln!("Unknown command: {}. See ytmapi --help", command);
        }
    }
}

async fn cmd_fixture(args: &[String], json: bool) {
    if args.is_empty() {
        eprintln!("Usage: ytmapi fixture <file> [--type search|playlist|album]");
        return;
    }
    let file = &args[0];
    let mut fixture_type = "search";
    for arg in args {
        if arg == "--type" {
            let idx = args.iter().position(|a| a == "--type").unwrap();
            if idx + 1 < args.len() {
                fixture_type = &args[idx + 1];
            }
        }
    }

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error reading {}: {}", file, e); return; }
    };

    let output: String = match fixture_type {
        "playlist" => {
            let query = GetPlaylistTracksQuery::new(PlaylistID::from_raw(""));
            match process_json::<GetPlaylistTracksQuery<'_>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        "album" => {
            let query = GetAlbumQuery::new(AlbumID::from_raw(""));
            match process_json::<GetAlbumQuery<'_>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        _ => {
            let query: SearchQuery<'_, ytmapi_rs::query::search::FilteredSearch<SongsFilter>> = SearchQuery::new("").with_filter(SongsFilter);
            match process_json::<SearchQuery<'_, ytmapi_rs::query::search::FilteredSearch<SongsFilter>>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
    };

    if json {
        // Output is already {:#?} formatted, wrap it
        println!("{{\"parsed\": {}}}", serde_json::to_string(&output).unwrap_or_else(|_| format!("\"{}\"", output)));
    } else {
        println!("{}", output);
    }
}

fn print_results<T: std::fmt::Debug>(results: &T, json: bool) {
    if json {
        // Output as JSON-esque debug format
        println!("{}", serde_json::to_string(&format!("{:#?}", results)).unwrap());
    } else {
        println!("{:#?}", results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_usage_does_not_panic() {
        // Just ensure print_usage doesn't crash
        print_usage();
    }

    #[test]
    fn test_cmd_fixture_search_parse() {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("ytmapi-rs/test_json/search_songs_20231226.json");
        if !fixture_path.exists() {
            eprintln!("Fixture not found at {:?}, skipping", fixture_path);
            return;
        }
        let args = vec![
            fixture_path.to_string_lossy().to_string(),
            "--type".to_string(),
            "search".to_string(),
        ];
        // Should not panic — just parse and print
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cmd_fixture(&args, false));
    }

    #[test]
    fn test_cmd_fixture_search_parse_json() {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("ytmapi-rs/test_json/search_songs_20231226.json");
        if !fixture_path.exists() {
            eprintln!("Fixture not found at {:?}, skipping", fixture_path);
            return;
        }
        let args = vec![
            fixture_path.to_string_lossy().to_string(),
            "--type".to_string(),
            "search".to_string(),
        ];
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cmd_fixture(&args, true));
    }

    #[test]
    fn test_cmd_fixture_playlist_parse() {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("ytmapi-rs/test_json/get_playlist_20250604.json");
        if !fixture_path.exists() {
            eprintln!("Fixture not found at {:?}, skipping", fixture_path);
            return;
        }
        let args = vec![
            fixture_path.to_string_lossy().to_string(),
            "--type".to_string(),
            "playlist".to_string(),
        ];
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cmd_fixture(&args, false));
    }

    #[test]
    fn test_cmd_fixture_album_parse() {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("ytmapi-rs/test_json/get_album_20240724.json");
        if !fixture_path.exists() {
            eprintln!("Fixture not found at {:?}, skipping", fixture_path);
            return;
        }
        let args = vec![
            fixture_path.to_string_lossy().to_string(),
            "--type".to_string(),
            "album".to_string(),
        ];
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cmd_fixture(&args, false));
    }

    #[test]
    fn test_cmd_fixture_missing_file() {
        let args = vec!["nonexistent.json".to_string()];
        let rt = tokio::runtime::Runtime::new().unwrap();
        // Should print error, not panic
        rt.block_on(cmd_fixture(&args, false));
    }

    #[test]
    fn test_cmd_fixture_no_args() {
        let args: Vec<String> = vec![];
        let rt = tokio::runtime::Runtime::new().unwrap();
        // Should print usage, not panic
        rt.block_on(cmd_fixture(&args, false));
    }
}
