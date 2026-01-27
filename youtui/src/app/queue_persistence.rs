use crate::app::structures::ListSong;
use crate::app::ui::playlist::Playlist;
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info};

const QUEUE_DIR: &str = "youtui/queues";
const AUTO_SAVE: &str = "__autosave";

#[derive(Serialize, Deserialize)]
struct SavedQueue {
    songs: Vec<ListSong>,
    current_index: Option<usize>,
}

pub fn get_queue_dir() -> PathBuf {
    let dir = data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(QUEUE_DIR);
    fs::create_dir_all(&dir).ok();
    dir
}

pub fn save_queue(playlist: &Playlist, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crate::app::structures::{AlbumArtState, DownloadStatus};

    let raw_songs: Vec<ListSong> = playlist.list.get_list_iter().cloned().collect();

    // Strip unserializable data - songs will re-download/fetch art on restore
    let songs: Vec<ListSong> = raw_songs
        .into_iter()
        .map(|song| ListSong {
            download_status: match song.download_status {
                DownloadStatus::Downloaded(_) => DownloadStatus::None,
                other => other,
            },
            album_art: match song.album_art {
                AlbumArtState::Downloaded(_) => AlbumArtState::None,
                other => other,
            },
            ..song
        })
        .collect();

    let current_idx = playlist.get_cur_playing_index();

    let saved = SavedQueue {
        songs,
        current_index: current_idx,
    };

    let path = get_queue_dir().join(format!("{}.json", name));
    let json = serde_json::to_string_pretty(&saved)?;
    fs::write(&path, json)?;
    info!(
        "Successfully saved queue '{}' ({} songs)",
        name,
        saved.songs.len()
    );

    Ok(())
}

pub fn load_queue(playlist: &mut Playlist, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_queue_dir().join(format!("{}.json", name));
    debug!("Loading queue from path: {:?}", path);

    let json = fs::read_to_string(&path)?;
    debug!("Read JSON: {}", json);

    let saved: SavedQueue = serde_json::from_str(&json)?;
    debug!("Parsed {} songs from JSON", saved.songs.len());

    info!("Clearing playlist (reset)");
    playlist.reset();

    if !saved.songs.is_empty() {
        info!("Loading {} songs into playlist", saved.songs.len());
        let (_first_id, _effect) = playlist.push_song_list(saved.songs);
        info!("Load complete");
    } else {
        info!("No songs to load from save file");
    }

    Ok(())
}

pub fn list_queues() -> Vec<String> {
    let Ok(dir) = fs::read_dir(get_queue_dir()) else {
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
    let path = get_queue_dir().join(format!("{}.json", name));
    fs::remove_file(path)?;
    Ok(())
}

pub fn auto_save(playlist: &Playlist) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Auto-saving queue");
    let result = save_queue(playlist, AUTO_SAVE);
    if let Err(ref e) = result {
        error!("Auto-save failed: {}", e);
    } else {
        info!("Auto-save successful");
    }
    result
}

pub fn auto_load(playlist: &mut Playlist) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Auto-loading queue");
    let result = load_queue(playlist, AUTO_SAVE);
    if let Err(ref e) = result {
        error!("Auto-load failed: {}", e);
    } else {
        info!("Auto-load successful");
    }
    result
}
