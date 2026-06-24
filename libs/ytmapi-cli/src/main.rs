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
        "debug" => cmd_debug(rest).await,
        "genius" => cmd_genius(rest).await,
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
    eprintln!("  debug meta <title> [artist]    Test title cleaning + artist normalization");
    eprintln!("  debug clean <title>            Test title cleaning only");
    eprintln!("  debug artist <name>            Test artist normalization only");
    eprintln!("  debug resolve <artist> <title>  Test full resolution pipeline");
    eprintln!("  debug genre <genre>            Test genre normalization");
    eprintln!("  debug genre-list [filter]      List known genres");
    eprintln!();
    eprintln!("GENIUS (no auth, uses GENIUS_TOKEN env):");
    eprintln!("  genius search <artist> <title>        Search Genius for song");
    eprintln!("  genius annotations <artist> <title>   Fetch annotations");
    eprintln!("  genius lyrics <artist> <title>        Fetch lyrics");
    eprintln!("  genius all <artist> <title>            Fetch lyrics + annotations");
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

fn normalize_artist_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() { return String::new(); }
    let mut chars = trimmed.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    first + chars.as_str()
}

fn clean_title(title: &str, artist: &str) -> String {
    // Strip "{artist} - " prefix
    let artist_prefix = format!("{} - ", artist);
    let s = if title.to_lowercase().starts_with(&artist_prefix.to_lowercase()) {
        title[artist_prefix.len().min(title.len())..].to_string()
    } else {
        title.to_string()
    };
    // Strip noise tags
    let noise_tags: &[&str] = &[
        "official audio", "official video", "lyric video", "lyrics",
        "legendado", "c legendado", "c legenda", "com legenda",
        "com legendado", "legendado pt", "legendado pt-br",
        "subtitle", "subtitles",
    ];
    let mut s = s;
    loop {
        let lower = s.to_lowercase().trim().to_string();
        let mut found = false;
        for tag in noise_tags {
            if let Some(pos) = lower.rfind(tag) {
                // Find the '(' before the tag, if any
                let before = &s[..pos];
                let cut = if let Some(paren) = before.rfind('(') {
                    // Check if tag is inside parenthesized suffix
                    if s[paren..].to_lowercase().contains(tag) {
                        // Strip the entire parenthesized suffix including paren
                        s[..paren].trim().to_string()
                    } else {
                        // Strip from tag position
                        s[..pos].trim().to_string()
                    }
                } else {
                    s[..pos].trim().to_string()
                };
                if cut.len() < s.len() {
                    s = cut.trim().to_string();
                    found = true;
                    break;
                }
            }
        }
        if !found {
            s = s.trim_end_matches(|c| c == '(').trim().to_string();
            break;
        }
    }
    // Strip album suffix tags
    let lower = s.to_lowercase();
    let album_tags = ["full album", "full ep", "full lp", "full demo", "full single", "album", "demo", "ep", "single", "singles"];
    let pos = album_tags.iter().filter_map(|t| lower.rfind(t)).max();
    if let Some(pos) = pos {
        let start = s[..pos].rfind('(').unwrap_or(pos);
        s = if start > 0 { s[..start].trim().to_string() } else { s[..pos].trim().to_string() };
    } else if let Some(pos) = s.find("  (") {
        s = s[..pos].trim().to_string();
    }
    s.trim().to_string()
}

