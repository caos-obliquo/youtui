use lrclib_rs::LrcLibClient;
use std::env;

#[tokio::main]
async fn main() {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.is_empty() {
        eprintln!("Usage: lrclib <command> [options] <artist> <title> [album]");
        eprintln!();
        eprintln!("Commands:");
        eprintln!("  fetch | lyrics <artist> <title> [album]   Fetch lyrics");
        eprintln!("  search <query>                                Search LRCLIB");
        eprintln!("  raw <artist> <title> [album]                  Show raw API response");
        eprintln!();
        eprintln!("Flags:");
        eprintln!("  --json        JSON output");
        eprintln!("  --synced      Show synced lyrics instead of plain");
        eprintln!("  --all         Show all search results with scores");
        return;
    }

    let mut json = false;
    let mut _synced = false;
    let mut _show_all = false;
    let mut parsed: Vec<String> = Vec::new();

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--json" => json = true,
            "--synced" => _synced = true,
            "--all" => _show_all = true,
            _ => parsed.push(raw[i].clone()),
        }
        i += 1;
    }

    if parsed.is_empty() {
        eprintln!("Error: need a command");
        std::process::exit(1);
    }

    let command = &parsed[0];
    let rest: Vec<&str> = parsed.iter().skip(1).map(|s| s.as_str()).collect();

    let client = LrcLibClient::new(
        reqwest::Client::builder()
            .user_agent("lrclib-rs/0.1.0")
            .build()
            .expect("Failed to build reqwest client"),
    );

    let result = match command.as_str() {
        "fetch" | "lyrics" => {
            if rest.len() < 2 {
                eprintln!("Error: need <artist> <title>");
                std::process::exit(1);
            }
            let artist = rest[0];
            let title = rest[1];
            let album = rest.get(2).copied();

            match client.fetch_lyrics(artist, title, album).await {
                Ok(lyrics) => {
                    if json {
                        let out = serde_json::json!({
                            "artist": artist,
                            "title": title,
                            "album": album,
                            "lyrics": lyrics,
                        });
                        println!("{}", serde_json::to_string_pretty(&out).unwrap());
                    } else {
                        println!("--- {} - {} ---", artist, title);
                        println!("{}", lyrics);
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        "search" => {
            let query = rest.join(" ");
            if query.is_empty() {
                eprintln!("Error: need search query");
                std::process::exit(1);
            }
            let search_url = format!(
                "https://lrclib.net/api/search?q={}",
                urlenc(&query),
            );
            match client.client().get(&search_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<Vec<lrclib_rs::LrcLibResponse>>().await {
                        Ok(results) => {
                            if json {
                                println!("{}", serde_json::to_string_pretty(&results).unwrap());
                            } else {
                                if results.is_empty() {
                                    println!("No results");
                                } else {
                                    for (i, r) in results.iter().enumerate() {
                                        let album = r.album_name.as_deref().unwrap_or("?");
                                        let dur = format_duration(r.duration);
                                        let instrumental = if r.instrumental { " [INSTR]" } else { "" };
                                        let has_plain = if r.plain_lyrics.is_some() { " P" } else { "" };
                                        let has_synced = if r.synced_lyrics.is_some() { " S" } else { "" };
                                        println!("{}. {} - {} [{}]{} [{}{}]{}", 
                                            i + 1, r.artist_name, r.track_name, album, dur, has_plain, has_synced, instrumental);
                                    }
                                }
                            }
                            Ok(())
                        }
                        Err(e) => Err(format!("JSON parse error: {}", e)),
                    }
                }
                Ok(resp) => Err(format!("API returned {}", resp.status())),
                Err(e) => Err(format!("Request failed: {}", e)),
            }
        }
        "raw" => {
            if rest.len() < 2 {
                eprintln!("Error: need <artist> <title>");
                std::process::exit(1);
            }
            let artist = rest[0];
            let title = rest[1];
            let album = rest.get(2).copied();

            let mut url = format!(
                "https://lrclib.net/api/get?artist_name={}&track_name={}",
                urlenc(artist),
                urlenc(title),
            );
            if let Some(a) = album {
                url.push_str("&album_name=");
                url.push_str(&urlenc(a));
            }

            match client.client().get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    match resp.text().await {
                        Ok(body) => {
                            if json {
                                // Try to pretty-print
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                                    println!("{}", serde_json::to_string_pretty(&v).unwrap());
                                } else {
                                    println!("{}", body);
                                }
                            }
                            if !status.is_success() {
                                eprintln!("Status: {}", status);
                            }
                            Ok(())
                        }
                        Err(e) => Err(format!("Body read error: {}", e)),
                    }
                }
                Err(e) => Err(format!("Request failed: {}", e)),
            }
        }
        _ => Err(format!("Unknown command: {}. Use 'fetch', 'search', or 'raw'", command)),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn format_duration(secs: f64) -> String {
    let m = (secs / 60.0).floor() as u64;
    let s = (secs % 60.0).round() as u64;
    format!("{}:{:02}", m, s)
}

fn urlenc(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '&' => result.push_str("%26"),
            '?' => result.push_str("%3F"),
            '=' => result.push_str("%3D"),
            '/' => result.push_str("%2F"),
            '#' => result.push_str("%23"),
            '"' => result.push_str("%22"),
            '\'' => result.push_str("%27"),
            '+' => result.push_str("%2B"),
            ',' => result.push_str("%2C"),
            ';' => result.push_str("%3B"),
            _ => result.push(c),
        }
    }
    result
}
