use crate::app::structures::ListSong;
use crate::app::ui::playlist::Playlist;
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use ytmapi_rs::common::VideoID;

const QUEUE_DIR: &str = "youtui/queues";
const AUTO_SAVE: &str = "__autosave";

#[derive(Serialize, Deserialize)]
struct SavedQueue {
    songs: Vec<ListSong>,
    current_index: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MinimalSongRef {
    pub video_id: VideoID<'static>,
}

#[derive(Serialize, Deserialize)]
struct MinimalSavedQueue {
    songs: Vec<MinimalSongRef>,
    current_index: Option<usize>,
}

pub fn get_queue_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(QUEUE_DIR);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn save_queue(playlist: &Playlist, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let raw_songs: Vec<ListSong> = playlist.list.get_list_iter().cloned().collect();

    let songs: Vec<MinimalSongRef> = raw_songs
        .iter()
        .map(|song| MinimalSongRef {
            video_id: song.video_id.clone(),
        })
        .collect();

    let current_idx = playlist.get_cur_playing_index();
    let saved = MinimalSavedQueue {
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

    if let Ok(saved) = serde_json::from_str::<MinimalSavedQueue>(&json) {
        load_minimal_queue(playlist, saved)?;
    } else {
        warn!("Legacy save format detected - loading with full metadata");
        drop(json);
        load_legacy_format(playlist, name)?;
    }
    Ok(())
}

fn load_minimal_queue(playlist: &mut Playlist, saved: MinimalSavedQueue) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Loaded minimal queue with {} songs", saved.songs.len());
    info!("Clearing playlist (reset)");
    let _ = playlist.reset();
    
    if !saved.songs.is_empty() {
        let songs: Vec<ListSong> = saved.songs
            .iter()
            .map(|ref_| {
                ListSong::create_placeholder(ref_.video_id.clone())
            })
            .collect();
        
        info!("Created {} placeholder songs (metadata will refresh on API fetch)", songs.len());
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

fn load_legacy_format(playlist: &mut Playlist, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_queue_dir()?.join(format!("{}.json", name));
    let json = fs::read_to_string(&path)?;
    let saved: SavedQueue = serde_json::from_str(&json)?;
    debug!("Parsed {} songs from legacy JSON", saved.songs.len());
    info!("Clearing playlist (reset)");
    let _ = playlist.reset();
    if !saved.songs.is_empty() {
        info!("Loading {} songs into playlist (legacy format)", saved.songs.len());
        let (_first_id, _effect) = playlist.push_song_list(saved.songs);
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

pub fn list_queues() -> Vec<String> {
    let Ok(queue_dir) = get_queue_dir() else {
        return Vec::new();
    };
    let Ok(dir) = fs::read_dir(queue_dir) else {
        return Vec::new();
    };
    dir.filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") && !name.starts_with("__") {
                Some(name.trim_end_matches(".json").to_string())
            } else {
                None
            }
        })
        .collect()
}

pub fn delete_queue(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_queue_dir()?.join(format!("{}.json", name));
    fs::remove_file(path)?;
    Ok(())
}

pub fn auto_save(playlist: &Playlist) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Auto-saving queue");
    save_queue(playlist, AUTO_SAVE)
}

pub fn auto_load(playlist: &mut Playlist) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Auto-loading queue");
    load_queue(playlist, AUTO_SAVE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ytmapi_rs::common::YoutubeID;

    #[test]
    fn test_minimal_song_ref_serialization() {
        let video_id = VideoID::from_raw("abc123");
        let song_ref = MinimalSongRef {
            video_id: video_id.clone(),
        };
        let json = serde_json::to_string_pretty(&song_ref).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("video_id"));
    }
}