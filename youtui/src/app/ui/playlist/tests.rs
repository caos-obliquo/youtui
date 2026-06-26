use crate::app::queue_persistence::{CompactSongRef, CompactSavedQueue};
use crate::app::server::song_downloader::InMemSong;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use crate::app::server::{AlbumTrack, DecodeSong, GetSongThumbnail, PlayDecodedSong, TaskMetadata};
use crate::app::structures::{
    AlbumArtState, DownloadStatus, ListSong, ListSongArtist, ListSongDisplayableField,
    ListSongID, ListStatus, MaybeRc, PlayState,
};
use ytmapi_rs::common::LikeStatus;
use crate::app::ui::playlist::{
    DownloadTask, HandleGetSongThumbnailError, HandleGetSongThumbnailOk,
    HandlePlayUpdateError, HandlePlayUpdateOk, Playlist, QueueState,
};
use async_callback_manager::{AsyncTask, Constraint, TryBackendTaskExt};
use pretty_assertions::assert_eq;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use ytmapi_rs::auth::BrowserToken;
use ytmapi_rs::common::{AlbumID, Thumbnail, VideoID, YoutubeID};
use ytmapi_rs::parse::{GetAlbum, ParsedSongAlbum};
use ytmapi_rs::query::GetAlbumQuery;

static DUMMY_ALBUM: OnceLock<GetAlbum> = OnceLock::new();

fn get_dummy_album() -> GetAlbum {
    DUMMY_ALBUM
        .get_or_init(|| {
            let json =
                std::fs::read_to_string("../ytmapi-rs/test_json/get_album_20240724.json").unwrap();
            ytmapi_rs::process_json::<_, BrowserToken>(
                json,
                GetAlbumQuery::new(AlbumID::from_raw("")),
            )
            .unwrap()
        })
        .clone()
}

fn get_dummy_playlist() -> Playlist {
    let (mut playlist, _effect) = Playlist::new();
    playlist.list.state = ListStatus::Loaded;
    let GetAlbum {
        title,
        year,
        tracks,
        ..
    } = get_dummy_album();
    playlist.list.append_raw_album_songs(
        tracks,
        ParsedSongAlbum {
            name: title,
            id: AlbumID::from_raw(""),
        },
        year,
        vec![],
        vec![],
    );
    playlist
}

#[test]
fn newly_added_song_downloads_album_art() {
    let mut p = get_dummy_playlist();
    let s = p.list.get_list_iter_mut().next().unwrap();
    s.thumbnails = MaybeRc::Owned(vec![Thumbnail {
        height: 0,
        width: 0,
        url: "dummy_url".to_string(),
    }]);
    let dummy_song = s.clone();
    let thumbnail_id = SongThumbnailID::from(&dummy_song as &ListSong).into_owned();
    let (_, effect) = p.push_song_list(vec![dummy_song]);
    let expected_effect = AsyncTask::new_future_try(
        GetSongThumbnail {
            thumbnail_url: "dummy_url".to_string(),
            thumbnail_id: thumbnail_id.clone(),
        },
        HandleGetSongThumbnailOk,
        HandleGetSongThumbnailError(thumbnail_id),
        None,
    );
    assert!(
        effect.contains(&expected_effect),
        "Expected Left to contain Right {}",
        pretty_assertions::Comparison::new(&effect, &expected_effect)
    );
}

#[test]
fn downloaded_song_plays_if_buffered() {
    let mut p = get_dummy_playlist();
    p.play_status = PlayState::Buffering(ListSongID(1));
    let dummy_song = Arc::new(InMemSong(vec![1]));
    p.list.get_list_iter_mut().nth(1).unwrap().download_status =
        DownloadStatus::Downloaded(dummy_song.clone());
    let effect = p.handle_song_downloaded(ListSongID(1));
    assert_eq!(p.play_status, PlayState::Playing(ListSongID(1)));
    let expected_effect = AsyncTask::new_stream_try(
        DecodeSong(dummy_song.clone(), None, None).map_stream(PlayDecodedSong(ListSongID(1))),
        HandlePlayUpdateOk,
        HandlePlayUpdateError(ListSongID(1)),
        Some(Constraint::new_block_matching_metadata(
            TaskMetadata::PlayingSong,
        )),
    );
    assert!(
        effect.contains(&expected_effect),
        "Expected to contain effect to play song {:?}",
        expected_effect
    );
}

