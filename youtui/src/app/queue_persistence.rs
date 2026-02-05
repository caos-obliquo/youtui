use crate::app::structures::ListSong;
use crate::app::ui::playlist::Playlist;
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

const QUEUE_DIR: &str = "youtui/queues";
const AUTO_SAVE: &str = "__autosave";

#[derive(Serialize, Deserialize)]
struct SavedQueue {
    songs: Vec<ListSong>,
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
    use crate::app::structures::{AlbumArtState, DownloadStatus};

    let raw_songs: Vec<ListSong> = playlist.list.get_list_iter().cloned().collect();

    // Strip unserializable data - songs will re-download/fetch art on restore
    let songs: Vec<ListSong> = raw_songs
        .into_iter()
        .map(|song| ListSong {
            download_status: match song.download_status {
                DownloadStatus::Downloaded(_) | DownloadStatus::Downloading(_) => {
                    DownloadStatus::None
                }
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

    let queue_dir = get_queue_dir()?;
    let path = queue_dir.join(format!("{}.json", name));
    let temp_path = queue_dir.join(format!("{}.json.tmp", name));

    // Atomic write: write to temp file, then rename
    let json = serde_json::to_string_pretty(&saved)?;
    let mut temp_file = fs::File::create(&temp_path)?;
    temp_file.write_all(json.as_bytes())?;
    temp_file.sync_all()?; // Ensure data is flushed to disk
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
    let saved: SavedQueue = serde_json::from_str(&json)?;
    debug!("Parsed {} songs from JSON", saved.songs.len());
    info!("Clearing playlist (reset)");
    playlist.reset();
    if !saved.songs.is_empty() {
        info!("Loading {} songs into playlist", saved.songs.len());
        let (first_id, _effect) = playlist.push_song_list(saved.songs);
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
