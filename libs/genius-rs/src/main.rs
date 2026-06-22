use genius_rs::{GeniusClient, search};
use std::env;

#[tokio::main]
async fn main() {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.is_empty() {
        eprintln!("Usage: genius <command> [options] <artist> <title>");
        eprintln!();
        eprintln!("Commands:");	
        eprintln!("  fetch | lyrics <artist> <title>   Fetch lyrics");
        eprintln!("  search <artist> <title>            Search Genius for song");
        eprintln!("  all | full <artist> <title>        Lyrics + annotations");
        eprintln!("  slug <artist> <title>              Compute song URL only");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --json           JSON output (machine-readable)");
        eprintln!("  --fixture <dir>  Save raw HTML + parsed output to dir");
        return;
    }

    // Parse flags and arguments
    let mut json = false;
    let mut fixture_dir: Option<String> = None;
    let mut parsed_args: Vec<String> = Vec::new();

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--json" => json = true,
            "--fixture" => {
                i += 1;
                fixture_dir = Some(raw.get(i).cloned().unwrap_or_default());
            }
            _ => parsed_args.push(raw[i].clone()),
        }
        i += 1;
    }

    if parsed_args.len() < 2 {
        eprintln!("Error: need at least <command> <artist>");
        return;
    }

    let command = &parsed_args[0];
    let artist = &parsed_args[1];
    let title = &parsed_args[2..].join(" ");

    let token = env::var("GENIUS_TOKEN").ok();
    let client = GeniusClient::with_default_client(token);

    let result = match command.as_str() {
        "slug" => {
            let path = search::compute_path(artist, title);
            if json {
                println!("{{\"path\":\"{}\",\"url\":\"https://genius.com{}\"}}", path, path);
            } else {
                println!("{}", path);
                println!("https://genius.com{}", path);
            }
            Ok(())
        }
        "fetch" | "lyrics" => {
            match client.find_and_fetch(artist, title).await {
                Ok((hit, lyrics)) => {
                    if json {
                        let ann = serde_json::json!({
                            "artist": hit.artist,
                            "title": hit.title,
                            "id": hit.id,
                            "path": hit.path,
                            "lyrics": lyrics,
                        });
                        println!("{}", serde_json::to_string_pretty(&ann).unwrap());
                    } else {
                        println!("--- {} - {} (id={}) ---", hit.artist, hit.title, hit.id);
                        println!("{}", lyrics);
                    }
                    if let Some(dir) = &fixture_dir {
                        let slug = hit.path.trim_start_matches('/').replace('/', "-");
                        let path = std::path::Path::new(dir);
                        std::fs::create_dir_all(path).ok();
                        std::fs::write(path.join(format!("{}.txt", slug)), &lyrics).ok();
                        eprintln!("Saved fixture to {}/{}.txt", dir, slug);
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        "search" => {
            match client.find_song(artist, title).await {
                Ok(Some(hit)) => {
                    if json {
                        let j = serde_json::json!({
                            "id": hit.id,
                            "path": hit.path,
                            "title": hit.title,
                            "artist": hit.artist,
                            "year": hit.year,
                            "album": hit.album,
                        });
                        println!("{}", serde_json::to_string_pretty(&j).unwrap());
                    } else {
                        println!("Found: {} - {} (id={})", hit.artist, hit.title, hit.id);
                        println!("  Path: {}", hit.path);
                        println!("  Year: {:?}", hit.year);
                        println!("  Album: {:?}", hit.album);
                    }
                    Ok(())
                }
                Ok(None) => Err(format!("No results for '{} - {}'", artist, title)),
                Err(e) => Err(e),
            }
        }
        "all" | "full" => {
            match client.find_fetch_all(artist, title).await {
                Ok((hit, lyrics, annotations)) => {
                    if json {
                        let ann_list: Vec<serde_json::Value> = annotations.iter().map(|a| {
                            serde_json::json!({"fragment": a.fragment, "body": a.body})
                        }).collect();
                        let j = serde_json::json!({
                            "artist": hit.artist,
                            "title": hit.title,
                            "id": hit.id,
                            "path": hit.path,
                            "lyrics": lyrics,
                            "annotations": ann_list,
                        });
                        println!("{}", serde_json::to_string_pretty(&j).unwrap());
                    } else {
                        println!("=== {} - {} (id={}) ===", hit.artist, hit.title, hit.id);
                        println!("\n--- Lyrics ---");
                        println!("{}", lyrics);
                        println!("\n--- Annotations ({} total) ---", annotations.len());
                        for (i, ann) in annotations.iter().enumerate() {
                            println!("\n[{}/{}] \"{}\"", i + 1, annotations.len(), ann.fragment);
                            println!("{}", ann.body);
                        }
                    }
                    if let Some(dir) = &fixture_dir {
                        let slug = hit.path.trim_start_matches('/').replace('/', "-");
                        let path = std::path::Path::new(dir);
                        std::fs::create_dir_all(path).ok();
                        std::fs::write(path.join(format!("{}-lyrics.txt", slug)), &lyrics).ok();
                        let ann_text: String = annotations.iter().map(|a| {
                            format!("--- {} ---\n{}\n", a.fragment, a.body)
                        }).collect::<Vec<_>>().join("\n");
                        std::fs::write(path.join(format!("{}-annotations.txt", slug)), &ann_text).ok();
                        eprintln!("Saved fixtures to {}/", dir);
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        _ => Err(format!("Unknown command: {}. Use 'fetch', 'search', 'all', or 'slug'", command)),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