#[test]
fn queued_song_plays_if_not_already_playing() {
    let mut p = get_dummy_playlist();
    p.play_status = PlayState::Buffering(ListSongID(0));
    p.queue_status = QueueState::Queued(ListSongID(0));
    let dummy_song = Arc::new(InMemSong(vec![1]));
    p.list.get_list_iter_mut().nth(0).unwrap().download_status =
        DownloadStatus::Downloaded(dummy_song.clone());
    let _effect = p.handle_song_downloaded(ListSongID(0));
    assert_eq!(p.play_status, PlayState::Playing(ListSongID(0)));
    // queue_status is set to NotQueued by autoplay_song_id
    assert_eq!(p.queue_status, QueueState::NotQueued);
}

#[test]
fn compact_song_ref_contains_all_fields() {
    let song_ref = CompactSongRef {
        video_id: VideoID::from_raw("test123"),
        title: "Test Song".to_string(),
        artists: vec!["Artist 1".to_string(), "Artist 2".to_string()],
        album: Some("Test Album".to_string()),
        duration_string: "3:45".to_string(),
        thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
        like_status: LikeStatus::Indifferent,
    };
    
    assert_eq!(song_ref.video_id.get_raw(), "test123");
    assert_eq!(song_ref.title, "Test Song");
    assert_eq!(song_ref.artists.len(), 2);
    assert_eq!(song_ref.album, Some("Test Album".to_string()));
    assert_eq!(song_ref.duration_string, "3:45");
    assert!(song_ref.thumbnail_url.is_some());
}

#[test]
fn compact_song_ref_serialization_roundtrip() {
    let song_ref = CompactSongRef {
        video_id: VideoID::from_raw("abc123"),
        title: "Roundtrip Test".to_string(),
        artists: vec!["Solo Artist".to_string()],
        album: None,
        duration_string: "4:20".to_string(),
        thumbnail_url: None,
        like_status: LikeStatus::Indifferent,
    };
    
    let json = serde_json::to_string(&song_ref).unwrap();
    let parsed: CompactSongRef = serde_json::from_str(&json).unwrap();
    
    assert_eq!(parsed.video_id.get_raw(), song_ref.video_id.get_raw());
    assert_eq!(parsed.title, song_ref.title);
    assert_eq!(parsed.artists, song_ref.artists);
    assert_eq!(parsed.album, song_ref.album);
    assert_eq!(parsed.duration_string, song_ref.duration_string);
    assert_eq!(parsed.like_status, song_ref.like_status);
}

#[test]
fn compact_queue_with_current_index() {
    let songs = vec![
        CompactSongRef {
            video_id: VideoID::from_raw("song1"),
            title: "First Song".to_string(),
            artists: vec!["Artist".to_string()],
            album: Some("Album".to_string()),
            duration_string: "3:00".to_string(),
            thumbnail_url: None,
            like_status: LikeStatus::Indifferent,
        },
        CompactSongRef {
            video_id: VideoID::from_raw("song2"),
            title: "Second Song".to_string(),
            artists: vec!["Artist".to_string()],
            album: Some("Album".to_string()),
            duration_string: "4:00".to_string(),
            thumbnail_url: None,
            like_status: LikeStatus::Indifferent,
        },
    ];
    
    let queue = CompactSavedQueue {
        songs,
        current_index: Some(1),
    };
    
    let json = serde_json::to_string(&queue).unwrap();
    let parsed: CompactSavedQueue = serde_json::from_str(&json).unwrap();
    
    assert_eq!(parsed.songs.len(), 2);
    assert_eq!(parsed.current_index, Some(1));
    assert_eq!(parsed.songs[1].title, "Second Song");
}

#[test]
fn download_task_creation() {
    let cancel_token = Arc::new(tokio_util::sync::CancellationToken::new());
    let task = DownloadTask {
        cancel_token,
    };
    
    assert!(task.cancel_token.is_cancelled() == false);
}

#[test]
fn list_song_create_with_metadata_has_album() {
    let song = ListSong::create_with_metadata(
        VideoID::from_raw("test"),
        "Title".to_string(),
        vec!["Artist".to_string()],
        Some("Album Name".to_string()),
        "3:33".to_string(),
        None,
    );
    
    use crate::app::structures::ListSongDisplayableField;
    
    assert!(song.album.is_some());
    assert_eq!(song.album.as_ref().unwrap().name, "Album Name");
    assert_eq!(song.get_field(ListSongDisplayableField::Artists).as_ref(), "Artist");
    assert_eq!(song.title, "Title");
}

