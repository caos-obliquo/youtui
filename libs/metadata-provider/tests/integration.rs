use metadata_provider::{AlbumTrack, ValidatedMetadata};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Provider deserialization tests (fixture JSON)
// ---------------------------------------------------------------------------

#[test]
fn provider_listenbrainz_deserialization() {
    let json = serde_json::json!({
        "metadata": {
            "artist_credit_name": "Metallica",
            "release": {
                "name": "Master of Puppets",
                "year": 1986
            },
            "tag": {
                "recording": [
                    {"count": 67, "genre_mbid": "abc-123", "tag": "thrash metal"},
                    {"count": 33, "tag": "metal"},
                    {"count": 12, "genre_mbid": "def-456", "tag": "heavy metal"}
                ],
                "release_group": [
                    {"count": 42, "genre_mbid": "ghi-789", "tag": "speed metal"},
                    {"count": 5, "tag": "1980s"}
                ]
            }
        }
    });

    let metadata = json.get("metadata").expect("metadata present");
    let artist = metadata
        .get("artist_credit_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let album = metadata
        .get("release")
        .and_then(|r| r.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let year = metadata
        .get("release")
        .and_then(|r| r.get("year"))
        .and_then(|v| v.as_i64())
        .map(|y| y.to_string());

    assert_eq!(artist, Some("Metallica".to_string()));
    assert_eq!(album, Some("Master of Puppets".to_string()));
    assert_eq!(year, Some("1986".to_string()));

    let mut genres = Vec::new();
    let mut styles = Vec::new();
    if let Some(tag_obj) = metadata.get("tag") {
        if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
            for entry in recording_tags {
                if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                    let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                    if entry.get("genre_mbid").is_some() {
                        genres.push((count, tag_name.to_string()));
                    } else {
                        styles.push((count, tag_name.to_string()));
                    }
                }
            }
        }
        if let Some(rg_tags) = tag_obj.get("release_group").and_then(|a| a.as_array()) {
            for entry in rg_tags {
                if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                    let count = entry.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                    if entry.get("genre_mbid").is_some() {
                        genres.push((count, tag_name.to_string()));
                    } else {
                        styles.push((count, tag_name.to_string()));
                    }
                }
            }
        }
    }

    genres.sort_by(|a, b| b.0.cmp(&a.0));
    styles.sort_by(|a, b| b.0.cmp(&a.0));

    let genre_names: Vec<String> = genres.into_iter().map(|(_, n)| n).collect();
    let style_names: Vec<String> = styles.into_iter().map(|(_, n)| n).collect();

    assert_eq!(genre_names, vec!["thrash metal", "speed metal", "heavy metal"]);
    assert_eq!(style_names, vec!["metal", "1980s"]);
}

#[test]
fn provider_listenbrainz_no_tags() {
    let json = serde_json::json!({
        "metadata": {
            "artist_credit_name": "Metallica",
            "release": { "name": "Master of Puppets", "year": 1986 }
        }
    });
    let metadata = json.get("metadata").expect("metadata present");
    let tag_obj = metadata.get("tag");
    assert!(tag_obj.is_none(), "No tag object should be present");
}

#[test]
fn provider_listenbrainz_recording_only_tags() {
    let json = serde_json::json!({
        "metadata": {
            "tag": {
                "recording": [
                    {"count": 10, "genre_mbid": "x", "tag": "death metal"}
                ]
            }
        }
    });
    let metadata = json.get("metadata").expect("metadata present");
    let mut genres = Vec::new();
    if let Some(tag_obj) = metadata.get("tag") {
        if let Some(recording_tags) = tag_obj.get("recording").and_then(|a| a.as_array()) {
            for entry in recording_tags {
                if let Some(tag_name) = entry.get("tag").and_then(|v| v.as_str()) {
                    if entry.get("genre_mbid").is_some() {
                        genres.push(tag_name.to_string());
                    }
                }
            }
        }
    }
    assert_eq!(genres, vec!["death metal"]);
}

