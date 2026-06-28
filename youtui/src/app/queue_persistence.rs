use crate::app::structures::{ListSong, Thumbnail};
use crate::app::ui::playlist::Playlist;
use crate::get_data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use ytmapi_rs::common::{LikeStatus, VideoID};

const QUEUE_DIR: &str = "youtui/queues";
const AUTO_SAVE: &str = "__autosave";

#[derive(Serialize, Deserialize)]
struct LegacySong {
    songs: Vec<ListSong>,
    current_index: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CompactSongRef {
    pub video_id: VideoID<'static>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_string: String,
    pub thumbnail_url: Option<String>,
    #[serde(default = "default_like_status")]
    pub like_status: LikeStatus,
}

fn default_like_status() -> LikeStatus {
    LikeStatus::Indifferent
}

#[derive(Serialize, Deserialize)]
pub struct CompactSavedQueue {
    pub songs: Vec<CompactSongRef>,
    pub current_index: Option<usize>,
}

pub fn get_queue_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = get_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(QUEUE_DIR);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn save_queue(playlist: &Playlist, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let raw_songs: Vec<ListSong> = playlist.list.get_list_iter().cloned().collect();

    let get_largest_thumbnail_url = |thumbs: &Vec<Thumbnail>| -> Option<String> {
        thumbs
            .iter()
            .max_by_key(|t| t.height * t.width)
            .map(|t| t.url.clone())
    };

    let songs: Vec<CompactSongRef> = raw_songs
        .iter()
        .map(|song| {
            let artists: Vec<String> = song.artists.iter().map(|a| a.name.clone()).collect();
            let album = song.album.as_ref().map(|a| a.name.clone());
            CompactSongRef {
                video_id: song.video_id.clone(),
                title: song.title.clone(),
                artists,
                album,
                duration_string: song.duration_string.clone(),
                thumbnail_url: get_largest_thumbnail_url(song.thumbnails.as_ref()),
                like_status: song.like_status.clone(),
            }
        })
        .collect();

    let current_idx = playlist.get_cur_playing_index();
    let saved = CompactSavedQueue {
        songs,
        current_index: current_idx,
    };

    let queue_dir = get_queue_dir()?;
    let path = queue_dir.join(format!("{}.json", name));
    let temp_path = queue_dir.join(format!("{}.json.tmp", name));

    let json = serde_json::to_string_pretty(&saved)?;
    let mut temp_file = fs::File::create(&temp_path)?;
    temp_file.write_all(json.as_bytes())?;
    temp_file.sync_all()?;
    drop(temp_file);

    fs::rename(&temp_path, &path)?;

    info!(
        "Successfully saved queue '{}' ({} songs, current_index: {:?})",
        name,
        saved.songs.len(),
        current_idx
    );
    Ok(())
}

pub fn load_queue(playlist: &mut Playlist, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_queue_dir()?.join(format!("{}.json", name));
    debug!("Loading queue from path: {:?}", path);

    let json = fs::read_to_string(&path)?;
    debug!("Read JSON: {}", json);

    if let Ok(saved) = serde_json::from_str::<CompactSavedQueue>(&json) {
        load_compact_queue(playlist, saved)?;
    } else if let Ok(saved) = serde_json::from_str::<LegacySong>(&json) {
        normalize_and_load(playlist, saved, name)?;
    } else {
        warn!("Queue file corrupted, starting fresh");
    }
    Ok(())
}

fn load_compact_queue(
    playlist: &mut Playlist,
    saved: CompactSavedQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Loaded compact queue with {} songs", saved.songs.len());
    info!("Clearing playlist (reset)");
    let _ = playlist.reset();

    if !saved.songs.is_empty() {
        let songs: Vec<ListSong> = saved
            .songs
            .iter()
            .map(|ref_| {
                let mut song = ListSong::create_with_metadata(
                    ref_.video_id.clone(),
                    ref_.title.clone(),
                    ref_.artists.clone(),
                    ref_.album.clone(),
                    ref_.duration_string.clone(),
                    ref_.thumbnail_url.clone(),
                );
                song.like_status = ref_.like_status.clone();
                song
            })
            .collect();

        info!("Created {} songs from compact metadata", songs.len());
        let (_first_id, _effect) = playlist.push_song_list(songs);

        if let Some(idx) = saved.current_index {
            if let Some(song_id) = playlist.get_id_from_index(idx) {
                let _effect = playlist.play_song_id(song_id);
                info!("Restored playback to song at index {}", idx);
            } else {
                warn!("Saved index {} out of bounds, not restoring playback", idx);
            }
        }
        info!("Load complete");
    } else {
        info!("No songs to load from save file");
    }
    Ok(())
}

fn normalize_and_load(
    playlist: &mut Playlist,
    saved: LegacySong,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Normalizing queue file to compact format");
    let get_largest_thumbnail_url = |thumbs: &Vec<Thumbnail>| -> Option<String> {
        thumbs
            .iter()
            .max_by_key(|t| t.height * t.width)
            .map(|t| t.url.clone())
    };

    let songs: Vec<CompactSongRef> = saved
        .songs
        .iter()
        .map(|song| {
            let artists: Vec<String> = song.artists.iter().map(|a| a.name.clone()).collect();
            let album = song.album.as_ref().map(|a| a.name.clone());
            CompactSongRef {
                video_id: song.video_id.clone(),
                title: song.title.clone(),
                artists,
                album,
                duration_string: song.duration_string.clone(),
                thumbnail_url: get_largest_thumbnail_url(song.thumbnails.as_ref()),
                like_status: song.like_status.clone(),
            }
        })
        .collect();

    let current_idx = saved.current_index;
    let compact = CompactSavedQueue {
        songs,
        current_index: current_idx,
    };

    let queue_dir = get_queue_dir()?;
    let path = queue_dir.join(format!("{}.json", name));
    let temp_path = queue_dir.join(format!("{}.json.tmp", name));

    let json = serde_json::to_string_pretty(&compact)?;
    let mut file = fs::File::create(&temp_path)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    fs::rename(&temp_path, &path)?;

    info!("Clearing playlist (reset)");
    let _ = playlist.reset();
    load_compact_queue(playlist, compact)
}

pub fn auto_save(playlist: &Playlist) -> Result<(), Box<dyn std::error::Error>> {
    let count = playlist.list.get_list_iter().count();
    info!("Saving queue ({} songs)", count);
    save_queue(playlist, AUTO_SAVE)
}

pub fn auto_load(playlist: &mut Playlist) -> Result<(), Box<dyn std::error::Error>> {
    info!("Loading saved queue");
    load_queue(playlist, AUTO_SAVE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ytmapi_rs::common::YoutubeID;

    #[test]
    fn test_compact_song_ref_serialization() {
        let video_id = VideoID::from_raw("abc123");
        let song_ref = CompactSongRef {
            video_id: video_id.clone(),
            title: "Test Song".to_string(),
            artists: vec!["Artist 1".to_string(), "Artist 2".to_string()],
            album: Some("Test Album".to_string()),
            duration_string: "3:45".to_string(),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            like_status: LikeStatus::Liked,
        };
        let json = serde_json::to_string_pretty(&song_ref).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("video_id"));
        assert!(json.contains("Test Song"));
        assert!(json.contains("Artist 1"));
        assert!(json.contains("Artist 2"));
        assert!(json.contains("Test Album"));
        assert!(json.contains("album"));
    }

    #[test]
    fn test_compact_song_ref_deserialization() {
        let json = r#"{
            "video_id": "test123",
            "title": "Loaded Song",
            "artists": ["Artist X", "Artist Y"],
            "album": "Album Name",
            "duration_string": "5:00",
            "thumbnail_url": null,
            "like_status": "LIKE"
        }"#;

        let song_ref: CompactSongRef = serde_json::from_str(json).unwrap();
        assert_eq!(song_ref.video_id.get_raw(), "test123");
        assert_eq!(song_ref.title, "Loaded Song");
        assert_eq!(song_ref.artists.len(), 2);
        assert_eq!(song_ref.artists[0], "Artist X");
        assert_eq!(song_ref.album, Some("Album Name".to_string()));
        assert_eq!(song_ref.duration_string, "5:00");
        assert!(song_ref.thumbnail_url.is_none());
        assert_eq!(song_ref.like_status, LikeStatus::Liked);
    }

