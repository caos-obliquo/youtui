use super::appevent::{AppEvent, EventHandler};
use crate::config::{ApiKey, Config};
use crate::core::get_limited_sequential_file;
use crate::{RuntimeInfo, get_data_dir};
use anyhow::{Context, Result, bail};
use async_callback_manager::{AsyncCallbackManager, AsyncTask, TaskOutcome};
use component::actionhandler::YoutuiEffect;
use tracing::warn;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use media_controls::MediaController;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::Picker;
use server::{ArcServer, Server, TaskMetadata, AddSongsToPlaylist, GetPlaylistTracks, CreatePlaylistWithVideos};
use std::borrow::Cow;
use std::time::Duration;
use ytmapi_rs::common::{PlaylistID, VideoID, ArtistChannelID};

#[derive(Debug)]
pub enum NavTarget {
    Artist(String),
    ArtistChannel(ArtistChannelID<'static>),
    Album { artist: String, album: String },
    SongSearch(String),
}
use std::fmt::Display;
use std::io;
use std::sync::Arc;
pub use structures::AudioQuality;
use structures::{ListSong, ListSongID};
use tracing::{debug, error, info};
use tracing_subscriber::prelude::*;
use ui::{
    WindowContext, YoutuiWindow,
    playlist::effect_handlers_playlist::{
        HandleAddSongsOk, HandleAddSongsError,
        HandleGetPlaylistTracksAppendOk, HandleGetPlaylistTracksOk, HandleGetPlaylistTracksErr,
        HandleCreatePlaylistOk, HandleCreatePlaylistError,
    },
};

#[macro_use]
pub mod component;
mod media_controls;
pub mod server;
mod structures;
pub mod queue_persistence;
pub mod scrobbler;
pub mod ui;
pub mod view;

// We need this thread_local to ensure we know which is the main thread. Panic
// hook that destructs terminal should only run on the main thread.
thread_local! {
    static IS_MAIN_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const CALLBACK_CHANNEL_SIZE: usize = 64;
const EVENT_CHANNEL_SIZE: usize = 256;
const LOG_FILE_NAME: &str = "debug";
const LOG_FILE_EXT: &str = "log";
const MAX_LOG_FILES: u16 = 5;

pub struct Youtui {
    status: AppStatus,
    event_handler: EventHandler,
    window_state: YoutuiWindow,
    task_manager: AsyncCallbackManager<YoutuiWindow, ArcServer, TaskMetadata>,
    server: Arc<Server>,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    // Optional as may be disabled at runtime.
    media_controls: Option<MediaController>,
    /// Capabilities of the user's terminal in regards to image rendering - ie,
    /// font size / kitty protocal etc. This
    terminal_image_capabilities: Picker,
    rescrobbled_process: Option<tokio::process::Child>,
}

#[derive(PartialEq)]
pub enum AppStatus {
    Running,
    // Cow: Message
    Exiting(Cow<'static, str>),
}

// A callback from one of the application components to the top level.
#[derive(Debug)]
#[must_use]
pub enum AppCallback {
    Quit,
    ChangeContext(WindowContext),
    AddSongsToPlaylist(Vec<ListSong>),
    AddSongsToPlaylistAndPlay(Vec<ListSong>),
    #[allow(dead_code)]
    OpenPlaylistSavePopup(Vec<VideoID<'static>>),
    #[allow(dead_code)]
    OpenPlaylistUpdatePopup(Vec<VideoID<'static>>),
    AddVideosToPlaylistFromPopup {
        playlist_id: PlaylistID<'static>,
        video_ids: Vec<VideoID<'static>>,
        overwrite: bool,
    },
    ViewLyrics {
        artist: String,
        title: String,
    },
    ViewSongInfo {
        song: ListSong,
    },
    ViewAlbumCover {
        thumbnail: std::rc::Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>,
    },
    UpdateSongInfo {
        id: ListSongID,
        song: ListSong,
    },
    ClosePopup,
    LoadPlaylistFromPopup(PlaylistID<'static>),
    AppendPlaylistFromPopup(PlaylistID<'static>),
    CreatePlaylistFromPopup {
        title: String,
        description: Option<String>,
        video_ids: Vec<VideoID<'static>>,
    },
    Navigate(NavTarget),
    SeekBack,
    SeekForward,
    SeekTo(Duration),
    ViewNextInQueue,
    ViewPrevInQueue,
    PlayNext,
    PlayPrev,
    ReloadConfig,
    InsertNext(Vec<ListSong>),
    OpenPlaylistEditor {
        playlist_id: ytmapi_rs::common::PlaylistID<'static>,
        playlist_title: String,
        tracks: Vec<crate::app::structures::ListSong>,
    },
    #[allow(dead_code)]
    Back,
}

impl Youtui {
    pub async fn new(rt: RuntimeInfo) -> Result<Youtui> {
        let RuntimeInfo {
            api_key,
            debug,
            po_token,
            cookie_path,
            config,
            disable_media_controls,
            url,
        } = rt;
        // Setup tracing and link to tui_logger.
        // NOTE: File logging is always enabled for now - I can't think of a use case
        // where we wouldn't want this.
        init_tracing(debug, true).await?;
        match debug {
            true => info!("Starting in debug mode"),
            false => info!("Starting"),
        }
        // Youtui is not designed to try to bypass youtube music advertising.
        // Authentication is required to use it.
        if let ApiKey::None = api_key {
            bail!("Authentication is required to run youtui");
        }
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture,)?;
        // By only performing panic cleanup from the main thread, this largely prevents
        // exits that occur part-way through a redraw.
        IS_MAIN_THREAD.with(|flag| flag.set(true));
        std::panic::set_hook(Box::new(|panic_info| {
            if IS_MAIN_THREAD.with(|flag| flag.get()) {
                tracing::error!(
                    "Panic detected on main thread. \
                     Message: {panic_info}"
                );
                // If we fail to exit cleanly, ignore the error as panicking anyway.
                let _ = cleanup_tui_and_print_panic_message(&panic_info);
            } else {
                tracing::warn!(
                    "Panic detected outside main thread - \
                     this is not necessarily an error but may indicate one. \
                     Message: {panic_info}"
                );
            }
        }));
        // Setup components
        let mut task_manager = async_callback_manager::AsyncCallbackManager::new()
            .with_on_task_spawn_callback(|task| {
                info!(
                    "Received task {:?}: type_id: {:?},  constraint: {:?}",
                    task.type_debug, task.type_id, task.constraint
                )
            });
        let server = Arc::new(server::Server::new(api_key, po_token, cookie_path.clone(), &config));
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        // The docs for this function state that it must be run after entering alternate
        // screen but before events are read, therefore this is hoisted for
        // visibility. Note that this may briefly block, delaying startup, but likely
        // unavoidable.
        let terminal_image_capabilities = Picker::from_query_stdio()?;
        debug!("Terminal info: {terminal_image_capabilities:#?}");
        let (media_controls, media_control_event_stream) = if disable_media_controls {
            (None, None)
        } else {
            let (media_controls, media_control_event_stream) = MediaController::new().context(
                "Unable to initialise media controls - is the application already running?",
            )?;
            (Some(media_controls), Some(media_control_event_stream))
        };
        let event_handler = EventHandler::new(EVENT_CHANNEL_SIZE, media_control_event_stream)?;
        let rescrobbled_process = Self::spawn_rescrobbled(&config);
        let (window_state, effect) = YoutuiWindow::new(config, cookie_path, url);
        // Even the creation of a YoutuiWindow causes an effect. We'll spawn it straight
        // away.
        task_manager.spawn_task(&server, effect);
        Ok(Youtui {
            status: AppStatus::Running,
            event_handler,
            window_state,
            task_manager,
            server,
            terminal,
            media_controls,
            terminal_image_capabilities,
            rescrobbled_process,
        })
    }
    fn spawn_rescrobbled(config: &crate::config::Config) -> Option<tokio::process::Child> {
        if !config.scrobbling.enabled {
            return None;
        }
        match tokio::process::Command::new("rescrobbled")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn() {
            Ok(child) => {
                tracing::info!("Rescrobbled spawned successfully");
                Some(child)
            }
            Err(e) => {
                tracing::warn!("Failed to spawn rescrobbled: {}", e);
                None
            }
        }
    }
    pub async fn run(&mut self) -> Result<()> {
        // Initial draw before first event
        self.terminal.draw(|f| {
            ui::draw::draw_app(
                f,
                &mut self.window_state,
                &self.terminal_image_capabilities,
            );
        })?;
        if let Some(media_controls) = &mut self.media_controls {
            media_controls.update_controls(
                ui::draw_media_controls::draw_app_media_controls(&self.window_state),
            )?;
        }
        loop {
            match &self.status {
                AppStatus::Running => {
                    tokio::select! {
                        Some(event) = self.event_handler.next() => {
                            self.handle_event(event).await;
                        }
                        Some(outcome) = self.task_manager.get_next_response() => {
                            self.handle_effect(outcome);
                        }
                    }
                    self.terminal.draw(|f| {
                        ui::draw::draw_app(
                            f,
                            &mut self.window_state,
                            &self.terminal_image_capabilities,
                        );
                    })?;
                    if let Some(media_controls) = &mut self.media_controls {
                        media_controls.update_controls(
                            ui::draw_media_controls::draw_app_media_controls(&self.window_state),
                        )?;
                    }
                }
                AppStatus::Exiting(s) => {
                    if let Some(mut child) = self.rescrobbled_process.take() {
                        let _ = child.start_kill();
                        tracing::info!("Rescrobbled stopped");
                    }
                    destruct_terminal()?;
                    println!("{s}");
                    break;
                }
            }
        }
        Ok(())
    }
    fn handle_effect(&mut self, effect: TaskOutcome<YoutuiWindow, ArcServer, TaskMetadata>) {
        match effect {
            async_callback_manager::TaskOutcome::StreamFinished {
                type_id,
                type_debug,
                task_id,
                ..
            } => {
                info!(
                    "Stream task {:?}: type_id: {:?}, task_id: {:?} finished",
                    type_debug, type_id, task_id
                );
            }
            async_callback_manager::TaskOutcome::TaskPanicked {
                type_debug, error, ..
            }
            | async_callback_manager::TaskOutcome::StreamPanicked {
                type_debug, error, ..
            } => {
                error!("Task {type_debug} panicked!");
                // We are about to panic - ignore terminal destruction error.
                let _ = cleanup_tui_and_print_panic_message(&error);
                std::panic::resume_unwind(error.into_panic());
            }
            async_callback_manager::TaskOutcome::MutationReceived {
                mutation,
                type_id,
                type_debug,
                task_id,
                ..
            } => {
                info!(
                    "Received response to {:?}: type_id: {:?}, task_id: {:?}",
                    type_debug, type_id, task_id
                );
                let next_task = mutation(&mut self.window_state);
                self.task_manager.spawn_task(&self.server, next_task);
            }
        }
    }
    async fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => self.window_state.handle_tick().await,
            AppEvent::Crossterm(e) => {
                let YoutuiEffect { effect, callback } =
                    self.window_state.handle_crossterm_event(e).await;
                self.task_manager.spawn_task(&self.server, effect);
                if let Some(callback) = callback {
                    self.handle_callback(callback);
                }
            }
            AppEvent::MediaControls(e) => {
                let YoutuiEffect { effect, callback } =
                    self.window_state.handle_media_controls_event(e).await;
                self.task_manager.spawn_task(&self.server, effect);
                if let Some(callback) = callback {
                    self.handle_callback(callback);
                }
            }
            AppEvent::QuitSignal => self.status = AppStatus::Exiting("Quit signal received".into()),
        }
    }
    pub fn handle_callback(&mut self, callback: AppCallback) {
        match callback {
            AppCallback::Quit => self.status = AppStatus::Exiting("Quitting".into()),
            AppCallback::ChangeContext(context) => self.window_state.handle_change_context(context),
            AppCallback::AddSongsToPlaylist(song_list) => self.task_manager.spawn_task(
                &self.server,
                self.window_state.handle_add_songs_to_playlist(song_list),
            ),
            AppCallback::AddSongsToPlaylistAndPlay(song_list) => self.task_manager.spawn_task(
                &self.server,
                self.window_state
                    .handle_add_songs_to_playlist_and_play(song_list),
            ),
            AppCallback::InsertNext(song_list) => self.task_manager.spawn_task(
                &self.server,
                self.window_state.handle_insert_next(song_list),
            ),
            AppCallback::OpenPlaylistSavePopup(video_ids) => {
                self.window_state.open_playlist_save_popup(video_ids);
            }
            AppCallback::OpenPlaylistUpdatePopup(video_ids) => {
                let effect = self.window_state.open_playlist_update_popup(video_ids);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::AddVideosToPlaylistFromPopup {
                playlist_id,
                video_ids,
                overwrite,
            } => {
                self.window_state.close_popup();
                let add_effect = AsyncTask::new_future_try(
                    AddSongsToPlaylist {
                        playlist_id: playlist_id.clone(),
                        video_ids,
                    },
                    HandleAddSongsOk,
                    HandleAddSongsError,
                    None,
                )
                .map_frontend(|window: &mut YoutuiWindow| &mut window.playlist);

                let effect = add_effect;

                if overwrite {
                    // Overwrite: remove existing tracks first, then add new ones
                    // TODO: Fetch current playlist tracks via GetPlaylistTracks + RemovePlaylistItems
                    // For now, append-only (overwrite flag tracked for future implementation)
                    info!("Overwrite mode selected — will replace playlist tracks in future implementation");
                }

                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::ViewLyrics { artist, title } => {
                let effect = self.window_state.open_lyrics_popup(artist, title);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::ViewSongInfo { song } => {
                let effect = self.window_state.open_song_info_popup(song);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::ViewAlbumCover { thumbnail } => {
                self.window_state.album_art_popup = Some(ui::playlist::album_art_popup::AlbumArtPopup::new(thumbnail));
            }
            AppCallback::UpdateSongInfo { id, song } => {
                let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                let meta = crate::app::server::ValidatedMetadata {
                    artist: Some(artist.clone()),
                    album: song.album.as_ref().map(|a| a.name.clone()),
                    year: song.year.as_ref().map(|y| y.as_str().to_string()),
                    track_no: song.track_no,
                    album_tracks: Vec::new(),
                    genres: song.genres.clone(),
                    styles: song.styles.clone(),
                };
                self.server.metadata_registry.save_override(&artist, &song.title, &meta);
                self.window_state.playlist.update_song_info(id, song);
                self.window_state.close_popup();
            }
            AppCallback::LoadPlaylistFromPopup(playlist_id) => {
                self.window_state.close_popup();
                let effect = AsyncTask::new_future_try(
                    GetPlaylistTracks(playlist_id),
                    HandleGetPlaylistTracksOk,
                    HandleGetPlaylistTracksErr,
                    None,
                )
                .map_frontend(|window: &mut YoutuiWindow| &mut window.playlist);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::AppendPlaylistFromPopup(playlist_id) => {
                let effect = AsyncTask::new_future_try(
                    GetPlaylistTracks(playlist_id),
                    HandleGetPlaylistTracksAppendOk,
                    HandleGetPlaylistTracksErr,
                    None,
                )
                .map_frontend(|window: &mut YoutuiWindow| &mut window.playlist);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::ClosePopup => {
                self.window_state.close_popup();
            }
            AppCallback::CreatePlaylistFromPopup {
                title,
                description,
                video_ids,
            } => {
                self.window_state.close_popup();
                const MAX_YTM_SONGS: usize = 5000;
                let total = video_ids.len();
                if total > MAX_YTM_SONGS {
                    let chunks: Vec<Vec<VideoID<'static>>> = video_ids.chunks(MAX_YTM_SONGS).map(|c| c.to_vec()).collect();
                    let chunks_needed = chunks.len();
                    info!("Splitting {} songs into {} playlists", total, chunks_needed);
                    // Store remaining chunks (skip first, it's about to be spawned)
                    self.window_state.playlist.pending_playlist_chunks = Some((chunks[1..].to_vec(), title.clone(), description.clone()));
                    // Spawn first chunk
                    let first_title = format!("{} ({}/{})", title, 1, chunks_needed);
                    let effect = AsyncTask::new_future_try(
                        CreatePlaylistWithVideos {
                            title: first_title,
                            description: description.clone(),
                            video_ids: chunks[0].clone(),
                        },
                        HandleCreatePlaylistOk,
                        HandleCreatePlaylistError,
                        None,
                    )
                    .map_frontend(|window: &mut YoutuiWindow| &mut window.playlist);
                    self.task_manager.spawn_task(&self.server, effect);
                } else {
                    let effect = self.window_state.handle_create_playlist_from_popup(
                        title,
                        description,
                        video_ids,
                    );
                    self.task_manager.spawn_task(&self.server, effect);
                }
            }
            AppCallback::PlayNext => {
                let effect = self.window_state.handle_next();
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::PlayPrev => {
                let effect = self.window_state.handle_prev();
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::SeekBack => {
                use crate::async_rodio_sink::SeekDirection;
                let effect = self.window_state.playlist.handle_seek(
                    Duration::from_secs(5),
                    SeekDirection::Back,
                ).map_frontend(|window: &mut YoutuiWindow| &mut window.playlist);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::SeekForward => {
                use crate::async_rodio_sink::SeekDirection;
                let effect = self.window_state.playlist.handle_seek(
                    Duration::from_secs(5),
                    SeekDirection::Forward,
                ).map_frontend(|window: &mut YoutuiWindow| &mut window.playlist);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::Navigate(target) => {
                self.window_state.context = WindowContext::Browser;
                if let Some(task) = self.window_state.browser.navigate_to(target) {
                    let task = task.map_frontend(|window: &mut YoutuiWindow| &mut window.browser);
                    self.task_manager.spawn_task(&self.server, task);
                }
            }
            AppCallback::SeekTo(pos) => {
                let effect = self.window_state.playlist.handle_seek_to(pos)
                    .map_frontend(|window: &mut YoutuiWindow| &mut window.playlist);
                self.task_manager.spawn_task(&self.server, effect);
            }
            AppCallback::ViewNextInQueue => {
                let songs: Vec<_> = self.window_state.playlist.list.get_list_iter().collect();
                let start_idx = self.window_state.lyrics_viewing_idx
                    .or_else(|| {
                        use crate::app::structures::PlayState;
                        match &self.window_state.playlist.play_status {
                            PlayState::Playing(id) | PlayState::Paused(id) | PlayState::Buffering(id) => {
                                songs.iter().position(|s| s.id == *id)
                            }
                            _ => None,
                        }
                    });
                if let Some(pos) = start_idx {
                    let target_idx = pos.saturating_add(1).min(songs.len().saturating_sub(1));
                    if let Some(song) = songs.get(target_idx) {
                        let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                        self.window_state.lyrics_viewing_idx = Some(target_idx);
                        let effect = self.window_state.open_lyrics_popup(artist, song.title.clone());
                        self.task_manager.spawn_task(&self.server, effect);
                    }
                }
            }
            AppCallback::ViewPrevInQueue => {
                let songs: Vec<_> = self.window_state.playlist.list.get_list_iter().collect();
                let start_idx = self.window_state.lyrics_viewing_idx
                    .or_else(|| {
                        use crate::app::structures::PlayState;
                        match &self.window_state.playlist.play_status {
                            PlayState::Playing(id) | PlayState::Paused(id) | PlayState::Buffering(id) => {
                                songs.iter().position(|s| s.id == *id)
                            }
                            _ => None,
                        }
                    });
                if let Some(pos) = start_idx {
                    let target_idx = pos.saturating_sub(1);
                    if let Some(song) = songs.get(target_idx) {
                        let artist = song.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
                        self.window_state.lyrics_viewing_idx = Some(target_idx);
                        let effect = self.window_state.open_lyrics_popup(artist, song.title.clone());
                        self.task_manager.spawn_task(&self.server, effect);
                    }
                }
            }
            AppCallback::OpenPlaylistEditor { playlist_id, playlist_title, tracks } => {
                use crate::app::ui::playlist::playlist_editor_popup::PlaylistEditorPopup;
                self.window_state.playlist_editor_popup = Some(PlaylistEditorPopup::new(playlist_id, playlist_title, tracks));
                self.window_state.context = WindowContext::PlaylistEditor;
            }
            AppCallback::ReloadConfig => {
                let config_dir = crate::get_config_dir().ok();
                let config_path = config_dir.map(|d| d.join("config.toml")).unwrap_or_else(|| std::path::PathBuf::from("config.toml"));
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => {
                        match toml::from_str::<crate::config::ConfigIR>(&content) {
                            Ok(ir) => {
                                match Config::try_from(ir) {
                                    Ok(new_config) => {
                                        info!("Config reloaded from {:?}", config_path);
                                        self.window_state.config = new_config;
                                    }
                                    Err(e) => warn!("Failed to build config: {}", e),
                                }
                            }
                            Err(e) => warn!("Failed to parse config: {}", e),
                        }
                    }
                    Err(e) => warn!("Failed to read config: {}", e),
                }
            }
            AppCallback::Back => {
                self.window_state.browser.navigate_back();
            }
        }
    }
}

/// When panicking in the tui, terminal cleanup and error message must be in the
/// correct order.
fn cleanup_tui_and_print_panic_message(panic: &impl Display) -> Result<()> {
    destruct_terminal()?;
    println!("{panic}");
    Ok(())
}

/// Cleanly exit the tui
fn destruct_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::cursor::Show
    )?;
    Ok(())
}

/// Initialise tracing and subscribers such as tuilogger and file logging.
/// # Panics
/// If tracing fails to initialise, function will panic
async fn init_tracing(debug: bool, logging: bool) -> Result<()> {
    let tui_logger_layer = tui_logger::TuiTracingSubscriberLayer;
    let (tracing_log_level, tui_logger_log_level) = if debug {
        (tracing::Level::DEBUG, tui_logger::LevelFilter::Debug)
    } else {
        (tracing::Level::INFO, tui_logger::LevelFilter::Info)
    };
    let context_layer =
        tracing_subscriber::filter::Targets::new().with_target("youtui", tracing_log_level);
    if logging {
        let (log_file, log_file_name) = get_limited_sequential_file(
            &get_data_dir()?,
            LOG_FILE_NAME,
            LOG_FILE_EXT,
            MAX_LOG_FILES,
        )
        .await?;
        let log_file_layer = tracing_subscriber::fmt::layer().with_writer(Arc::new(
            log_file
                .try_into_std()
                .expect("No file operation should be in-flight yet"),
        ));
        tracing_subscriber::registry()
            .with(tui_logger_layer.and_then(log_file_layer))
            .with(context_layer)
            .init();
        info!("Logging to {:?}.", log_file_name);
    } else {
        let context_layer =
            tracing_subscriber::filter::Targets::new().with_target("youtui", tracing_log_level);
        tracing_subscriber::registry()
            .with(tui_logger_layer)
            .with(context_layer)
            .init();
    }
    tui_logger::init_logger(tui_logger_log_level)
        .expect("Expected logger to initialise succesfully");
    Ok(())
}
