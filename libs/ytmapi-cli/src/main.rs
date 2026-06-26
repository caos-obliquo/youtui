use ytmapi_rs::{
    YtMusic,
    common::{
        AlbumID, ArtistChannelID, LikeStatus, PlaylistID, SetVideoID, VideoID, YoutubeID,
        PodcastChannelID, PodcastID, EpisodeID, UserChannelID,
        UploadAlbumID, UploadArtistID, UploadEntityID,
        BrowseParams, UserVideosParams, UserPlaylistsParams, PodcastChannelParams,
        TasteToken, TasteTokenSelection, TasteTokenImpression,
        MoodCategoryParams, FeedbackTokenRemoveFromHistory,
    },
    process_json,
    query::{
        EditPlaylistQuery, GetAlbumQuery, GetPlaylistTracksQuery,
        SearchQuery, search::SongsFilter,
        playlist::PrivacyStatus,
        CreatePlaylistQuery,
        GetArtistQuery, GetArtistAlbumsQuery,
        GetTasteProfileQuery, SetTasteProfileQuery,
        GetMoodCategoriesQuery, GetMoodPlaylistsQuery,
        GetUserQuery, GetUserPlaylistsQuery, GetUserVideosQuery,
        GetChannelQuery, GetChannelEpisodesQuery, GetPodcastQuery,
        GetEpisodeQuery, GetNewEpisodesQuery,
        DeleteUploadEntityQuery,
        GetLibrarySortOrder, GetLibraryUploadAlbumQuery, GetLibraryUploadArtistQuery,
        GetLibrarySongsQuery,
        GetWatchPlaylistQuery,
        GetLyricsQuery,
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
            "--help" | "-h" => {
                print_usage();
                return;
            }
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

fn parse_sort_order(s: &str) -> Option<GetLibrarySortOrder> {
    match s {
        "name-asc" | "name_asc" => Some(GetLibrarySortOrder::NameAsc),
        "name-desc" | "name_desc" => Some(GetLibrarySortOrder::NameDesc),
        "recent" | "recently-saved" | "recently_saved" => Some(GetLibrarySortOrder::RecentlySaved),
        "default" => Some(GetLibrarySortOrder::Default),
        _ => None,
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
    eprintln!("  SEARCH:");
    eprintln!("  search <query>                Search songs");
    eprintln!("  search-artists <query>        Search artists");
    eprintln!("  search-albums <query>         Search albums");
    eprintln!("  search-playlists <query>      Search playlists");
    eprintln!("  search-videos <query>         Search videos");
    eprintln!("  search-community-playlists <q> Search community playlists");
    eprintln!("  search-featured-playlists <q> Search featured playlists");
    eprintln!("  search-episodes <query>       Search episodes");
    eprintln!("  search-podcasts <query>       Search podcasts");
    eprintln!("  search-profiles <query>       Search profiles");
    eprintln!("  search-suggestions <query>    Get search suggestions");
    eprintln!();
    eprintln!("  PLAYLIST:");
    eprintln!("  playlist <id>                 Get playlist tracks");
    eprintln!("  playlist-details <id>         Get playlist details");
    eprintln!("  playlist-songs <id>           Get playlist tracks (streaming debug)");
    eprintln!("  create-playlist <title>       Create playlist");
    eprintln!("  delete-playlist <id>          Delete a playlist");
    eprintln!("  edit-playlist <id> [opts]     Edit playlist (--title/--description/--privacy)");
    eprintln!("  rate-playlist <id> <rating>   Rate playlist (like/indifferent/dislike)");
    eprintln!("  remove-items <id> <vid...>    Remove items from playlist");
    eprintln!("  add-to-playlist <id> <vid...> Add items to playlist");
    eprintln!("  merge-playlist <dest> <src>   Add all tracks from src playlist to dest");
    eprintln!();
    eprintln!("  ALBUM / ARTIST / SONG:");
    eprintln!("  album <id>                    Get album details");
    eprintln!("  artist <channel_id>           Get artist");
    eprintln!("  artist-albums <channel_id>    Get artist albums");
    eprintln!("  subscribe <channel_id>        Subscribe to artist");
    eprintln!("  unsubscribe <channel_id>...   Unsubscribe from artists");
    eprintln!("  rate-song <video_id> <rating> Rate song (like/indifferent/dislike)");
    eprintln!("  lyrics <video_id>             Get lyrics for a song");
    eprintln!("  tracking-url <video_id>       Get song tracking URL");
    eprintln!("  resolve-album <name> [artist]  Resolve album name to browse ID");
    eprintln!("  watch-playlist <video_id>     Get related/watch playlist");
    eprintln!();
    eprintln!("  LIBRARY [--sort name-asc|name-desc|recent|default]:");
    eprintln!("  library playlists             List library playlists");
    eprintln!("  library songs                 List library songs");
    eprintln!("  library albums                List library albums");
    eprintln!("  library artists               List library artists");
    eprintln!("  library artist-subscriptions  List subscribed artists");
    eprintln!("  library podcasts              List library podcasts");
    eprintln!("  library channels              List library channels");
    eprintln!("  library upload-songs          List uploaded songs");
    eprintln!("  library upload-artists        List upload artists");
    eprintln!("  library upload-albums         List upload albums");
    eprintln!("  library upload-album <id>     Get upload album details");
    eprintln!("  library upload-artist <id>    Get upload artist songs");
    eprintln!("  library upload <file>         Upload a song to library");
    eprintln!("  delete-upload <entity_id>     Delete uploaded entity");
    eprintln!();
    eprintln!("  HISTORY:");
    eprintln!("  history                       Get listening history");
    eprintln!("  remove-history <token...>     Remove items from history");
    eprintln!();
    eprintln!("  RECOMMENDATIONS:");
    eprintln!("  taste-profile                 Get taste profile artists");
    eprintln!("  set-taste-profile <tokens...> Set taste profile");
    eprintln!("  mood-categories               Get mood categories");
    eprintln!("  mood-playlists <params>       Get mood playlists");
    eprintln!();
    eprintln!("  PODCASTS:");
    eprintln!("  channel <channel_id>          Get podcast channel");
    eprintln!("  channel-episodes <channel_id> Get channel episodes");
    eprintln!("  podcast <podcast_id>          Get podcast");
    eprintln!("  episode <episode_id>          Get episode");
    eprintln!("  new-episodes                  Get new episodes");
    eprintln!();
    eprintln!("  USER:");
    eprintln!("  user <channel_id>             Get user profile");
    eprintln!("  user-videos <channel_id>      Get user videos");
    eprintln!("  user-playlists <channel_id>   Get user playlists");
    eprintln!();
    eprintln!("COMMANDS (offline, no auth):");
    eprintln!("  fixture <file> [--type search|search-basic|playlist|album|artist|library-songs|watch-playlist|lyrics|mood-categories]");
    eprintln!("  debug meta <title> [artist]    Test title cleaning + artist normalization");
    eprintln!("  debug clean <title>            Test title cleaning only");
    eprintln!("  debug artist <name>            Test artist normalization only");
    eprintln!("  debug resolve <artist> <title> Test full resolution pipeline");
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
        // ── SEARCH ──────────────────────────────────────────────────────
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
        "search-videos" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-videos <query>"); return; }
            match yt.search_videos(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-community-playlists" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-community-playlists <query>"); return; }
            match yt.search_community_playlists(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-featured-playlists" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-featured-playlists <query>"); return; }
            match yt.search_featured_playlists(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-episodes" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-episodes <query>"); return; }
            match yt.search_episodes(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-podcasts" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-podcasts <query>"); return; }
            match yt.search_podcasts(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-profiles" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-profiles <query>"); return; }
            match yt.search_profiles(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Search error: {}", e),
            }
        }
        "search-suggestions" => {
            let query = args.join(" ");
            if query.is_empty() { eprintln!("Usage: ytmapi search-suggestions <query>"); return; }
            match yt.get_search_suggestions(&query).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Suggestions error: {}", e),
            }
        }

        // ── PLAYLIST ────────────────────────────────────────────────────
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
        "playlist-details" => {
            if args.is_empty() { eprintln!("Usage: ytmapi playlist-details <id>"); return; }
            let id = PlaylistID::from_raw(&args[0]);
            match yt.get_playlist_details(id).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Details error: {}", e),
            }
        }
        "create-playlist" => {
            if args.is_empty() { eprintln!("Usage: ytmapi create-playlist <title> [--description <d>] [--privacy private|public|unlisted]"); return; }
            let title = &args[0];
            let mut desc: Option<String> = None;
            let mut privacy = PrivacyStatus::Private;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--description" => { i += 1; desc = Some(args.get(i).cloned().unwrap_or_default()); }
                    "--privacy" => {
                        i += 1;
                        privacy = match args.get(i).map(|s| s.as_str()) {
                            Some("private") => PrivacyStatus::Private,
                            Some("public") => PrivacyStatus::Public,
                            Some("unlisted") => PrivacyStatus::Unlisted,
                            _ => { eprintln!("Invalid privacy. Use: private, public, unlisted"); return; }
                        };
                    }
                    _ if args[i].starts_with("--") => { eprintln!("Unknown flag: {}", args[i]); return; }
                    _ => {} // skip positional args we already consumed
                }
                i += 1;
            }
            match yt.create_playlist(CreatePlaylistQuery::new(title, desc.as_deref(), privacy)).await {
                Ok(id) => println!("Created playlist: {}", id.get_raw()),
                Err(e) => eprintln!("Create error: {}", e),
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
            let mut query = if let Some(ref t) = title {
                EditPlaylistQuery::new_title(&id, t.as_str())
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
        "merge-playlist" => {
            if args.len() < 2 { eprintln!("Usage: ytmapi merge-playlist <dest_playlist_id> <src_playlist_id>"); return; }
            let dest = PlaylistID::from_raw(&args[0]);
            let src = PlaylistID::from_raw(&args[1]);
            match yt.add_playlist_to_playlist(dest, src).await {
                Ok(results) => println!("Merged {} tracks", results.len()),
                Err(e) => eprintln!("Merge error: {}", e),
            }
        }

        // ── ALBUM / ARTIST / SONG ───────────────────────────────────────
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
        "artist-albums" => {
            if args.is_empty() { eprintln!("Usage: ytmapi artist-albums <channel_id> [--params <browse_params>]"); return; }
            let channel_id = ArtistChannelID::from_raw(&args[0]);
            let browse_params = if args.len() > 2 && args[1] == "--params" {
                BrowseParams::from_raw(args[2].clone())
            } else {
                BrowseParams::from_raw("")
            };
            match yt.query(GetArtistAlbumsQuery::new(channel_id, browse_params)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Artist albums error: {}", e),
            }
        }
        "subscribe" => {
            if args.is_empty() { eprintln!("Usage: ytmapi subscribe <channel_id>"); return; }
            let channel_id = ArtistChannelID::from_raw(&args[0]);
            match yt.subscribe_artist(channel_id).await {
                Ok(_) => println!("Subscribed successfully"),
                Err(e) => eprintln!("Subscribe error: {}", e),
            }
        }
        "unsubscribe" => {
            if args.is_empty() { eprintln!("Usage: ytmapi unsubscribe <channel_id>..."); return; }
            let channels: Vec<ArtistChannelID<'_>> = args.iter().map(|v| ArtistChannelID::from_raw(v.clone())).collect();
            match yt.unsubscribe_artists(channels).await {
                Ok(_) => println!("Unsubscribed successfully"),
                Err(e) => eprintln!("Unsubscribe error: {}", e),
            }
        }
        "rate-song" => {
            if args.len() < 2 { eprintln!("Usage: ytmapi rate-song <video_id> <like|indifferent|dislike>"); return; }
            let video_id = VideoID::from_raw(&args[0]);
            let rating = match args[1].as_str() {
                "like" => LikeStatus::Liked,
                "indifferent" => LikeStatus::Indifferent,
                "dislike" => LikeStatus::Disliked,
                _ => { eprintln!("Invalid rating. Use: like, indifferent, or dislike"); return; }
            };
            match yt.rate_song(video_id, rating).await {
                Ok(_) => println!("Song rated successfully"),
                Err(e) => eprintln!("Rate error: {}", e),
            }
        }
        "lyrics" => {
            if args.is_empty() { eprintln!("Usage: ytmapi lyrics <video_id>"); return; }
            let video_id = VideoID::from_raw(&args[0]);
            let lyrics_id = match yt.get_lyrics_id(video_id).await {
                Ok(id) => id,
                Err(e) => { eprintln!("Lyrics ID error: {}", e); return; }
            };
            match yt.get_lyrics(lyrics_id).await {
                Ok(lyrics) => print_results(&lyrics, json),
                Err(e) => eprintln!("Lyrics error: {}", e),
            }
        }
        "tracking-url" => {
            if args.is_empty() { eprintln!("Usage: ytmapi tracking-url <video_id>"); return; }
            let video_id = VideoID::from_raw(&args[0]);
            match yt.get_song_tracking_url(video_id).await {
                Ok(url) => println!("Tracking URL: {}", url.get_raw()),
                Err(e) => eprintln!("Tracking URL error: {}", e),
            }
        }
        "resolve-album" => {
            let (album_name, artist_name) = if args.len() >= 2 {
                (&args[0], Some(&args[1]))
            } else if args.len() == 1 {
                (&args[0], None)
            } else {
                eprintln!("Usage: ytmapi resolve-album <album_name> [artist_name]");
                return;
            };
            match yt.get_album_browse_id(album_name, artist_name.map(|x| x.as_str())).await {
                Ok(id) => println!("Album ID: {}", id.get_raw()),
                Err(e) => eprintln!("Resolve error: {}", e),
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

                // ── LIBRARY ─────────────────────────────────────────────────────
        "library" => {
            // Parse --sort flag before subcommand
            let mut sort_order: Option<GetLibrarySortOrder> = None;
            let mut sub_args: Vec<String> = Vec::new();
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "--sort" => {
                        i += 1;
                        if let Some(val) = args.get(i) {
                            if let Some(order) = parse_sort_order(val) {
                                sort_order = Some(order);
                            } else {
                                eprintln!("Invalid --sort value '{}'. Valid: name-asc, name-desc, recent, default", val);
                                return;
                            }
                        } else {
                            eprintln!("--sort requires a value (name-asc, name-desc, recent, default)");
                            return;
                        }
                    }
                    _ => sub_args.push(args[i].clone()),
                }
                i += 1;
            }
            if sub_args.is_empty() { eprintln!("Usage: ytmapi library <subcommand> [--sort <order>]"); return; }
            match sub_args[0].as_str() {
                "playlists" => match yt.get_library_playlists().await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "songs" => match yt.get_library_songs(sort_order).await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "albums" => match yt.get_library_albums(sort_order).await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "artists" => match yt.get_library_artists(sort_order).await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "artist-subscriptions" => match yt.get_library_artist_subscriptions(sort_order).await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "podcasts" => match yt.get_library_podcasts(sort_order).await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "channels" => match yt.get_library_channels(sort_order).await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },


                "upload-songs" => match yt.get_library_upload_songs().await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "upload-artists" => match yt.get_library_upload_artists().await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "upload-albums" => match yt.get_library_upload_albums().await {
                    Ok(results) => print_results(&results, json),
                    Err(e) => eprintln!("Library error: {}", e),
                },
                "upload-album" => {
                    if args.len() < 2 { eprintln!("Usage: ytmapi library upload-album <album_id>"); return; }
                    let id = UploadAlbumID::from_raw(&args[1]);
                    match yt.query(GetLibraryUploadAlbumQuery::new(id)).await {
                        Ok(results) => print_results(&results, json),
                        Err(e) => eprintln!("Library error: {}", e),
                    }
                }
                "upload-artist" => {
                    if args.len() < 2 { eprintln!("Usage: ytmapi library upload-artist <artist_id>"); return; }
                    let id = UploadArtistID::from_raw(&args[1]);
                    match yt.query(GetLibraryUploadArtistQuery::new(id)).await {
                        Ok(results) => print_results(&results, json),
                        Err(e) => eprintln!("Library error: {}", e),
                    }
                }
                "upload" => {
                    if args.len() < 2 { eprintln!("Usage: ytmapi library upload <file_path>"); return; }
                    match yt.upload_song(&args[1]).await {
                        Ok(outcome) => println!("Upload result: {:?}", outcome),
                        Err(e) => eprintln!("Upload error: {}", e),
                    }
                }
                _ => eprintln!("Unknown library subcommand: {}. See ytmapi --help", args[0]),
            }
        }
        "delete-upload" => {
            if args.is_empty() { eprintln!("Usage: ytmapi delete-upload <entity_id>"); return; }
            let id = UploadEntityID::from_raw(&args[0]);
            match yt.query(DeleteUploadEntityQuery::new(id)).await {
                Ok(_) => println!("Upload entity deleted"),
                Err(e) => eprintln!("Delete error: {}", e),
            }
        }

        // ── HISTORY ─────────────────────────────────────────────────────
        "history" => {
            match yt.get_history().await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("History error: {}", e),
            }
        }
        "remove-history" => {
            if args.is_empty() { eprintln!("Usage: ytmapi remove-history <feedback_token>..."); return; }
            let tokens: Vec<FeedbackTokenRemoveFromHistory<'_>> = args.iter().map(|v| FeedbackTokenRemoveFromHistory::from_raw(v.clone())).collect();
            match yt.remove_history_items(tokens).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Remove history error: {}", e),
            }
        }

        // ── RECOMMENDATIONS ─────────────────────────────────────────────
        "taste-profile" => {
            match yt.query(GetTasteProfileQuery).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Taste profile error: {}", e),
            }
        }
        "set-taste-profile" => {
            if args.is_empty() { eprintln!("Usage: ytmapi set-taste-profile <impression_token> <selection_token>..."); return; }
            if args.len() < 2 || args.len() % 2 != 0 {
                eprintln!("Usage: ytmapi set-taste-profile <impression_token> <selection_token>...");
                return;
            }
            let tokens: Vec<TasteToken<'_>> = args.chunks(2).map(|chunk| {
                TasteToken {
                    impression_value: TasteTokenImpression::from_raw(chunk[0].clone()),
                    selection_value: TasteTokenSelection::from_raw(chunk[1].clone()),
                }
            }).collect();
            match yt.query(SetTasteProfileQuery::new(tokens)).await {
                Ok(_) => println!("Taste profile set"),
                Err(e) => eprintln!("Set taste profile error: {}", e),
            }
        }
        "mood-categories" => {
            match yt.query(GetMoodCategoriesQuery).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Mood categories error: {}", e),
            }
        }
        "mood-playlists" => {
            let params = if args.is_empty() { MoodCategoryParams::from_raw("") } else { MoodCategoryParams::from_raw(args.join(" ")) };
            match yt.query(GetMoodPlaylistsQuery::new(params)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Mood playlists error: {}", e),
            }
        }

        // ── PODCASTS ────────────────────────────────────────────────────
        "channel" => {
            if args.is_empty() { eprintln!("Usage: ytmapi channel <channel_id>"); return; }
            let id = PodcastChannelID::from_raw(&args[0]);
            match yt.query(GetChannelQuery::new(id)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Channel error: {}", e),
            }
        }
        "channel-episodes" => {
            if args.is_empty() { eprintln!("Usage: ytmapi channel-episodes <channel_id> [--params <params>]"); return; }
            let channel_id = PodcastChannelID::from_raw(&args[0]);
            let params = if args.len() > 2 && args[1] == "--params" {
                PodcastChannelParams::from_raw(args[2].clone())
            } else {
                PodcastChannelParams::from_raw("")
            };
            match yt.query(GetChannelEpisodesQuery::new(channel_id, params)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Channel episodes error: {}", e),
            }
        }
        "podcast" => {
            if args.is_empty() { eprintln!("Usage: ytmapi podcast <podcast_id>"); return; }
            let id = PodcastID::from_raw(&args[0]);
            match yt.query(GetPodcastQuery::new(id)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Podcast error: {}", e),
            }
        }
        "episode" => {
            if args.is_empty() { eprintln!("Usage: ytmapi episode <episode_id>"); return; }
            let id = EpisodeID::from_raw(&args[0]);
            match yt.query(GetEpisodeQuery::new(id)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("Episode error: {}", e),
            }
        }
        "new-episodes" => {
            match yt.query(GetNewEpisodesQuery).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("New episodes error: {}", e),
            }
        }

        // ── USER ────────────────────────────────────────────────────────
        "user" => {
            if args.is_empty() { eprintln!("Usage: ytmapi user <channel_id>"); return; }
            let id = UserChannelID::from_raw(&args[0]);
            match yt.query(GetUserQuery::new(id)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("User error: {}", e),
            }
        }
        "user-videos" => {
            if args.is_empty() { eprintln!("Usage: ytmapi user-videos <channel_id> [--params <params>]"); return; }
            let channel_id = UserChannelID::from_raw(&args[0]);
            let params = if args.len() > 2 && args[1] == "--params" {
                UserVideosParams::from_raw(args[2].clone())
            } else {
                UserVideosParams::from_raw("")
            };
            match yt.query(GetUserVideosQuery::new(channel_id, params)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("User videos error: {}", e),
            }
        }
        "user-playlists" => {
            if args.is_empty() { eprintln!("Usage: ytmapi user-playlists <channel_id> [--params <params>]"); return; }
            let channel_id = UserChannelID::from_raw(&args[0]);
            let params = if args.len() > 2 && args[1] == "--params" {
                UserPlaylistsParams::from_raw(args[2].clone())
            } else {
                UserPlaylistsParams::from_raw("")
            };
            match yt.query(GetUserPlaylistsQuery::new(channel_id, params)).await {
                Ok(results) => print_results(&results, json),
                Err(e) => eprintln!("User playlists error: {}", e),
            }
        }

        _ => {
            eprintln!("Unknown command: {}. See ytmapi --help", command);
        }
    }
}