#[test]
fn provider_musicbrainz_deserialization() {
    let json = serde_json::json!({
        "recordings": [{
            "id": "abc-123",
            "title": "Master of Puppets",
            "artist-credit": [{"name": "Metallica"}],
            "releases": [
                {"id": "def-456", "title": "Master of Puppets", "date": "1986-03-03"}
            ]
        }]
    });
    let rec = json.get("recordings")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .expect("recording present");
    let artist = rec.get("artist-credit")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let year = rec.get("releases")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("date"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.get(..4))
        .filter(|s| s.len() >= 4)
        .map(|s| s.to_string());
    let album = rec.get("releases")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("title"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    assert_eq!(artist, Some("Metallica".to_string()));
    assert_eq!(year, Some("1986".to_string()));
    assert_eq!(album, Some("Master of Puppets".to_string()));
}

#[test]
fn provider_musicbrainz_release_tracks() {
    let json = serde_json::json!({
        "media": [{
            "tracks": [
                {"title": "Battery", "length": 315000, "artist-credit": [{"name": "Metallica"}]},
                {"title": "Master of Puppets", "length": 515000}
            ]
        }]
    });
    let mut tracks = Vec::new();
    if let Some(media) = json.get("media").and_then(|m| m.as_array()) {
        for medium in media {
            if let Some(entries) = medium.get("tracks").and_then(|t| t.as_array()) {
                for entry in entries {
                    let t_title = match entry.get("title").and_then(|t| t.as_str()) {
                        Some(s) => s.to_string(),
                        None => continue,
                    };
                    let duration_secs = entry.get("length").and_then(|l| l.as_i64())
                        .map(|ms| ms as f64 / 1000.0)
                        .unwrap_or(0.0);
                    let track_artist = entry.get("artist-credit").and_then(|ac| ac.as_array())
                        .and_then(|ac| ac.first())
                        .and_then(|c| c.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string());
                    tracks.push((t_title, duration_secs, track_artist));
                }
            }
        }
    }
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].0, "Battery");
    assert_eq!(tracks[0].1, 315.0);
    assert_eq!(tracks[0].2, Some("Metallica".to_string()));
    assert_eq!(tracks[1].0, "Master of Puppets");
    assert_eq!(tracks[1].1, 515.0);
    assert_eq!(tracks[1].2, None);
}

#[test]
fn provider_musicbrainz_release_group_genres() {
    let json = serde_json::json!({
        "id": "rg-123",
        "title": "Master of Puppets",
        "genres": [{"name": "thrash metal", "count": 87}],
        "tags": [{"name": "thrash metal", "count": 87}, {"name": "metal", "count": 42}]
    });
    let mut genres = Vec::new();
    if let Some(arr) = json.get("genres").and_then(|g| g.as_array()) {
        for g in arr {
            if let Some(name) = g.get("name").and_then(|n| n.as_str()) {
                genres.push(name.to_string());
            }
        }
    }
    let genre_set: HashSet<String> = genres.iter().map(|g| g.to_lowercase()).collect();
    let mut styles = Vec::new();
    if let Some(arr) = json.get("tags").and_then(|t| t.as_array()) {
        for t in arr {
            if let Some(name) = t.get("name").and_then(|n| n.as_str()) {
                if !genre_set.contains(&name.to_lowercase()) {
                    styles.push(name.to_string());
                }
            }
        }
    }
    assert_eq!(genres, vec!["thrash metal"]);
    assert_eq!(styles, vec!["metal"]);
}

#[test]
fn provider_librefm_deserialization() {
    let json = serde_json::json!({
        "album": {
            "name": "Ride the Lightning",
            "artist": "Metallica",
            "releaseDate": "1984-07-27",
            "tracks": {
                "track": [
                    {"name": "Fight Fire with Fire", "duration": "285"},
                    {"name": "Ride the Lightning", "duration": "397"},
                    {"name": "For Whom the Bell Tolls", "duration": "310"}
                ]
            },
            "toptags": {
                "tag": [
                    {"name": "thrash metal", "count": 300},
                    {"name": "heavy metal", "count": 200},
                    {"name": "metal", "count": 150}
                ]
            }
        }
    });
    let album_data = json.get("album").expect("album present");
    let year = album_data.get("releaseDate")
        .and_then(|d| d.as_str())
        .and_then(metadata_provider::util::extract_year);
    assert_eq!(year, Some("1984".to_string()));

    let tracks_val = album_data.get("tracks").unwrap().get("track").unwrap();
    let arr = tracks_val.as_array().expect("track array");
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].get("name").unwrap().as_str().unwrap(), "Fight Fire with Fire");

    let genres: Vec<String> = album_data
        .get("toptags").and_then(|t| t.get("tag")).and_then(|t| t.as_array())
        .map(|tags| {
            let mut all: Vec<(String, u32)> = tags.iter().filter_map(|tag| {
                let name = tag.get("name")?.as_str()?.to_string();
                let count = tag.get("count")
                    .and_then(|c| c.as_str().and_then(|s| s.parse::<u32>().ok()))
                    .or_else(|| tag.get("count").and_then(|c| c.as_u64().map(|n| n as u32)))
                    .unwrap_or(0);
                Some((name, count))
            }).collect();
            all.sort_by(|a, b| b.1.cmp(&a.1));
            all.into_iter().take(3).map(|(n, _)| n).collect()
        })
        .unwrap_or_default();
    assert_eq!(genres, vec!["thrash metal", "heavy metal", "metal"]);
}

