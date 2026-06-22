use genius_rs::GeniusClient;
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: genius fetch <artist> <title>");
        eprintln!("       genius search <artist> <title>");
        eprintln!("       genius all <artist> <title>  (lyrics + annotations)");
        std::process::exit(1);
    }

    let token = env::var("GENIUS_TOKEN").ok();
    let client = GeniusClient::with_default_client(token);
    let command = &args[1];
    let artist = &args[2];
    let title = &args[3..].join(" ");

    match command.as_str() {
        "fetch" | "lyrics" => match client.find_and_fetch(artist, title).await {
            Ok((hit, lyrics)) => {
                println!("--- {} - {} (id={}) ---", hit.artist, hit.title, hit.id);
                println!("{}", lyrics);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        "search" => match client.find_song(artist, title).await {
            Ok(Some(hit)) => {
                println!("Found: {} - {} (id={})", hit.artist, hit.title, hit.id);
                println!("  Path: {}", hit.path);
                println!("  Year: {:?}", hit.year);
                println!("  Album: {:?}", hit.album);
            }
            Ok(None) => {
                eprintln!("No results for '{} - {}", artist, title);
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        "all" | "full" => match client.find_fetch_all(artist, title).await {
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
            eprintln!("Unknown command: {}. Use 'fetch', 'search', or 'all'", command);
            std::process::exit(1);
        }
    }
}