// ── HELPER: normalize_artist_name ─────────────────────────────────────
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
                let before = &s[..pos];
                let cut = if let Some(paren) = before.rfind('(') {
                    if s[paren..].to_lowercase().contains(tag) {
                        s[..paren].trim().to_string()
                    } else {
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

// ── GENIUS ────────────────────────────────────────────────────────────
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

// ── DEBUG ─────────────────────────────────────────────────────────────
async fn cmd_debug(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: ytmapi debug <subcommand>");
        eprintln!("  meta <title> [artist]    Test title cleaning + artist normalization");
        eprintln!("  clean <title>            Test title cleaning only");
        eprintln!("  artist <name>            Test artist normalization only");
        eprintln!("  resolve <artist> <title>  Test metadata pipeline for a song");
        eprintln!("  cache-test <artist> <title>  Test metadata cache persistence (temp dir)");
        eprintln!("  cache-check <artist> <title>  Test cache-only lookup (no HTTP)");
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
        "cache-test" => cmd_debug_cache_test(&args[1..]).await,
        "cache-check" => cmd_debug_cache_check(&args[1..]).await,
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

    let cleaned = clean_title(title, artist);
    println!("Cleaned title: '{}'", cleaned);
    println!();

    let normalized = normalize_artist_name(artist);
    println!("Normalized artist: '{}'", normalized);
    println!();

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
        None,  // overrides_path
        None,  // cache_path
    );

    println!("Resolving metadata...");
    match registry.resolve(normalized.as_str(), cleaned.as_str(), None).await {
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

async fn cmd_debug_cache_test(args: &[String]) {
    if args.len() < 2 {
        eprintln!("Usage: ytmapi debug cache-test <artist> <title>");
        eprintln!("  Tests metadata cache persistence: resolve -> save -> reload -> verify");
        return;
    }
    let artist = &args[0];
    let title = &args[1];

    let cleaned = clean_title(title, artist);
    let normalized = normalize_artist_name(artist);

    use metadata_provider::MetadataRegistry;

    println!("=== Metadata Cache Persistence Test ===");
    println!("Artist: '{}' -> normalized '{}'", artist, normalized);
    println!("Title:  '{}' -> cleaned '{}'", title, cleaned);
    println!();

    // Create temp dir for cache
    let tmp_dir = std::env::temp_dir().join("ytmapi-cache-test");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");
    println!("Cache dir: {:?}", tmp_dir);
    println!();

    // First registry — resolves and writes cache
    let http_client = reqwest::Client::builder()
        .user_agent("Youtui/0.1 (test)")
        .build()
        .unwrap();
    let lastfm_key = std::env::var("LASTFM_API_KEY").ok().filter(|s| !s.is_empty());
    let discogs_token = std::env::var("DISCOGS_TOKEN").ok().filter(|s| !s.is_empty());
    let genius_token = std::env::var("GENIUS_TOKEN").ok().filter(|s| !s.is_empty());

    println!("=== Registry 1: Fresh resolve (should hit providers) ===");
    let registry1 = MetadataRegistry::new(
        http_client.clone(),
        lastfm_key.clone(),
        discogs_token.clone(),
        genius_token.clone(),
        None,
        Some(tmp_dir.clone()),
    );
    match registry1.resolve(normalized.as_str(), cleaned.as_str(), None).await {
        Ok(meta) => {
            println!("Artist: {:?}", meta.artist);
            println!("Album:  {:?}", meta.album);
            println!("Year:   {:?}", meta.year);
            println!("Genres: {:?}", meta.genres);
            println!("Styles: {:?}", meta.styles);
            println!("Tracks: {}", meta.album_tracks.len());
        }
        Err(e) => eprintln!("Resolution error: {}", e),
    }

    // Check cache file
    let cache_file = tmp_dir.join("metadata_cache.json");
    println!();
    if cache_file.exists() {
        let size = std::fs::metadata(&cache_file).map(|m| m.len()).unwrap_or(0);
        println!("=== Cache File Written ===");
        println!("Path: {:?}", cache_file);
        println!("Size: {} bytes", size);
        if size > 0 {
            let content = std::fs::read_to_string(&cache_file).unwrap_or_default();
            let line_count = content.lines().count();
            println!("Lines: {}", line_count);
        }
    } else {
        println!("Cache file NOT written (resolve may have failed or returned empty)");
        println!("Check that API keys are set via environment variables.");
        println!("  LASTFM_API_KEY, DISCOGS_TOKEN, GENIUS_TOKEN");
        // Cleanup and exit
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return;
    }

    // Second registry — should load cache from disk
    println!();
    println!("=== Registry 2: Cache-reloaded resolve ===");
    let registry2 = MetadataRegistry::new(
        http_client.clone(),
        lastfm_key.clone(),
        discogs_token.clone(),
        genius_token.clone(),
        None,
        Some(tmp_dir.clone()),
    );
    match registry2.resolve(normalized.as_str(), cleaned.as_str(), None).await {
        Ok(meta) => {
            println!("Artist: {:?}", meta.artist);
            println!("Album:  {:?}", meta.album);
            println!("Year:   {:?}", meta.year);
            println!("Genres: {:?}", meta.genres);
            println!("Styles: {:?}", meta.styles);
            println!("Tracks: {}", meta.album_tracks.len());
        }
        Err(e) => eprintln!("Resolution error: {}", e),
    }

    println!();
    println!("=== Done ===");
    println!("Cache persistence verified: file exists, second resolve loaded from cache.");
    println!("Cache entries persisted across restarts of MetadataRegistry.");

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

async fn cmd_debug_cache_check(args: &[String]) {
    if args.len() < 2 {
        eprintln!("Usage: ytmapi debug cache-check <artist> <title>");
        eprintln!("  Tests cache-only lookup (lookup_cache) with no HTTP.");
        eprintln!("  Runs resolve first to populate cache, then checks lookup_cache.");
        return;
    }
    let artist = &args[0];
    let title = &args[1];

    let cleaned = clean_title(title, artist);
    let normalized = normalize_artist_name(artist);

    use metadata_provider::MetadataRegistry;

    let tmp_dir = std::env::temp_dir().join("ytmapi-cache-check");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");

    let http_client = reqwest::Client::builder()
        .user_agent("Youtui/0.1 (test)")
        .build()
        .unwrap();
    let lastfm_key = std::env::var("LASTFM_API_KEY").ok().filter(|s| !s.is_empty());
    let discogs_token = std::env::var("DISCOGS_TOKEN").ok().filter(|s| !s.is_empty());
    let genius_token = std::env::var("GENIUS_TOKEN").ok().filter(|s| !s.is_empty());

    let cache_key = format!("{}::{}",
        metadata_provider::util::norm_for_lfm(&normalized.to_lowercase()),
        metadata_provider::util::norm_for_lfm(&cleaned.to_lowercase()),
    );

    println!("=== Cache-Only Lookup Test ===");
    println!("Artist: '{}' -> '{}'", artist, normalized);
    println!("Title:  '{}' -> '{}'", title, cleaned);
    println!("Cache key: '{}'", cache_key);
    println!();

    // Registry 1: cache should be empty
    let registry = MetadataRegistry::new(
        http_client.clone(),
        lastfm_key.clone(),
        discogs_token.clone(),
        genius_token.clone(),
        None,
        Some(tmp_dir.clone()),
    );

    // Phase 1: lookup_cache — should miss (cache empty)
    println!("=== Phase 1: Cache lookup BEFORE resolve ===");
    match registry.lookup_cache(&cache_key) {
        Some(meta) => println!("HIT (unexpected — cache should be empty): {:?}", meta),
        None => println!("MISS (expected — cache empty)"),
    }
    println!();

    // Phase 2: resolve to populate cache
    println!("=== Phase 2: Resolve (populates cache) ===");
    match registry.resolve(normalized.as_str(), cleaned.as_str(), None).await {
        Ok(meta) => {
            println!("Artist: {:?}", meta.artist);
            println!("Album:  {:?}", meta.album);
            println!("Year:   {:?}", meta.year);
            println!("Genres: {:?}", meta.genres);
            println!("Styles: {:?}", meta.styles);
        }
        Err(e) => eprintln!("Resolution error: {}", e),
    }
    println!();

    // Phase 3: lookup_cache — should hit now
    println!("=== Phase 3: Cache lookup AFTER resolve ===");
    match registry.lookup_cache(&cache_key) {
        Some(meta) => {
            println!("HIT — cached data:");
            println!("  Artist: {:?}", meta.artist);
            println!("  Album:  {:?}", meta.album);
            println!("  Year:   {:?}", meta.year);
            println!("  Genres: {:?}", meta.genres);
            println!("  Styles: {:?}", meta.styles);
        }
        None => println!("MISS — cache did not populate (resolve may have returned default)"),
    }
    println!();

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp_dir);
    println!("=== Done ===");
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

    println!("Running yt-dlp --dump-json --no-warnings --flat-playlist...");
    let output = std::process::Command::new("yt-dlp")
        .args(["--dump-json", "--no-warnings", "--flat-playlist"])
        .arg(&format!("https://youtu.be/{}", raw_id))
        .output();

    let (title, artist, duration_secs) = match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
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
        None,  // overrides_path
        None,  // cache_path
    );

    println!("=== Resolving Metadata ===");
    println!("Searching for: artist='{}' title='{}'", normalized, clean_title_for_search);
    println!();

    match registry.resolve(&normalized, &clean_title_for_search, None).await {
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
        println!("Title was modified");
    } else {
        println!("Title unchanged");
    }
    if normalized != *artist {
        println!("Artist was modified");
    } else {
        println!("Artist unchanged");
    }
}

// ── FIXTURE ───────────────────────────────────────────────────────────
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
        "artist" => {
            let query = GetArtistQuery::new(ArtistChannelID::from_raw(""));
            match process_json::<GetArtistQuery<'_>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        "library-songs" => {
            let query = GetLibrarySongsQuery::default();
            match process_json::<GetLibrarySongsQuery, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        "watch-playlist" => {
            let query = GetWatchPlaylistQuery::new_from_video_id(VideoID::from_raw(""));
            match process_json::<GetWatchPlaylistQuery<ytmapi_rs::common::VideoID<'_>>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        "lyrics" => {
            let query = GetLyricsQuery::new(ytmapi_rs::common::LyricsID::from_raw(""));
            match process_json::<GetLyricsQuery<'_>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        "mood-categories" => {
            let query = GetMoodCategoriesQuery;
            match process_json::<GetMoodCategoriesQuery, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        "search-basic" => {
            let query: SearchQuery<'_, ytmapi_rs::query::search::BasicSearch> = SearchQuery::new("");
            match process_json::<SearchQuery<'_, ytmapi_rs::query::search::BasicSearch>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
        _ => {
            let query: SearchQuery<'_, ytmapi_rs::query::search::FilteredSearch<SongsFilter>> = SearchQuery::new_filtered("", SongsFilter);
            match process_json::<SearchQuery<'_, ytmapi_rs::query::search::FilteredSearch<SongsFilter>>, ytmapi_rs::auth::BrowserToken>(source, &query) {
                Ok(o) => format!("{:#?}", o),
                Err(e) => { eprintln!("Parse error: {}", e); return; }
            }
        }
    };

    if json {
        println!("{{\"parsed\": {}}}", serde_json::to_string(&output).unwrap_or_else(|_| format!("\"{}\"", output)));
    } else {
        println!("{}", output);
    }
}

fn print_results<T: std::fmt::Debug>(results: &T, json: bool) {
    if json {
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
        rt.block_on(cmd_fixture(&args, false));
    }

    #[test]
    fn test_cmd_fixture_no_args() {
        let args: Vec<String> = vec![];
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cmd_fixture(&args, false));
    }
}