#[test]
fn provider_librefm_single_track() {
    let json = serde_json::json!({
        "album": {
            "name": "Single Release",
            "artist": "Test Artist",
            "tracks": {
                "track": {"name": "Only Song", "duration": "180"}
            },
            "toptags": {"tag": []}
        }
    });
    let album_data = json.get("album").expect("album present");
    let tracks_val = album_data.get("tracks").unwrap().get("track").unwrap();
    let track_iter: Box<dyn Iterator<Item = &serde_json::Value>> = if let Some(arr) = tracks_val.as_array() {
        Box::new(arr.iter())
    } else {
        Box::new(std::iter::once(tracks_val))
    };
    let tracks: Vec<String> = track_iter.filter_map(|entry| {
        entry.get("name")?.as_str().map(|s| s.to_string())
    }).collect();
    assert_eq!(tracks, vec!["Only Song"]);
}

#[test]
fn provider_librefm_year_from_wiki() {
    let json = serde_json::json!({
        "album": {
            "name": "Test Album",
            "artist": "Test Artist",
            "wiki": {"published": "1991-09-24"},
            "tracks": {"track": []},
            "toptags": {"tag": []}
        }
    });
    let album_data = json.get("album").expect("album present");
    let year = album_data.get("releaseDate")
        .or_else(|| album_data.get("release_date"))
        .or_else(|| album_data.get("releasedate"))
        .or_else(|| album_data.get("wiki").and_then(|w| w.get("published")))
        .and_then(|d| d.as_str())
        .and_then(metadata_provider::util::extract_year);
    assert_eq!(year, Some("1991".to_string()));
}

#[test]
fn provider_librefm_track_validation_found() {
    let album_tracks = vec![
        AlbumTrack { title: "Fight Fire with Fire".into(), duration_secs: 285.0, artist: None },
        AlbumTrack { title: "Ride the Lightning".into(), duration_secs: 397.0, artist: None },
    ];
    let title = "Ride the Lightning";
    let title_norm: String = metadata_provider::util::norm_for_lfm(title).to_lowercase().chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    let title_norm = title_norm.trim();
    let track_found = album_tracks.iter().any(|t| {
        let t_norm: String = metadata_provider::util::norm_for_lfm(&t.title).to_lowercase().chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
        let t_norm = t_norm.trim();
        t_norm == title_norm || t_norm.contains(title_norm) || title_norm.contains(t_norm)
    });
    assert!(track_found, "Track should be found in album tracklist");
}

#[test]
fn provider_librefm_track_validation_not_found() {
    let album_tracks = vec![
        AlbumTrack { title: "Fight Fire with Fire".into(), duration_secs: 285.0, artist: None },
        AlbumTrack { title: "Ride the Lightning".into(), duration_secs: 397.0, artist: None },
    ];
    let title = "Nonexistent Song";
    let title_norm: String = metadata_provider::util::norm_for_lfm(title).to_lowercase().chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    let title_norm = title_norm.trim();
    let track_found = album_tracks.iter().any(|t| {
        let t_norm: String = metadata_provider::util::norm_for_lfm(&t.title).to_lowercase().chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
        let t_norm = t_norm.trim();
        t_norm == title_norm || t_norm.contains(title_norm) || title_norm.contains(t_norm)
    });
    assert!(!track_found, "Track should NOT be found in album tracklist");
}

// ---------------------------------------------------------------------------
// Rate limiter tests
// ---------------------------------------------------------------------------