    #[test]
    fn test_compact_song_ref_deserialization_backwards_compat() {
        let json = r#"{
            "video_id": "old123",
            "title": "Old Song",
            "artists": ["Old Artist"],
            "album": null,
            "duration_string": "3:00",
            "thumbnail_url": null
        }"#;

        let song_ref: CompactSongRef = serde_json::from_str(json).unwrap();
        assert_eq!(song_ref.like_status, LikeStatus::Indifferent);
    }

    #[test]
    fn test_compact_format_json_structure() {
        let song_ref = CompactSongRef {
            video_id: VideoID::from_raw("v123"),
            title: "Compact Song".to_string(),
            artists: vec!["Solo Artist".to_string()],
            album: Some("Album Title".to_string()),
            duration_string: "3:33".to_string(),
            thumbnail_url: Some("https://example.com/img.jpg".to_string()),
            like_status: LikeStatus::Indifferent,
        };

        let json = serde_json::to_string(&song_ref).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Verify structure has only compact fields
        let keys: Vec<_> = parsed.as_object().unwrap().keys().collect();
        let expected_keys = vec![
            "video_id",
            "title",
            "artists",
            "album",
            "duration_string",
            "thumbnail_url",
            "like_status",
        ];
        for key in expected_keys {
            assert!(keys.iter().any(|k| *k == key));
        }

        // Verify no heavy fields
        let excluded_keys = vec!["thumbnails", "album_art", "artists_string"];
        for key in excluded_keys {
            assert!(!keys.iter().any(|k| *k == key));
        }
    }

    #[test]
    fn test_artists_serialization_format() {
        let song_ref = CompactSongRef {
            video_id: VideoID::from_raw("multi"),
            title: "Multi Artist Song".to_string(),
            artists: vec![
                "First".to_string(),
                "Second".to_string(),
                "Third".to_string(),
            ],
            album: None,
            duration_string: "4:00".to_string(),
            thumbnail_url: None,
            like_status: LikeStatus::Indifferent,
        };

        let json = serde_json::to_string(&song_ref).unwrap();
        // Artists should be a JSON array
        assert!(json.contains(r#""artists":["First","Second","Third"]"#));
    }
}