#[test]
fn list_song_create_with_metadata_no_album() {
    let song = ListSong::create_with_metadata(
        VideoID::from_raw("test"),
        "Title".to_string(),
        vec!["Artist1".to_string(), "Artist2".to_string()],
        None,
        "4:00".to_string(),
        Some("https://example.com/thumb.jpg".to_string()),
    );
    
    assert!(song.album.is_none());
    assert_eq!(song.get_field(ListSongDisplayableField::Artists).as_ref(), "Artist1, Artist2");
    assert!(!song.thumbnails.is_empty());
}

#[test]
fn songs_ahead_buffer_is_2() {
    assert_eq!(crate::app::ui::playlist::SONGS_AHEAD_TO_BUFFER, 2);
}

#[test]
fn songs_behind_save_is_1() {
    assert_eq!(crate::app::ui::playlist::SONGS_BEHIND_TO_SAVE, 1);
}

fn make_album_original(video: &'static str, year: Option<&str>) -> ListSong {
    ListSong {
        video_id: VideoID::from_raw(video),
        track_no: None,
        plays: String::new(),
        title: "Album Title".into(),
        explicit: None,
        download_status: DownloadStatus::None,
        id: ListSongID(0),
        duration_string: "36:49".into(),
        actual_duration: None,
        start_offset: None,
        year: year.map(|y| Rc::new(y.to_string())),
        album_art: AlbumArtState::None,
        genres: Vec::new(),
        styles: Vec::new(),
        artists: MaybeRc::Owned(vec![ListSongArtist { name: "Artist".into(), id: None }]),
        thumbnails: MaybeRc::Owned(Vec::new()),
        album: None,
        like_status: LikeStatus::Indifferent,
    }
}

fn make_track_entry(video: &'static str, track_no: usize, title: &'static str, duration_secs: f64, start_secs: f64) -> ListSong {
    let dur_secs = duration_secs as u64;
    let dur_str = format!("{}:{:02}", dur_secs / 60, dur_secs % 60);
    ListSong {
        video_id: VideoID::from_raw(video),
        track_no: Some(track_no),
        plays: String::new(),
        title: title.into(),
        explicit: None,
        download_status: DownloadStatus::None,
        id: ListSongID(0),
        duration_string: dur_str,
        actual_duration: Some(Duration::from_secs_f64(duration_secs)),
        start_offset: Some(Duration::from_secs_f64(start_secs)),
        year: None,
        album_art: AlbumArtState::None,
        genres: Vec::new(),
        styles: Vec::new(),
        artists: MaybeRc::Owned(vec![ListSongArtist { name: "Artist".into(), id: None }]),
        thumbnails: MaybeRc::Owned(Vec::new()),
        album: None,
        like_status: LikeStatus::Indifferent,
    }
}

fn dummy_tracks() -> Vec<AlbumTrack> {
    vec![
        AlbumTrack { title: "Track 1".into(), duration_secs: 203.0, artist: None },
        AlbumTrack { title: "Track 2".into(), duration_secs: 148.0, artist: None },
        AlbumTrack { title: "Track 3".into(), duration_secs: 194.0, artist: None },
    ]
}

// --- Album partitioning tests ---

#[test]
fn insert_album_tracks_sets_correct_metadata() {
    let (mut p, _) = Playlist::new();
    p.list.state = ListStatus::Loaded;
    let orig_id = p.list.push_song_list(vec![make_album_original("vx1", Some("2021"))]);
    let tracks = dummy_tracks();

    p.insert_album_tracks(
        orig_id, &tracks,
        &Some("Artist".into()), &Some("Album".into()), &None, &None,
    );

    // original + 3 tracks
    assert_eq!(p.list.get_list_iter().count(), 4);

    // Track 1: offset 0, year from original fallback
    let t1 = p.list.get_list_iter().nth(1).unwrap();
    assert_eq!(t1.track_no, Some(1));
    assert_eq!(t1.start_offset, Some(Duration::from_secs_f64(0.0)));
    assert_eq!(t1.actual_duration, Some(Duration::from_secs_f64(203.0)));
    assert_eq!(t1.title, "Track 1");
    assert_eq!(t1.year, Some(Rc::new("2021".into()))); // fallback from original
    assert_eq!(t1.duration_string, "3:23");

    // Track 2: offset 203
    let t2 = p.list.get_list_iter().nth(2).unwrap();
    assert_eq!(t2.track_no, Some(2));
    assert_eq!(t2.start_offset, Some(Duration::from_secs_f64(203.0)));
    assert_eq!(t2.actual_duration, Some(Duration::from_secs_f64(148.0)));
    assert_eq!(t2.title, "Track 2");

    // Track 3: last track, offset 203+148 = 351, plays to EOF (None duration)
    let t3 = p.list.get_list_iter().nth(3).unwrap();
    assert_eq!(t3.track_no, Some(3));
    assert_eq!(t3.start_offset, Some(Duration::from_secs_f64(351.0)));
    assert_eq!(t3.actual_duration, None); // last track plays to EOF
    assert_eq!(t3.title, "Track 3");
}

