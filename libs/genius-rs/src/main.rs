use genius_rs::GeniusClient;
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<&str> = env::args().skip(1).map(|s| Box::leak(s.into_boxed_str()) as &str).collect();
    if args.len() < 2 {
        eprintln!("Usage: genius fetch <artist> <title>");
        eprintln!("       genius search <artist> <title>");
        eprintln!("       genius all <artist> <title>  (lyrics + annotations)");
        eprintln!("       genius slug <artist> <title>  (compute URL only)");
        std::process::exit(1);
    }

    // Parse: command [args...]
    // Need to handle "artist" and "title" which may contain spaces
    // Strategy: first arg is command, remaining are artist + title joined by spaces
    let command = args[0];
    // First remaining arg is artist, rest is title (space-joined)
    let (artist, title) = if args.len() >= 3 {
        (args[1], args[2..].join(" "))
    } else {
        ("", args[1..].join(" "))
    };

    let token = env::var("GENIUS_TOKEN").ok();
    let client = GeniusClient::with_default_client(token);

    match command {
        "slug" => {
            let path = genius_rs::search::compute_path(artist, &title);
            println!("{}", path);
            println!("https://genius.com{}", path);
        }
        "fetch" | "lyrics" => match client.find_and_fetch(artist, &title).await {
            Ok((hit, lyrics)) => {
                println!("--- {} - {} (id={}) ---", hit.artist, hit.title, hit.id);
                println!("{}", lyrics);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        "search" => match client.find_song(artist, &title).await {
            Ok(Some(hit)) => {
                println!("Found: {} - {} (id={})", hit.artist, hit.title, hit.id);
                println!("  Path: {}", hit.path);
                println!("  Year: {:?}", hit.year);
                println!("  Album: {:?}", hit.album);
            }
            Ok(None) => {
                eprintln!("No results for '{} - {}'", artist, title);
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        "all" | "full" => match client.find_fetch_all(artist, &title).await {
            Ok((hit, lyrics, annotations)) => {
                println!("=== {} - {} (id={}) ===", hit.artist, hit.title, hit.id);
                println!("\n--- Lyrics ---");
                println!("{}", lyrics);
                println!("\n--- Annotations ({} total) ---", annotations.len());
                for (i, ann) in annotations.iter().enumerate() {
                    println!("\n[{}/{}] \"{}\"", i + 1, annotations.len(), ann.fragment);
                    println!("{}", ann.body);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        _ => {
            eprintln!("Unknown command: {}. Use 'fetch', 'search', 'all', or 'slug'", command);
            std::process::exit(1);
        }
    }
}