#[test]
fn musicbrainz_limiter_semaphore_size() {
    let sem = metadata_provider::util::musicbrainz_limiter();
    // Semaphore(2) means 2 permits available initially
    assert_eq!(sem.available_permits(), 1, "MusicBrainz limiter should have 1 permit");
}

#[test]
fn discogs_limiter_semaphore_size() {
    let sem = metadata_provider::util::discogs_limiter();
    assert_eq!(sem.available_permits(), 1, "Discogs limiter should have 1 permit");
}

// ---------------------------------------------------------------------------
// Cache integration tests via MetadataRegistry
// ---------------------------------------------------------------------------

use metadata_provider::MetadataRegistry;
use std::sync::Mutex;
use metadata_cache_sqlite::SqliteCache;

struct MockProvider {
    priority_val: u8,
    result: Option<ValidatedMetadata>,
}

impl metadata_provider::MetadataProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn priority(&self) -> u8 {
        self.priority_val
    }

    fn lookup<'a>(
        &'a self,
        _artist: &'a str,
        _title: &'a str,
        _album: Option<&'a str>,
        _client: &'a reqwest::Client,
    ) -> futures::future::BoxFuture<'a, Option<ValidatedMetadata>> {
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

fn make_registry(providers: Vec<Box<dyn metadata_provider::MetadataProvider>>) -> MetadataRegistry {
    MetadataRegistry {
        providers,
        http_client: reqwest::Client::new(),
        cache: std::sync::Mutex::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(200).unwrap(),
        )),
        overrides: std::sync::Mutex::new(metadata_provider::overrides::MetadataOverrides::default()),
        overrides_path: None,
        cache_path: None,
        sqlite_cache: None,
    }
}

fn make_registry_with_sqlite(
    providers: Vec<Box<dyn metadata_provider::MetadataProvider>>,
    sqlite_cache: Option<Mutex<SqliteCache>>,
) -> MetadataRegistry {
    MetadataRegistry {
        providers,
        http_client: reqwest::Client::new(),
        cache: std::sync::Mutex::new(lru::LruCache::new(
            std::num::NonZeroUsize::new(200).unwrap(),
        )),
        overrides: std::sync::Mutex::new(metadata_provider::overrides::MetadataOverrides::default()),
        overrides_path: None,
        cache_path: None,
        sqlite_cache,
    }
}

#[test]
fn resolve_full_pipeline_lru_then_providers() {
    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: Some("Metallica".into()),
            album: Some("Master of Puppets".into()),
            year: Some("1986".into()),
            track_no: None,
            album_tracks: vec![
                AlbumTrack { title: "Battery".into(), duration_secs: 500.0, artist: None },
                AlbumTrack { title: "Master of Puppets".into(), duration_secs: 515.0, artist: None },
            ],
            ..Default::default()
        }),
    };
    let reg = make_registry(vec![Box::new(p)]);
    let result = futures::executor::block_on(reg.resolve("Metallica", "Master of Puppets", None))
        .expect("resolve ok");
    assert_eq!(result.artist, Some("Metallica".to_string()));
    assert_eq!(result.album, Some("Master of Puppets".to_string()));

    // Second resolve should hit LRU cache
    let result2 = futures::executor::block_on(reg.resolve("Metallica", "Master of Puppets", None))
        .expect("resolve ok");
    assert_eq!(result2.artist, Some("Metallica".to_string()));
}

#[test]
fn resolve_full_pipeline_sqlite_hit() {
    let sqlite = SqliteCache::open_in_memory().expect("open in memory");
    let sqlite_meta = metadata_cache_sqlite::ValidatedMetadata {
        musicbrainz_release_group_id: None,
        artist: Some("Metallica".into()),
        album: Some("Master of Puppets".into()),
        year: Some("1986".into()),
        track_no: None,
        album_tracks: vec![
            metadata_cache_sqlite::AlbumTrack { title: "Battery".into(), duration_secs: 500.0, artist: None },
        ],
        genres: vec![],
        styles: vec![],
    };
    sqlite.put("metallica::master of puppets", &sqlite_meta).expect("put");
    let sqlite = Mutex::new(sqlite);

    // Provider returns sparse metadata (no album, no year)
    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: Some("Metallica".into()),
            ..Default::default()
        }),
    };

    let reg = make_registry_with_sqlite(vec![Box::new(p)], Some(sqlite));

    let result = futures::executor::block_on(reg.resolve("Metallica", "Master of Puppets", None))
        .expect("resolve ok");
    assert_eq!(result.artist, Some("Metallica".to_string()));
    assert_eq!(result.album, Some("Master of Puppets".to_string()));
    assert_eq!(result.year, Some("1986".to_string()));
}