#[test]
fn insert_album_tracks_propagates_parent_metadata() {
    use ytmapi_rs::common::Thumbnail;
    let (mut p, _) = Playlist::new();
    p.list.state = ListStatus::Loaded;

    let mut orig = make_album_original("vx1", Some("2021"));
    orig.genres = vec!["Rock".to_string(), "Metal".to_string()];
    orig.styles = vec!["Heavy Metal".to_string()];
    orig.thumbnails = MaybeRc::Owned(vec![Thumbnail {
        height: 100,
        width: 100,
        url: "test.jpg".to_string(),
    }]);
    orig.like_status = LikeStatus::Liked;

    let orig_id = p.list.push_song_list(vec![orig]);
    let tracks = dummy_tracks();

    p.insert_album_tracks(
        orig_id, &tracks,
        &Some("Artist".into()), &Some("Album".into()), &None, &None,
    );

    for i in 0..3 {
        let track = p.list.get_list_iter().nth(i).unwrap();
        assert_eq!(track.genres, vec!["Rock", "Metal"],
            "Track {}: genres should propagate", i + 1);
        assert_eq!(track.styles, vec!["Heavy Metal"],
            "Track {}: styles should propagate", i + 1);
        assert_eq!(track.like_status, LikeStatus::Liked,
            "Track {}: like_status should propagate", i + 1);
        match &track.thumbnails {
            MaybeRc::Owned(t) => {
                assert_eq!(t.len(), 1, "Track {}: should have 1 thumbnail", i + 1);
                assert_eq!(t[0].url, "test.jpg");
            }
            _ => panic!("Track {}: thumbnails should be Owned", i + 1),
        }
    }
}

#[test]
fn album_download_shares_arc_with_tracks() {
    let (mut p, _) = Playlist::new();
    p.list.state = ListStatus::Loaded;
    let orig_id = p.list.push_song_list(vec![make_album_original("vx1", None)]);

    // Real flow: MetadataEffect::Validated sets album_tracks BEFORE insert_album_tracks
    let tracks = vec![
        AlbumTrack { title: "T1".into(), duration_secs: 100.0, artist: None },
        AlbumTrack { title: "T2".into(), duration_secs: 100.0, artist: None },
    ];
    p.album_tracks = Some(tracks.clone());
    p.insert_album_tracks(orig_id, &tracks, &Some("Artist".into()), &Some("Album".into()), &None, &None);

    // Set original's download to Valid data and verify Arc sharing
    let shared = Arc::new(InMemSong(vec![1, 2, 3, 4, 5]));
    p.list.get_list_iter_mut().nth(0).unwrap().download_status = DownloadStatus::Downloaded(shared.clone());

    let _ = p.handle_song_downloaded(orig_id);

    // Both tracks should share the same Arc (original removed, tracks at 0, 1)
    for i in 0..=1 {
        let song = p.list.get_list_iter().nth(i).unwrap();
        match &song.download_status {
            DownloadStatus::Downloaded(a) => assert!(Arc::ptr_eq(a, &shared)),
            _ => panic!("Track {} not Downloaded", i + 1),
        }
    }
}

#[test]
fn play_song_id_uses_start_offset_in_decode() {
    let (mut p, _) = Playlist::new();
    p.list.state = ListStatus::Loaded;

    let data = Arc::new(InMemSong(vec![1, 2, 3]));
    let mut song = make_track_entry("vx1", 2, "Backyards", 148.0, 203.0);
    let offset = song.start_offset;
    let actual_dur = song.actual_duration;
    song.download_status = DownloadStatus::Downloaded(data.clone());
    let id = p.list.push_song_list(vec![song]);

    let effect = p.play_song_id(id);

    let expected = AsyncTask::new_stream_try(
        DecodeSong(data, offset, actual_dur).map_stream(PlayDecodedSong(id)),
        HandlePlayUpdateOk,
        HandlePlayUpdateError(id),
        Some(Constraint::new_block_matching_metadata(TaskMetadata::PlayingSong)),
    );
    assert!(
        effect.contains(&expected),
        "play_song_id should emit DecodeSong with start_offset"
    );
}

