mod scrape;
mod parse;

use clap::Parser;

#[derive(Parser)]
#[command(name = "rym-definitions")]
enum Cli {
    /// Test cookie connection to RYM
    Test {
        /// Dump raw HTML body for parser debugging
        #[arg(long)]
        raw: bool,
    },
    /// Scrape all genres
    Genres {
        #[arg(long)]
        json: bool,
    },
    /// Scrape all descriptors
    Descriptors {
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
        )
        .init();

    let cli = Cli::parse();

    // Read cookies from env var (full cookie header: "cf_clearance=...; sec_bs=...; ...")
    let cookies = std::env::var("RYM_COOKIES")
        .expect("Set RYM_COOKIES env var with cookie header string");

    let client = scrape::RymClient::new(cookies);

    match cli {
        Cli::Test { raw } => {
            match client.test_connection().await {
                Ok((status, body)) => {
                    println!("RYM reachable! status={}, body_bytes={}", status, body.len());
                    if raw {
                        println!("\n--- RAW HTML BODY ---\n{}", body);
                    } else {
                        // Print first 2000 chars for quick inspection
                        let preview: String = body.chars().take(2000).collect();
                        println!("\n--- HTML preview (first 2000 chars) ---\n{}", preview);
                    }
                }
                Err(e) => {
                    eprintln!("Failed: {}", e);
                }
            }
        }
        Cli::Genres { json } => {
            match client.fetch_genres().await {
                Ok(genres) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&genres).unwrap());
                    } else {
                        println!("Found {} genres:", genres.len());
                        for g in &genres {
                            let desc_short = g.description.chars().take(80).collect::<String>();
                            println!("  {} — {}", g.name, desc_short);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed: {}", e);
                }
            }
        }
        Cli::Descriptors { json } => {
            match client.fetch_descriptors().await {
                Ok(descriptors) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&descriptors).unwrap());
                    } else {
                        println!("Found {} descriptors:", descriptors.len());
                        for d in &descriptors {
                            let exp_short = d.explanation.chars().take(80).collect::<String>();
                            println!("  {} — {}", d.name, exp_short);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed: {}", e);
                }
            }
        }
    }
}