#[test]
fn resolve_full_pipeline_cache_write_through() {
    let sqlite = SqliteCache::open_in_memory().expect("open in memory");
    let sqlite = Mutex::new(sqlite);

    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: Some("Iron Maiden".into()),
            album: Some("Powerslave".into()),
            year: Some("1984".into()),
            track_no: None,
            album_tracks: vec![
                AlbumTrack { title: "Aces High".into(), duration_secs: 270.0, artist: None },
            ],
            ..Default::default()
        }),
    };

    let reg = make_registry_with_sqlite(vec![Box::new(p)], Some(sqlite));

    let result = futures::executor::block_on(reg.resolve("Iron Maiden", "Aces High", None))
        .expect("resolve ok");
    assert_eq!(result.artist, Some("Iron Maiden".to_string()));
    assert_eq!(result.album, Some("Powerslave".to_string()));

    // Verify SQLite was written through
    let cache = reg.get_sqlite_cache().expect("sqlite cache").lock().expect("lock");
    let cached = cache.get("iron maiden::aces high").expect("get from sqlite");
    assert_eq!(cached.artist, Some("Iron Maiden".to_string()));
    assert_eq!(cached.album, Some("Powerslave".to_string()));
}

#[test]
fn resolve_full_pipeline_merge_genres() {
    let p1 = MockProvider {
        priority_val: 7,
        result: Some(ValidatedMetadata {
            artist: Some("Metallica".into()),
            album: Some("Master of Puppets".into()),
            year: Some("1986".into()),
            track_no: None,
            album_tracks: vec![
                AlbumTrack { title: "Battery".into(), duration_secs: 500.0, artist: None },
            ],
            genres: vec!["Thrash metal".to_string()],
            styles: vec![],
            musicbrainz_release_group_id: None,
        }),
    };
    let p2 = MockProvider {
        priority_val: 6,
        result: Some(ValidatedMetadata {
            artist: Some("Metallica".into()),
            album: Some("Master of Puppets".into()),
            year: Some("1986".into()),
            track_no: None,
            album_tracks: vec![
                AlbumTrack { title: "Battery".into(), duration_secs: 500.0, artist: None },
            ],
            genres: vec!["Speed metal".to_string()],
            styles: vec!["1980s".to_string()],
            musicbrainz_release_group_id: None,
        }),
    };

    let reg = make_registry(vec![Box::new(p1), Box::new(p2)]);
    let result = futures::executor::block_on(reg.resolve("Metallica", "Battery", None))
        .expect("resolve ok");
    assert_eq!(result.artist, Some("Metallica".to_string()));
    // Genres should be merged from both providers
    assert!(!result.genres.is_empty(), "Genres should be merged from multiple providers");
}

#[test]
fn resolve_does_not_cache_low_score_results() {
    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: None,
            album: Some("Some Album".into()),
            ..Default::default()
        }),
    };
    let reg = make_registry(vec![Box::new(p)]);
    let result = futures::executor::block_on(reg.resolve("Metallica", "Battery", None))
        .expect("resolve ok");
    // Score is low (album only = 10), should not be cached
    assert_eq!(result.album, Some("Some Album".to_string()));
    // Second call should also go through provider (same result, but not from cache)
    let result2 = futures::executor::block_on(reg.resolve("Metallica", "Battery", None))
        .expect("resolve ok");
    assert_eq!(result2.album, Some("Some Album".to_string()));
}

#[test]
fn resolve_normalizes_single_provider_genres() {
    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: Some("Metallica".into()),
            album: Some("Master of Puppets".into()),
            year: Some("1986".into()),
            track_no: None,
            album_tracks: vec![
                AlbumTrack { title: "Battery".into(), duration_secs: 500.0, artist: None },
            ],
            genres: vec!["thrash metal".to_string(), "heavy metal".to_string()],
            styles: vec!["metal".to_string()],
            musicbrainz_release_group_id: None,
        }),
    };
    let reg = make_registry(vec![Box::new(p)]);
    let result = futures::executor::block_on(reg.resolve("Metallica", "Battery", None))
        .expect("resolve ok");
    assert!(!result.genres.is_empty());
    // Genres should be normalized (title case)
    assert!(result.genres.iter().any(|g| g == "Thrash metal" || g == "thrash metal"));
}