#[test]
fn progress_is_relative_to_start_offset() {
    let (mut p, _) = Playlist::new();
    p.list.state = ListStatus::Loaded;

    let data = Arc::new(InMemSong(vec![1]));
    let mut song = make_track_entry("vx1", 2, "Backyards", 148.0, 203.0);
    song.download_status = DownloadStatus::Downloaded(data);
    let id = p.list.push_song_list(vec![song]);

    // Simulate playing this track
    p.play_status = PlayState::Playing(id);

    // Album tracks use ffmpeg extraction → d is already track-relative
    let _ = p.handle_set_song_play_progress(Duration::from_secs(0), id);
    assert_eq!(p.cur_played_dur, Some(Duration::from_secs(0)));

    let _ = p.handle_set_song_play_progress(Duration::from_secs(100), id);
    assert_eq!(p.cur_played_dur, Some(Duration::from_secs(100)));

    let _ = p.handle_set_song_play_progress(Duration::from_secs(148), id);
    assert_eq!(p.cur_played_dur, Some(Duration::from_secs(148)));
}

#[test]
fn non_album_progress_subtracts_offset() {
    let (mut p, _) = Playlist::new();
    p.list.state = ListStatus::Loaded;

    let data = Arc::new(InMemSong(vec![1]));
    // Non-album entry (track_no=None) with start_offset
    let mut song = make_album_original("vx1", None);
    song.download_status = DownloadStatus::Downloaded(data);
    song.start_offset = Some(Duration::from_secs_f64(203.0));
    let id = p.list.push_song_list(vec![song]);

    p.play_status = PlayState::Playing(id);
    // Non-album: d - offset = 250 - 203 = 47
    let _ = p.handle_set_song_play_progress(Duration::from_secs(250), id);
    assert_eq!(p.cur_played_dur, Some(Duration::from_secs(47)));
}

#[test]
fn cancel_all_downloads_triggers_tokens() {
    let p = get_dummy_playlist();
    // Register a download task
    let cancel_token = Arc::new(tokio_util::sync::CancellationToken::new());
    let task = DownloadTask { cancel_token: cancel_token.clone() };
    p.active_downloads.lock().unwrap().push((ListSongID(999), task));

    assert_eq!(p.active_downloads.lock().unwrap().len(), 1);
    assert!(!cancel_token.is_cancelled());

    p.cancel_all_downloads();

    assert!(cancel_token.is_cancelled());
    assert!(p.active_downloads.lock().unwrap().is_empty());
}

#[test]
fn cancel_all_downloads_empty_is_noop() {
    let p = get_dummy_playlist();
    assert!(p.active_downloads.lock().unwrap().is_empty());
    p.cancel_all_downloads();
    assert!(p.active_downloads.lock().unwrap().is_empty());
}

#[test]
fn cancel_all_downloads_multiple_tasks() {
    let p = get_dummy_playlist();
    let token1 = Arc::new(tokio_util::sync::CancellationToken::new());
    let token2 = Arc::new(tokio_util::sync::CancellationToken::new());
    let token3 = Arc::new(tokio_util::sync::CancellationToken::new());
    p.active_downloads.lock().unwrap().push((ListSongID(1), DownloadTask { cancel_token: token1.clone() }));
    p.active_downloads.lock().unwrap().push((ListSongID(2), DownloadTask { cancel_token: token2.clone() }));
    p.active_downloads.lock().unwrap().push((ListSongID(3), DownloadTask { cancel_token: token3.clone() }));

    p.cancel_all_downloads();

    assert!(token1.is_cancelled());
    assert!(token2.is_cancelled());
    assert!(token3.is_cancelled());
    assert!(p.active_downloads.lock().unwrap().is_empty());
}

#[test]
fn download_scope_max_4_songs() {
    // Scope is: prev(1) + current + next(2) = 4 songs
    assert_eq!(
        crate::app::ui::playlist::SONGS_BEHIND_TO_SAVE
            + 1 // current
            + crate::app::ui::playlist::SONGS_AHEAD_TO_BUFFER,
        4
    );
}