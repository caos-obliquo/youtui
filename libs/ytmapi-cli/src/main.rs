use ytmapi_rs::{
    common::{PlaylistID, YoutubeID},
    query::{
        GetPlaylistTracksQuery,
        SearchQuery, search::SongsFilter,
    },
};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ytmapi <command> [args...]");
        eprintln!();
        eprintln!("Commands:");
        eprintln!("  search <query>              Search YouTube Music");
        eprintln!("  playlist <id>               Fetch playlist tracks");
        eprintln!("  album <id>                  Fetch album details");
        eprintln!("  fixture <query> [--dir .]   Search and save JSON + output");
        return;
    }

    let command = &args[1];
    let result = match command.as_str() {
        "search" => cmd_search(&args[2..]).await,
        "playlist" => cmd_playlist(&args[2..]).await,
        "album" => cmd_album(&args[2..]).await,
        "fixture" => cmd_fixture(&args[2..]).await,
        _ => {
            eprintln!("Unknown command: {}. Use one of: search, playlist, album, fixture", command);
            return;
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn cmd_search(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("Usage: ytmapi search <query>".to_string());
    }
    let query_str = args.join(" ");
    // Use SongsFilter to get structured song results
    let _query = SearchQuery::new(&query_str).with_filter(SongsFilter);
    let _source = r#"{}"#.to_string();
    // For now, print the query we'd execute (live queries require auth)
    println!("Search query: {}", query_str);
    println!("Filter: Songs");
    println!();
    println!("To run a live search, pipe through a fixture file.");
    println!("Example: ytmapi fixture \"Beatles\" --dir ./test_json/");
    Ok(())
}

async fn cmd_playlist(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("Usage: ytmapi playlist <playlist_id> [--json]".to_string());
    }
    let id = &args[0];
    let playlist_id = PlaylistID::from_raw(id);
    let _query = GetPlaylistTracksQuery::new(playlist_id);

    // For now, show info about what we'd do
    println!("Playlist ID: {}", id);
    println!("Use fixture mode to run actual query against saved JSON.");
    Ok(())
}

async fn cmd_album(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("Usage: ytmapi album <album_id>".to_string());
    }
    let id = &args[0];
    println!("Album ID: {}", id);
    println!("Use fixture mode to run actual query against saved JSON.");
    Ok(())
}

async fn cmd_fixture(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("Usage: ytmapi fixture <query> [--dir <dir>]".to_string());
    }
    let mut query_str = String::new();
    let mut dir = "./test_json".to_string();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--dir" {
            if i + 1 < args.len() {
                dir = args[i + 1].clone();
                i += 1;
            }
        } else {
            if !query_str.is_empty() { query_str.push(' '); }
            query_str.push_str(&args[i]);
        }
        i += 1;
    }
    println!("Fixture mode: search='{}', dir='{}'", query_str, dir);
    println!("This will search YTM and save JSON + parsed output.");
    println!("Requires YTM API credentials (cookie/auth).");
    Ok(())
}