async fn cmd_genius(args: &[String]) {
    if args.len() < 3 {
        eprintln!("Usage: ytmapi genius <search|annotations|lyrics|all> <artist> <title>");
        eprintln!("  GENIUS_TOKEN env var required for annotations");
        return;
    }
    let sub = &args[0];
    let artist = &args[1];
    let title = &args[2..].join(" ");
    let token = std::env::var("GENIUS_TOKEN").ok().filter(|s| !s.is_empty());

    let client = reqwest::Client::builder()
        .user_agent("ytmapi-cli/0.1 (genius debug)")
        .build()
        .unwrap();
    let genius = genius_rs::GeniusClient::new(token, client);

    match sub.as_str() {
        "search" => {
            println!("=== Genius Search ===");
            println!("Artist: '{}'  Title: '{}'", artist, title);
            match genius.find_song(artist, &title).await {
                Ok(Some(hit)) => {
                    println!("Found: id={}, path={}", hit.id, hit.path);
                    println!("  Title: '{}'", hit.title);
                    println!("  Artist: '{}'", hit.artist);
                    println!("  Year: {:?}", hit.year);
                    println!("  Album: {:?}", hit.album);
                    println!("  Thumbnail: {:?}", hit.thumbnail);
                }
                Ok(None) => println!("No results found"),
                Err(e) => println!("Search error: {}", e),
            }
        }
        "annotations" => {
            println!("=== Genius Annotations ===");
            println!("Artist: '{}'  Title: '{}'", artist, title);
            match genius.find_song(artist, &title).await {
                Ok(Some(hit)) => {
                    println!("Song found: id={}, path={}", hit.id, hit.path);
                    println!("Fetching annotations...");
                    match genius.fetch_annotations_with_token(&hit.path, hit.id).await {
                        Ok(anns) => {
                            println!("Fetched {} annotations:", anns.len());
                            for (i, a) in anns.iter().enumerate() {
                                println!("  {}. [{}] {}", i + 1, a.fragment, a.body.chars().take(200).collect::<String>());
                            }
                        }
                        Err(e) => println!("Annotations error: {}", e),
                    }
                }
                Ok(None) => println!("No Genius results for this song"),
                Err(e) => println!("Search error: {}", e),
            }
        }
        "lyrics" => {
            println!("=== Genius Lyrics ===");
            println!("Artist: '{}'  Title: '{}'", artist, title);
            match genius.find_and_fetch(artist, &title).await {
                Ok((hit, lyrics)) => {
                    println!("Song: id={}, path={}", hit.id, hit.path);
                    println!("Title: '{}'  Artist: '{}'", hit.title, hit.artist);
                    println!("\n--- Lyrics ---\n{}", lyrics);
                }
                Err(e) => println!("Lyrics error: {}", e),
            }
        }
        "all" => {
            println!("=== Genius All (Lyrics + Annotations) ===");
            println!("Artist: '{}'  Title: '{}'", artist, title);
            match genius.find_fetch_all(artist, &title).await {
                Ok((hit, lyrics, annotations)) => {
                    println!("Song: id={}, path={}", hit.id, hit.path);
                    println!("Title: '{}'  Artist: '{}'", hit.title, hit.artist);
                    println!("\n--- Lyrics ---\n{}", lyrics);
                    println!("\n--- Annotations ({}) ---", annotations.len());
                    for (i, a) in annotations.iter().enumerate() {
                        println!("  {}. [{}] {}", i + 1, a.fragment, a.body.chars().take(200).collect::<String>());
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        _ => eprintln!("Unknown genius sub: {}. Use search, annotations, lyrics, all", sub.as_str()),
    }
}

async fn cmd_debug(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: ytmapi debug <subcommand>");
        eprintln!("  meta <title> [artist]    Test title cleaning + artist normalization");
        eprintln!("  clean <title>            Test title cleaning only");
        eprintln!("  artist <name>            Test artist normalization only");
        eprintln!("  resolve <artist> <title>  Test metadata pipeline for a song");
        eprintln!("  simulate-url <url>        Simulate add_yt_video full path (yt-dlp + clean + resolve)");
        eprintln!("  genre <genre>             Test genre normalization");
        eprintln!("  genre-list [filter]       List known genres (optionally filtered)");
        return;
    }
    match args[0].as_str() {
        "meta" => cmd_debug_meta(&args[1..]).await,
        "clean" => {
            let title = args.get(1).map(|s| s.as_str()).unwrap_or("");
            if title.is_empty() { eprintln!("Usage: ytmapi debug clean <title>"); return; }
            let cleaned = clean_title(title, "Unknown");
            println!("Original: '{}'", title);
            println!("Cleaned:  '{}'", cleaned);
        }
        "artist" => {
            let name = args.get(1).map(|s| s.as_str()).unwrap_or("");
            if name.is_empty() { eprintln!("Usage: ytmapi debug artist <name>"); return; }
            let normalized = normalize_artist_name(name);
            println!("Original:  '{}'", name);
            println!("Normalized: '{}'", normalized);
        }
        "resolve" => cmd_debug_resolve(&args[1..]).await,
        "genre" => {
            let name = args.get(1).map(|s| s.as_str()).unwrap_or("");
            if name.is_empty() { eprintln!("Usage: ytmapi debug genre <genre>"); return; }
            let normalized = metadata_provider::genre_map::normalize_genre(name);
            let known = metadata_provider::genre_map::is_known_genre(name);
            println!("Input:  '{}'", name);
            println!("Normalized: '{}'", normalized);
            println!("Known: {}", known);
        }
        "simulate-url" => cmd_debug_simulate_url(&args[1..]).await,
        "genre-list" => {
            let filter = args.get(1).map(|s| s.to_lowercase());
            let all = metadata_provider::genre_map::all_genres();
            for g in &all {
                if let Some(ref f) = filter {
                    if !g.to_lowercase().contains(f) { continue; }
                }
                println!("{}", g);
            }
            println!("Total: {}", all.len());
        }
        _ => eprintln!("Unknown debug subcommand: {}. Use: meta, clean, artist, resolve, genre, genre-list", args[0]),
    }
}

async fn cmd_debug_resolve(args: &[String]) {
    if args.len() < 2 {
        eprintln!("Usage: ytmapi debug resolve <artist> <title>");
        return;
    }
    let artist = &args[0];
    let title = &args[1];
    
    println!("=== Metadata Pipeline Test ===");
    println!("Artist: '{}'", artist);
    println!("Title:  '{}'", title);
    println!();
    
    // Test title cleaning
    let cleaned = clean_title(title, artist);
    println!("Cleaned title: '{}'", cleaned);
    println!();
    
    // Test artist normalization
    let normalized = normalize_artist_name(artist);
    println!("Normalized artist: '{}'", normalized);
    println!();
    
    // Test the metadata registry directly
    use metadata_provider::MetadataRegistry;
    
    let http_client = reqwest::Client::builder()
        .user_agent("Youtui/0.1 (test)")
        .build()
        .unwrap();
    
    // Get API keys from env
    let lastfm_key = std::env::var("LASTFM_API_KEY").ok().filter(|s| !s.is_empty());
    let discogs_token = std::env::var("DISCOGS_TOKEN").ok().filter(|s| !s.is_empty());
    let genius_token = std::env::var("GENIUS_TOKEN").ok().filter(|s| !s.is_empty());
    
    let registry = MetadataRegistry::new(
        http_client,
        lastfm_key,
        discogs_token,
        genius_token,
        None,
    );
    
    println!("Resolving metadata...");
    match registry.resolve(normalized.as_str(), cleaned.as_str()).await {
        Ok(meta) => {
            println!();
            println!("=== Result ===");
            println!("Artist: {:?}", meta.artist);
            println!("Album:  {:?}", meta.album);
            println!("Year:   {:?}", meta.year);
            println!("Track#: {:?}", meta.track_no);
            println!("Tracks: {} entries", meta.album_tracks.len());
            if !meta.album_tracks.is_empty() {
                println!("First 3 tracks:");
                for (i, t) in meta.album_tracks.iter().take(3).enumerate() {
                    println!("  {}. {} ({:.0}s)", i + 1, t.title, t.duration_secs);
                }
            }
            println!("Genres: {:?}", meta.genres);
            println!("Styles: {:?}", meta.styles);
        }
        Err(e) => eprintln!("Resolution error: {}", e),
    }
}

async fn cmd_debug_simulate_url(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: ytmapi debug simulate-url <youtube-url>");
        eprintln!("  Simulates the full add_yt_video flow: yt-dlp -> title cleaning -> registry.resolve()");
        eprintln!("  Shows all tracks with quality guard check.");
        return;
    }
    let url = &args[0];
    println!("=== Simulating add_yt_video ===");
    println!("URL: {}", url);
    println!();

    // Step 1: Extract video ID (same as play_yt_url)
    let raw_id = if url.contains("watch?v=") {
        url.split("watch?v=").nth(1).unwrap_or(url)
            .split('&').next().unwrap_or("")
            .to_string()
    } else if url.contains("youtu.be/") {
        url.split("youtu.be/").nth(1).unwrap_or(url)
            .split('?').next().unwrap_or("")
            .to_string()
    } else {
        url.rsplit('/').next().unwrap_or(url)
            .split('?').next().unwrap_or(&url)
            .to_string()
    };
    println!("Extracted video ID: {}", raw_id);
    println!();

    // Step 2: Run yt-dlp (same as add_yt_video)
    println!("Running yt-dlp --dump-json --no-warnings --flat-playlist...");
    let output = std::process::Command::new("yt-dlp")
        .args(["--dump-json", "--no-warnings", "--flat-playlist"])
        .arg(&format!("https://youtu.be/{}", raw_id))
        .output();
    
    let (title, artist, duration_secs) = match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // yt-dlp outputs NDJSON; parse first entry (same as serde_json::from_str)
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stdout) {
                let t = v.get("title").and_then(|s| s.as_str()).unwrap_or(&raw_id).to_string();
                let uploader = v.get("uploader").and_then(|s| s.as_str()).unwrap_or("Unknown").to_string();
                let a = if t.contains(" - ") {
                    t.splitn(2, " - ").next().map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.len() < 80)
                        .unwrap_or_else(|| uploader.clone())
                } else {
                    uploader.clone()
                };
                let d = v.get("duration").and_then(|s| s.as_f64()).unwrap_or(0.0);
                println!("  Title:        '{}'", t);
                println!("  Uploader:     '{}'", uploader);
                println!("  Artist:       '{}'", a);
                println!("  Duration:     {:.0}s", d);
                (t, a, d)
            } else {
                eprintln!("Failed to parse yt-dlp output");
                return;
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("yt-dlp failed: {}", stderr);
            return;
        }
        Err(e) => {
            eprintln!("Failed to run yt-dlp: {}", e);
            return;
        }
    };
    println!();

    // Step 3: Title cleaning (match add_yt_video exactly)
    // 3a: Strip artist prefix
    let clean_title = {
        let lower = title.to_lowercase();
        let art_lower = artist.to_lowercase();
        if lower.starts_with(&format!("{} - ", art_lower)) {
            title[artist.len() + 3..].trim().to_string()
        } else if lower.starts_with(&art_lower) && !lower[art_lower.len()..].starts_with(&art_lower) {
            title[artist.len()..].trim().to_string()
        } else {
            title.to_string()
        }
    };
    // 3b: Strip noise tags
    let clean_title = {
        let noise_tags = [
            "official audio", "official video", "lyric video", "lyrics",
            "legendado", "c legendado", "c legenda", "com legenda",
            "com legendado", "legendado pt", "legendado pt-br",
            "subtitle", "subtitles",
        ];
        let mut s = clean_title;
        loop {
            let lower = s.to_lowercase().trim().to_string();
            let mut found = false;
            for tag in &noise_tags {
                if let Some(pos) = lower.rfind(tag) {
                    let before = &s[..pos].trim();
                    let cut = if let Some(paren_start) = before.rfind('(') {
                        let between = &before[paren_start..];
                        if between.to_lowercase().contains(tag) {
                            &before[..paren_start.max(1).saturating_sub(1)]
                        } else {
                            &s[..pos]
                        }
                    } else {
                        &s[..pos]
                    };
                    if cut.len() < s.len() {
                        s = cut.trim().to_string();
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                s = s.trim_end_matches(|c| c == '(').trim().to_string();
                break;
            }
        }
        s.trim().to_string()
    };
    // 3c: Strip album suffix tags
    let clean_title = {
        let s = clean_title.as_str();
        let lower = s.to_lowercase();
        let tags = ["full album", "full ep", "full lp", "full demo", "full single", "album", "demo", "ep", "single", "singles"];
        let pos = tags.iter().filter_map(|t| lower.find(t)).min();
        if let Some(pos) = pos {
            let start = s[..pos].rfind('(').unwrap_or(pos);
            if start > 0 { s[..start].trim().to_string() } else { s[..pos].trim().to_string() }
        } else if let Some(pos) = s.find("  (") {
            s[..pos].trim().to_string()
        } else {
            s.trim().to_string()
        }
    };
    // 3d: Strip year from parenthetical
    let clean_title_for_search = {
        let lower = clean_title.to_lowercase();
        if let Some(paren) = lower.rfind("(") {
            let inner = lower[paren..].trim_matches(|c| c == '(' || c == ')' || c == ' ');
            if inner.split(|c: char| !c.is_ascii_digit())
                .find(|p| p.len() == 4)
                .and_then(|p| {
                    let y = p.parse::<u16>().ok()?;
                    if (1900..2100).contains(&y) { Some(y) } else { None }
                })
                .is_some()
            {
                clean_title[..paren].trim().to_string()
            } else {
                clean_title.clone()
            }
        } else {
            clean_title.clone()
        }
    };

    println!("=== Title Cleaning ===");
    println!("Raw title:    '{}'", title);
    println!("Artist:       '{}'", artist);
    println!("Clean title:  '{}'", clean_title_for_search);
    println!("Song title (for UI): '{}'", clean_title);
    println!();

    // Step 4: Resolve metadata via registry
    let normalized = normalize_artist_name(&artist);
    use metadata_provider::MetadataRegistry;
    let http_client = reqwest::Client::builder()
        .user_agent("Youtui/0.1 (test)")
        .build()
        .unwrap();
    let lastfm_key = std::env::var("LASTFM_API_KEY").ok().filter(|s| !s.is_empty());
    let discogs_token = std::env::var("DISCOGS_TOKEN").ok().filter(|s| !s.is_empty());
    let genius_token = std::env::var("GENIUS_TOKEN").ok().filter(|s| !s.is_empty());
    let registry = MetadataRegistry::new(
        http_client,
        lastfm_key,
        discogs_token,
        genius_token,
        None,
    );

    println!("=== Resolving Metadata ===");
    println!("Searching for: artist='{}' title='{}'", normalized, clean_title_for_search);
    println!();

    match registry.resolve(&normalized, &clean_title_for_search).await {
        Ok(meta) => {
            println!("=== Result ===");
            println!("Artist: {:?}", meta.artist);
            println!("Album:  {:?}", meta.album);
            println!("Year:   {:?}", meta.year);
            println!("Track#: {:?}", meta.track_no);
            println!("Tracks: {} entries", meta.album_tracks.len());
            if !meta.album_tracks.is_empty() {
                let mut total_dur = 0.0f64;
                for (i, t) in meta.album_tracks.iter().enumerate() {
                    println!("  {:2}. {} ({:.0}s)", i + 1, t.title, t.duration_secs);
                    total_dur += t.duration_secs;
                }
                println!();
                println!("=== Quality Guard Check ===");
                let valid_tracks = meta.album_tracks.iter().all(|t| t.duration_secs > 0.0);
                let duration_ok = duration_secs == 0.0 || total_dur >= duration_secs * 0.5;
                println!("  Total track duration: {:.0}s", total_dur);
                println!("  Video duration:       {:.0}s", duration_secs);
                println!("  All tracks have duration > 0: {}", valid_tracks);
                println!("  Total >= 50% of video: {} ({:.0}s >= {:.0}s)", duration_ok, total_dur, duration_secs * 0.5);
                if valid_tracks && duration_ok {
                    println!("  => ALBUM SPLITTING WOULD TRIGGER");
                } else {
                    println!("  => ALBUM SPLITTING WOULD BE REJECTED");
                    if !valid_tracks { println!("     Reason: zero-duration tracks"); }
                    if !duration_ok { println!("     Reason: total duration {:.0}s < 50% of video {:.0}s", total_dur, duration_secs); }
                }
            }
            println!("Genres: {:?}", meta.genres);
            println!("Styles: {:?}", meta.styles);
        }
        Err(e) => eprintln!("Resolution error: {}", e),
    }
}

async fn cmd_debug_meta(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: ytmapi debug meta <title> [artist]");
        eprintln!("  Tests title cleaning and artist name normalization.");
        eprintln!("  Provide a title and optional artist.");
        return;
    }

    let title = &args[0];
    let artist = if args.len() > 1 { &args[1] } else { "Unknown" };
    
    println!("=== Metadata Pipeline Debug ===");
    println!("Input title:  '{}'", title);
    println!("Input artist: '{}'", artist);
    println!();
    
    let cleaned = clean_title(title, artist);
    println!("Cleaned title: '{}'", cleaned);
    println!();
    
    let normalized = normalize_artist_name(artist);
    println!("Normalized artist: '{}'", normalized);
    println!();
    
    if cleaned != *title {
        println!("✅ Title was modified");
    } else {
        println!("⏺ Title unchanged");
    }
    if normalized != *artist {
        println!("✅ Artist was modified");
    } else {
        println!("⏺ Artist unchanged");
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