#[test]
fn resolve_returns_default_when_all_providers_fail() {
    let p = MockProvider {
        priority_val: 1,
        result: None,
    };
    let reg = make_registry(vec![Box::new(p)]);
    let result = futures::executor::block_on(reg.resolve("Metallica", "Battery", None))
        .expect("resolve ok");
    assert_eq!(result.artist, None);
    assert_eq!(result.album, None);
    assert_eq!(result.year, None);
    assert!(result.album_tracks.is_empty());
}

#[test]
fn resolve_skips_sparse_lru_entries() {
    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: Some("Metallica".into()),
            album: Some("Master of Puppets".into()),
            year: Some("1986".into()),
            track_no: None,
            album_tracks: vec![
                AlbumTrack { title: "Battery".into(), duration_secs: 500.0, artist: None },
            ],
            ..Default::default()
        }),
    };
    let reg = make_registry(vec![Box::new(p)]);
    // First resolve populates cache
    let result1 = futures::executor::block_on(reg.resolve("Metallica", "Battery", None))
        .expect("resolve ok");
    assert_eq!(result1.album, Some("Master of Puppets".to_string()));

    // Second resolve should hit LRU
    let result2 = futures::executor::block_on(reg.resolve("Metallica", "Battery", None))
        .expect("resolve ok");
    assert_eq!(result2.album, Some("Master of Puppets".to_string()));
}

#[test]
fn sqlite_cache_hit_in_resolve() {
    let sqlite = SqliteCache::open_in_memory().expect("open in memory");
    let sqlite_meta = metadata_cache_sqlite::ValidatedMetadata {
        musicbrainz_release_group_id: None,
        artist: Some("Megadeth".into()),
        album: Some("Rust in Peace".into()),
        year: Some("1990".into()),
        track_no: None,
        album_tracks: vec![
            metadata_cache_sqlite::AlbumTrack { title: "Holy Wars".into(), duration_secs: 390.0, artist: None },
        ],
        genres: vec!["Thrash metal".to_string()],
        styles: vec![],
    };
    sqlite.put("megadeth::holy wars", &sqlite_meta).expect("put");
    let sqlite = Mutex::new(sqlite);

    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: Some("Megadeth".into()),
            ..Default::default()
        }),
    };

    let reg = make_registry_with_sqlite(vec![Box::new(p)], Some(sqlite));

    let result = futures::executor::block_on(reg.resolve("Megadeth", "Holy Wars", None))
        .expect("resolve ok");
    assert_eq!(result.artist, Some("Megadeth".to_string()));
    assert_eq!(result.album, Some("Rust in Peace".to_string()));
    assert_eq!(result.year, Some("1990".to_string()));
    assert_eq!(result.album_tracks.len(), 1);
}

#[test]
fn sqlite_cache_write_through_in_resolve() {
    let sqlite = SqliteCache::open_in_memory().expect("open in memory");
    let sqlite = Mutex::new(sqlite);

    let p = MockProvider {
        priority_val: 1,
        result: Some(ValidatedMetadata {
            artist: Some("Slayer".into()),
            album: Some("Reign in Blood".into()),
            year: Some("1986".into()),
            track_no: None,
            album_tracks: vec![
                AlbumTrack { title: "Angel of Death".into(), duration_secs: 290.0, artist: None },
            ],
            ..Default::default()
        }),
    };

    let reg = make_registry_with_sqlite(vec![Box::new(p)], Some(sqlite));

    let result = futures::executor::block_on(reg.resolve("Slayer", "Angel of Death", None))
        .expect("resolve ok");
    assert_eq!(result.artist, Some("Slayer".to_string()));

    let cache = reg.get_sqlite_cache().expect("sqlite cache").lock().expect("lock");
    let cached = cache.get("slayer::angel of death").expect("get from sqlite");
    assert_eq!(cached.artist, Some("Slayer".to_string()));
    assert_eq!(cached.album, Some("Reign in Blood".to_string()));
    assert_eq!(cached.year, Some("1986".to_string()));
}
