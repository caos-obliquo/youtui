use std::path::Path;
use ytmapi_rs::{
    auth::BrowserToken,
    common::{AlbumID, PlaylistID, YoutubeID},
    process_json,
    query::{
        GetAlbumQuery, GetPlaylistTracksQuery,
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
        eprintln!("  search <query>              Search (needs fixture file)");
        eprintln!("  playlist <id>              Fetch playlist (needs fixture)");
        eprintln!("  album <id>                 Fetch album (needs fixture)");
        eprintln!("  fixture <file>             Parse fixture JSON file");
        return;
    }

    let command = &args[1];
    match command.as_str() {
        "fixture" => cmd_fixture(&args[2..]).await,
        _ => {
            eprintln!("Use 'fixture' command to parse saved JSON files.");
            eprintln!("Example: ytmapi fixture ./test_json/search_songs_20231226.json");
        }
    }
}

async fn cmd_fixture(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: ytmapi fixture <file> [--search <query>]");
        eprintln!("       ytmapi fixture <file> --playlist");
        eprintln!("       ytmapi fixture <file> --album");
        return;
    }

    let file = &args[0];
    let path = Path::new(file);
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error reading {}: {}", file, e); return; }
    };

    // Detect query type from args
    let mut query_type = "search";
    for arg in args {
        match arg.as_str() {
            "--playlist" => query_type = "playlist",
            "--album" => query_type = "album",
            _ => {}
        }
    }

    match query_type {
        "playlist" => {
            let query = GetPlaylistTracksQuery::new(PlaylistID::from_raw(""));
            match process_json::<GetPlaylistTracksQuery<'_>, BrowserToken>(source, &query) {
                Ok(output) => println!("{:#?}", output),
                Err(e) => eprintln!("Parse error: {}", e),
            }
        }
        "album" => {
            let query = GetAlbumQuery::new(AlbumID::from_raw(""));
            match process_json::<GetAlbumQuery<'_>, BrowserToken>(source, &query) {
                Ok(output) => println!("{:#?}", output),
                Err(e) => eprintln!("Parse error: {}", e),
            }
        }
        _ => {
            let query: SearchQuery<'_, ytmapi_rs::query::search::FilteredSearch<SongsFilter>> = SearchQuery::new("").with_filter(SongsFilter);
            match process_json::<SearchQuery<'_, ytmapi_rs::query::search::FilteredSearch<SongsFilter>>, BrowserToken>(source, &query) {
                Ok(output) => println!("{:#?}", output),
                Err(e) => eprintln!("Parse error: {}", e),
            }
        }
    }
}
