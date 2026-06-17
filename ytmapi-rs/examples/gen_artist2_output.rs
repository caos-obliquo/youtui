use ytmapi_rs::process_json;
use ytmapi_rs::auth::noauth::NoAuthToken;
use ytmapi_rs::common::{ArtistChannelID, YoutubeID};
use ytmapi_rs::query::GetArtistQuery;

#[tokio::main]
async fn main() {
    let source = tokio::fs::read_to_string(
        "/home/caos/builds/youtui/ytmapi-rs/test_json/get_artist_20250310.json"
    ).await.unwrap();
    let output = process_json::<GetArtistQuery<'_>, NoAuthToken>(
        source,
        GetArtistQuery::new(ArtistChannelID::from_raw(""))
    ).unwrap();
    let formatted = format!("{:#?}", output);
    tokio::fs::write(
        "/home/caos/builds/youtui/ytmapi-rs/test_json/get_artist_20250310_output.txt",
        formatted.as_bytes()
    ).await.unwrap();
    println!("Done");
}
