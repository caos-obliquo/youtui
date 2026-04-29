use crate::app::queue_persistence::{CompactSongRef, CompactSavedQueue};
use crate::app::server::song_downloader::InMemSong;
use crate::app::server::song_thumbnail_downloader::SongThumbnailID;
use crate::app::server::{DecodeSong, GetSongThumbnail, PlayDecodedSong, Stop, TaskMetadata};
use crate::app::structures::{
    AlbumArtState, DownloadStatus, ListSong, ListSongDisplayableField, ListSongID, ListStatus,
    MaybeRc, PlayState,
};
use crate::app::ui::playlist::{
    DownloadTask, HandleGetSongThumbnailError, HandleGetSongThumbnailOk,
    HandlePlayUpdateError, HandlePlayUpdateOk, HandleStopped, Playlist, QueueState,
};
use crate::async_rodio_sink::{AllStopped, Stopped};
use async_callback_manager::{AsyncTask, Constraint, TryBackendTaskExt};
use pretty_assertions::assert_eq;
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
        DecodeSong(dummy_song.clone()).map_stream(PlayDecodedSong(ListSongID(1))),
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
    };
    
    let json = serde_json::to_string(&song_ref).unwrap();
    let parsed: CompactSongRef = serde_json::from_str(&json).unwrap();
    
    assert_eq!(parsed.video_id.get_raw(), song_ref.video_id.get_raw());
    assert_eq!(parsed.title, song_ref.title);
    assert_eq!(parsed.artists, song_ref.artists);
    assert_eq!(parsed.album, song_ref.album);
    assert_eq!(parsed.duration_string, song_ref.duration_string);
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
        },
        CompactSongRef {
            video_id: VideoID::from_raw("song2"),
            title: "Second Song".to_string(),
            artists: vec!["Artist".to_string()],
            album: Some("Album".to_string()),
            duration_string: "4:00".to_string(),
            thumbnail_url: None,
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