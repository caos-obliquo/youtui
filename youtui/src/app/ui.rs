use self::browser::{Browser, BrowserAction};
use self::logger::Logger;
use self::playlist::Playlist;
use self::playlist::album_art_popup::AlbumArtPopup;
use self::playlist::config_editor_popup::ConfigEditorPopup;
use self::playlist::playlist_editor_popup::PlaylistEditorPopup;
use self::playlist::lyrics_popup::LyricsPopup;
use self::playlist::song_info_popup::SongInfoPopup;
use vi_text_editor::ViTextEditor;
use std::collections::HashSet;
use std::path::PathBuf;
use self::playlist::playlist_save_popup::PlaylistSavePopup;
use self::playlist::playlist_update_popup::PlaylistUpdatePopup;
use self::playlist::playlist_rename_popup::PlaylistRenamePopup;
use self::playlist::playlist_edit_popup::PlaylistEditPopup;
use self::playlist::playlist_details_popup::PlaylistDetailsPopup;
use ytmapi_rs::common::PlaylistID;
use super::AppCallback;
use super::component::actionhandler::{
    ActionHandler, ComponentEffect, DominantKeyRouter, KeyHandleAction, KeyRouter, Scrollable,
    TextHandler, YoutuiEffect, apply_action_mapped, get_visible_keybinds_as_readable_iter,
    handle_key_stack,
};
use super::server::{IncreaseVolume, SetVolume};
use super::structures::ListSong;
use crate::async_rodio_sink::{SeekDirection, VolumeUpdate};
use crate::config::Config;
use crate::config::keymap::Keymap;
use crate::keyaction::{DisplayableKeyAction, DisplayableMode};
use crate::widgets::ScrollingTableState;
use action::{AppAction, ListAction, PAGE_KEY_LINES, SEEK_AMOUNT, TextEntryAction};
use async_callback_manager::{AsyncTask, Constraint};
use crossterm::event::{Event, KeyCode, KeyEvent};
use itertools::Either;
use std::time::Duration;

pub mod action;
pub mod browser;
pub mod draw;
pub mod draw_media_controls;
mod footer;
mod header;
pub mod logger;
pub mod playlist;

// Which app level keyboard shortcuts function.
// What is displayed in header
// The main pane of the application
// XXX: This is a bit like a route.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowContext {
    Browser,
    Playlist,
    Logs,
    PlaylistSavePopup,
    PlaylistUpdatePopup,
    Lyrics,
    SongInfo,
    PlaylistEditor,
    PlaylistRenamePopup,
    PlaylistEditPopup,
    PlaylistDetailsPopup,
}

pub struct YoutuiWindow {
    pub context: WindowContext,
    pub prev_context: WindowContext,
    pub playlist: Playlist,
    pub browser: Browser,
    pub logger: Logger,
    pub playlist_save_popup: Option<PlaylistSavePopup>,
    pub playlist_update_popup: Option<PlaylistUpdatePopup>,
    pub lyrics_popup: Option<LyricsPopup>,
    pub song_info_popup: Option<SongInfoPopup>,
    pub album_art_popup: Option<AlbumArtPopup>,
    pub config_editor_popup: Option<ConfigEditorPopup>,
    pub playlist_editor_popup: Option<PlaylistEditorPopup>,
    pub playlist_rename_popup: Option<PlaylistRenamePopup>,
    pub playlist_edit_popup: Option<PlaylistEditPopup>,
    pub playlist_details_popup: Option<PlaylistDetailsPopup>,
    pub delete_confirm: Option<(PlaylistID<'static>, String)>,
    pub config: Config,
    pub key_stack: Vec<KeyEvent>,
    pub help: HelpMenu,
    pub tick: u64,
    pub quit_confirm: bool,
    pub command_mode: bool,
    pub command_editor: ViTextEditor,
    pub count_prefix: usize,
    pub lyrics_inflight: HashSet<String>,
    pub lyrics_viewing_idx: Option<usize>,
    pub last_album_art: Option<std::rc::Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>>,
}
impl_youtui_component!(YoutuiWindow);

pub struct HelpMenu {
    pub shown: bool,
    cur: usize,
    len: usize,
    pub widget_state: ScrollingTableState,
}

impl HelpMenu {
    fn new() -> Self {
        HelpMenu {
            shown: Default::default(),
            cur: Default::default(),
            len: Default::default(),
            widget_state: Default::default(),
        }
    }
}
impl_youtui_component!(HelpMenu);

impl Scrollable for HelpMenu {
    fn increment_list(&mut self, amount: isize) {
        self.cur = self
            .cur
            .saturating_add_signed(amount)
            .min(self.len.saturating_sub(1));
    }
    fn is_scrollable(&self) -> bool {
        true
    }
}

impl DominantKeyRouter<AppAction> for YoutuiWindow {
    fn dominant_keybinds_active(&self) -> bool {
        let has_popup = self.playlist_save_popup.is_some() || self.playlist_update_popup.is_some();
        if has_popup {
            return true;
        }
        self.help.shown
            || match self.context {
                WindowContext::Browser => self.browser.dominant_keybinds_active(),
                WindowContext::Playlist => false,
                WindowContext::Logs => false,
                WindowContext::PlaylistSavePopup => true,
                WindowContext::PlaylistUpdatePopup => true,
                WindowContext::Lyrics => true,
                WindowContext::SongInfo => true,
                WindowContext::PlaylistEditor => true,
                WindowContext::PlaylistRenamePopup => true,
                WindowContext::PlaylistEditPopup => true,
                WindowContext::PlaylistDetailsPopup => true,
            }
    }

    fn get_dominant_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        if self.playlist_save_popup.is_some() || self.playlist_update_popup.is_some() {
            return Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            ));
        }
        if self.help.shown {
            return Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            ));
        }
        match self.context {
            WindowContext::Browser => {
                Either::Left(Either::Left(self.browser.get_dominant_keybinds(config)))
            }
            WindowContext::Playlist => {
                Either::Left(Either::Right(self.playlist.get_active_keybinds(config)))
            }
            WindowContext::Logs => {
                Either::Right(Either::Left(self.logger.get_active_keybinds(config)))
            }
            WindowContext::PlaylistSavePopup => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
            WindowContext::PlaylistUpdatePopup => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
            WindowContext::Lyrics => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
            WindowContext::SongInfo => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
            WindowContext::PlaylistEditor => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
            WindowContext::PlaylistRenamePopup => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
            WindowContext::PlaylistEditPopup => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
            WindowContext::PlaylistDetailsPopup => Either::Right(Either::Right(
                [&config.keybinds.help, &config.keybinds.list].into_iter(),
            )),
        }
    }
}

impl Scrollable for YoutuiWindow {
    fn increment_list(&mut self, amount: isize) {
        if self.help.shown {
            return self.help.increment_list(amount);
        }
        match self.context {
            WindowContext::Browser => self.browser.increment_list(amount),
            WindowContext::Playlist => self.playlist.increment_list(amount),
            WindowContext::Logs => (),
            WindowContext::PlaylistSavePopup => (),
            WindowContext::PlaylistUpdatePopup => (),
            WindowContext::Lyrics => (),
            WindowContext::SongInfo => (),
            WindowContext::PlaylistEditor => {
                if let Some(ref mut ed) = self.playlist_editor_popup {
                    let max = ed.tracks.len().saturating_sub(1);
                    let new_cursor = (ed.cursor as isize).saturating_add(amount).clamp(0, max as isize) as usize;
                    ed.cursor = new_cursor;
                    if new_cursor < ed.scroll_offset || new_cursor >= ed.scroll_offset + 20 {
                        ed.scroll_offset = new_cursor.saturating_sub(5);
                    }
                }
            }
            WindowContext::PlaylistRenamePopup => (),
            WindowContext::PlaylistEditPopup => (),
            WindowContext::PlaylistDetailsPopup => (),
        }
    }
    fn is_scrollable(&self) -> bool {
        self.help.shown
            || match self.context {
                WindowContext::Browser => self.browser.is_scrollable(),
                WindowContext::Playlist => self.playlist.is_scrollable(),
                WindowContext::Logs => false,
                WindowContext::PlaylistSavePopup => false,
                WindowContext::PlaylistUpdatePopup => false,
                WindowContext::Lyrics => false,
                WindowContext::SongInfo => false,
                WindowContext::PlaylistEditor => false,
                WindowContext::PlaylistRenamePopup => false,
                WindowContext::PlaylistEditPopup => false,
                WindowContext::PlaylistDetailsPopup => false,
            }
    }
}

impl KeyRouter<AppAction> for YoutuiWindow {
    fn get_active_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        if self.playlist_save_popup.is_some() {
            let kbs = vec![&config.keybinds.playlist_save_popup];
            return kbs.into_iter();
        }
        if self.playlist_update_popup.is_some() {
            let kbs: Vec<&Keymap<AppAction>> = vec![];
            return kbs.into_iter();
        }
        let kb = if self.is_scrollable() {
            Either::Left(std::iter::once(&config.keybinds.list))
        } else {
            Either::Right(std::iter::empty())
        };
        if self.dominant_keybinds_active() {
            let mut v: Vec<&Keymap<AppAction>> = self.get_dominant_keybinds(config).collect();
            v.extend(kb);
            return v.into_iter();
        }
        let kb = kb.chain(std::iter::once(&config.keybinds.global));
        let kb = if self.is_text_handling() {
            Either::Left(kb.chain(std::iter::once(&config.keybinds.text_entry)))
        } else {
            Either::Right(kb)
        };
        match self.context {
            WindowContext::Browser => {
                let mut v: Vec<&Keymap<AppAction>> = kb.collect();
                v.extend(self.browser.get_active_keybinds(config));
                v.into_iter()
            }
            WindowContext::Playlist => {
                let mut v: Vec<&Keymap<AppAction>> = kb.collect();
                v.extend(self.playlist.get_active_keybinds(config));
                v.into_iter()
            }
            WindowContext::Logs => {
                let mut v: Vec<&Keymap<AppAction>> = kb.collect();
                v.extend(self.logger.get_active_keybinds(config));
                v.into_iter()
            }
            WindowContext::PlaylistSavePopup => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
            WindowContext::PlaylistUpdatePopup => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
            WindowContext::Lyrics => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
            WindowContext::SongInfo => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
            WindowContext::PlaylistEditor => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
            WindowContext::PlaylistRenamePopup => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
            WindowContext::PlaylistEditPopup => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
            WindowContext::PlaylistDetailsPopup => {
                let v: Vec<&Keymap<AppAction>> = kb.collect();
                v.into_iter()
            }
        }
    }
    fn get_all_keybinds<'a>(
        &self,
        config: &'a Config,
    ) -> impl Iterator<Item = &'a Keymap<AppAction>> + 'a {
        std::iter::once(&config.keybinds.global)
            .chain(self.browser.get_all_keybinds(config))
            .chain(self.playlist.get_all_keybinds(config))
            .chain(self.logger.get_all_keybinds(config))
    }
}

impl TextHandler for YoutuiWindow {
    fn is_text_handling(&self) -> bool {
        if self.playlist_save_popup.is_some() || self.playlist_update_popup.is_some() {
            return false;
        }
        if self.help.shown {
            return false;
        }
        match self.context {
            WindowContext::Browser => self.browser.is_text_handling(),
            WindowContext::Playlist => self.playlist.is_text_handling(),
            WindowContext::Logs => self.logger.is_text_handling(),
            WindowContext::PlaylistSavePopup => false,
            WindowContext::PlaylistUpdatePopup => false,
            WindowContext::Lyrics => false,
            WindowContext::SongInfo => false,
            WindowContext::PlaylistEditor => false,
            WindowContext::PlaylistRenamePopup => false,
            WindowContext::PlaylistEditPopup => false,
            WindowContext::PlaylistDetailsPopup => false,
        }
    }
    fn get_text(&self) -> std::option::Option<&str> {
        match self.context {
            WindowContext::Browser => self.browser.get_text(),
            WindowContext::Playlist => self.playlist.get_text(),
            WindowContext::Logs => self.logger.get_text(),
            WindowContext::PlaylistSavePopup => None,
            WindowContext::PlaylistUpdatePopup => None,
            WindowContext::Lyrics => None,
            WindowContext::SongInfo => None,
            WindowContext::PlaylistEditor => None,
            WindowContext::PlaylistRenamePopup => None,
            WindowContext::PlaylistEditPopup => None,
            WindowContext::PlaylistDetailsPopup => None,
        }
    }
    fn replace_text(&mut self, text: impl Into<String>) {
        match self.context {
            WindowContext::Browser => self.browser.replace_text(text),
            WindowContext::Playlist => self.playlist.replace_text(text),
            WindowContext::Logs => self.logger.replace_text(text),
            WindowContext::PlaylistSavePopup => {}
            WindowContext::PlaylistUpdatePopup => {}
            WindowContext::Lyrics => {}
            WindowContext::SongInfo => {}
            WindowContext::PlaylistEditor => {}
            WindowContext::PlaylistRenamePopup => {}
            WindowContext::PlaylistEditPopup => {}
            WindowContext::PlaylistDetailsPopup => {}
        }
    }
    fn clear_text(&mut self) -> bool {
        match self.context {
            WindowContext::Browser => self.browser.clear_text(),
            WindowContext::Playlist => self.playlist.clear_text(),
            WindowContext::Logs => self.logger.clear_text(),
            WindowContext::PlaylistSavePopup => false,
            WindowContext::PlaylistUpdatePopup => false,
            WindowContext::Lyrics => false,
            WindowContext::SongInfo => false,
            WindowContext::PlaylistEditor => false,
            WindowContext::PlaylistRenamePopup => false,
            WindowContext::PlaylistEditPopup => false,
            WindowContext::PlaylistDetailsPopup => false,
        }
    }
    fn handle_text_event_impl(&mut self, event: &Event) -> Option<ComponentEffect<Self>> {
        match self.context {
            WindowContext::Browser => self
                .browser
                .handle_text_event_impl(event)
                .map(|effect| effect.map_frontend(|this: &mut YoutuiWindow| &mut this.browser)),
            WindowContext::Playlist => self
                .playlist
                .handle_text_event_impl(event)
                .map(|effect| effect.map_frontend(|this: &mut YoutuiWindow| &mut this.playlist)),
            WindowContext::Logs => self
                .logger
                .handle_text_event_impl(event)
                .map(|effect| effect.map_frontend(|this: &mut YoutuiWindow| &mut this.logger)),
            WindowContext::PlaylistSavePopup => None,
            WindowContext::PlaylistUpdatePopup => None,
            WindowContext::Lyrics => None,
            WindowContext::SongInfo => None,
            WindowContext::PlaylistEditor => None,
            WindowContext::PlaylistRenamePopup => None,
            WindowContext::PlaylistEditPopup => None,
            WindowContext::PlaylistDetailsPopup => None,
        }
    }
}

impl ActionHandler<AppAction> for YoutuiWindow {
    fn apply_action(&mut self, action: AppAction) -> impl Into<YoutuiEffect<Self>> {
        match action {
            AppAction::VolUp => {
                return Into::<YoutuiEffect<Self>>::into(self.handle_increase_volume(5));
            }
            AppAction::VolDown => return self.handle_increase_volume(-5).into(),
            AppAction::NextSong => return self.handle_next().into(),
            AppAction::PrevSong => return self.handle_prev().into(),
            AppAction::SeekForward => {
                return self.handle_seek(SEEK_AMOUNT, SeekDirection::Forward).into();
            }
            AppAction::SeekBack => {
                return self.handle_seek(SEEK_AMOUNT, SeekDirection::Back).into();
            }
            AppAction::ToggleHelp => self.toggle_help(),
            AppAction::Quit => {
                self.quit_confirm = true;
                return AsyncTask::new_no_op().into();
            }
            AppAction::ViewLogs => self.handle_change_context(WindowContext::Logs),
            AppAction::PlayPause => return self.pauseplay().into(),
            AppAction::Log(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.logger);
            }
            AppAction::Playlist(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.playlist);
            }
            AppAction::Help(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.help);
            }
            AppAction::Browser(a) => {
                if a == BrowserAction::Search && self.context != WindowContext::Browser {
                    // F1 from non-Browser: switch to Browser and open search
                    self.prev_context = self.context;
                    self.context = WindowContext::Browser;
                    self.browser.handle_toggle_search();
                } else {
                    return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
                }
            }
            AppAction::Filter(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
            }
            AppAction::Sort(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
            }
            AppAction::BrowserArtists(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
            }
            AppAction::BrowserSearch(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
            }
            AppAction::BrowserArtistSongs(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
            }
            AppAction::BrowserSongs(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
            }
            #[allow(dead_code)]
            AppAction::BrowserPlaylists(_) => {
                tracing::warn!("BrowserPlaylists action is deprecated, no-op");
            }
            #[allow(dead_code)]
            AppAction::BrowserPlaylistSongs(_) => {
                tracing::warn!("BrowserPlaylistSongs action is deprecated, no-op");
            }
            AppAction::BrowserLibrary(a) => {
                return apply_action_mapped(self, a, |this: &mut Self| &mut this.browser);
            }
            AppAction::PlaylistSavePopup(a) => {
                if self.playlist_save_popup.is_some() {
                    return apply_action_mapped(self, a, |this: &mut Self| {
                        this.playlist_save_popup.as_mut().expect("popup exists")
                    });
                }
            }
            AppAction::ConfigEditor(a) => {
                if self.config_editor_popup.is_some() {
                    return apply_action_mapped(self, a, |this: &mut Self| {
                        this.config_editor_popup.as_mut().expect("popup exists")
                    });
                }
            }
            AppAction::LyricsPopup(a) => {
                if self.lyrics_popup.is_some() {
                    return apply_action_mapped(self, a, |this: &mut Self| {
                        this.lyrics_popup.as_mut().expect("popup exists")
                    });
                }
            }
            AppAction::SongInfo(a) => {
                if self.song_info_popup.is_some() {
                    return apply_action_mapped(self, a, |this: &mut Self| {
                        this.song_info_popup.as_mut().expect("popup exists")
                    });
                }
            }
            AppAction::TextEntry(a) => return self.handle_text_entry_action(a).into(),
            AppAction::List(a) => return self.handle_list_action(a).into(),
            AppAction::ToggleBrowser => {
                if self.context == WindowContext::Browser {
                    // Cycle to next browser tab
                    if let Some(task) = self.browser.handle_change_search_type() {
                        return task.map_frontend(|this: &mut Self| &mut this.browser).into();
                    }
                } else {
                    self.prev_context = self.context;
                    self.context = WindowContext::Browser;
                }
                self.dismiss_search();
            }
            AppAction::TogglePlaylist => self.handle_toggle_playlist(),
            AppAction::EditConfig => self.open_config_editor(),
            AppAction::OpenUrl => { self.command_mode = true; self.command_editor.clear(); },
            AppAction::NoOp => (),
        };
        AsyncTask::new_no_op().into()
    }
}

impl YoutuiWindow {
    pub fn new(config: Config, cookie_path: Option<String>, url: Option<String>) -> (YoutuiWindow, ComponentEffect<YoutuiWindow>) {
        let (mut playlist, task) = Playlist::new();
        playlist.set_scrobbling_config(config.scrobbling.clone());
        playlist.yt_dlp_cookie_path = cookie_path;
        let mut this = YoutuiWindow {
            context: WindowContext::Browser,
            prev_context: WindowContext::Browser,
            playlist,
            config,
            browser: Browser::new(),
            logger: Logger::new(),
            playlist_save_popup: None,
            playlist_update_popup: None,
            lyrics_popup: None,
            song_info_popup: None,
            album_art_popup: None,
            config_editor_popup: None,
            playlist_editor_popup: None,
            playlist_rename_popup: None,
            playlist_edit_popup: None,
            playlist_details_popup: None,
            delete_confirm: None,
            key_stack: Vec::new(),
            help: HelpMenu::new(),
            tick: 0,
            quit_confirm: false,
            command_mode: false,
            command_editor: ViTextEditor::new(),
            count_prefix: 0,
            lyrics_inflight: HashSet::new(),
            lyrics_viewing_idx: None,
            last_album_art: None,
        };
        let initial_effect = url.map(|u| this.play_yt_url(u));
        let mut combined = task.map_frontend(|this: &mut Self| &mut this.playlist);
        if let Some(e) = initial_effect {
            combined = combined.push(e);
        }
        (
            this,
            combined,
        )
    }
    pub fn get_help_list_items(&self) -> Vec<DisplayableKeyAction<'_>> {
        let mut items: Vec<DisplayableKeyAction<'_>> = match self.context {
            WindowContext::Browser => {
                get_visible_keybinds_as_readable_iter(self.browser.get_all_keybinds(&self.config)).collect()
            }
            WindowContext::Playlist => {
                get_visible_keybinds_as_readable_iter(self.playlist.get_all_keybinds(&self.config)).collect()
            }
            WindowContext::Logs => {
                get_visible_keybinds_as_readable_iter(self.logger.get_all_keybinds(&self.config)).collect()
            }
            WindowContext::PlaylistSavePopup => {
                get_visible_keybinds_as_readable_iter(
                    std::iter::once(&self.config.keybinds.playlist_save_popup),
                ).collect()
            }
            WindowContext::PlaylistUpdatePopup => {
                get_visible_keybinds_as_readable_iter(
                    std::iter::once(&self.config.keybinds.playlist_save_popup),
                ).collect()
            }
            WindowContext::Lyrics => vec![],
            WindowContext::SongInfo => vec![],
            WindowContext::PlaylistEditor => vec![],
            WindowContext::PlaylistRenamePopup => vec![],
            WindowContext::PlaylistEditPopup => vec![],
            WindowContext::PlaylistDetailsPopup => vec![],
        };
        items.extend(get_visible_keybinds_as_readable_iter(
            std::iter::once(&self.config.keybinds.global)
                .chain(std::iter::once(&self.config.keybinds.list))
                .chain(std::iter::once(&self.config.keybinds.text_entry)),
        ));
        items
    }
    pub async fn handle_crossterm_event(
        &mut self,
        event: crossterm::event::Event,
    ) -> YoutuiEffect<Self> {
        // Config editor popup intercepts events
        if self.config_editor_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.config_editor_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.config_editor_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }

        // Quit confirm screen intercepts all keys
        if self.quit_confirm {
            if let Event::Key(k) = event {
                if k.modifiers == crossterm::event::KeyModifiers::NONE {
                    match k.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            self.quit_confirm = false;
                            return YoutuiEffect {
                                effect: AsyncTask::new_no_op(),
                                callback: Some(AppCallback::Quit),
                            };
                        }
                        KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                            self.quit_confirm = false;
                        }
                        _ => {}
                    }
                }
            }
            return AsyncTask::new_no_op().into();
        }

        // Command mode (: prompt) with vi-mode editor
        if self.command_mode {
            if let Event::Key(k) = event {
                if k.kind == crossterm::event::KeyEventKind::Press {
                    // Esc or Ctrl+C to close command mode without submitting
                    if k.code == crossterm::event::KeyCode::Esc
                        || (k.code == crossterm::event::KeyCode::Char('c')
                            && k.modifiers
                                == crossterm::event::KeyModifiers::CONTROL)
                    {
                        self.command_mode = false;
                        self.command_editor.clear();
                        return AsyncTask::new_no_op().into();
                    }
                    let submitted = self.command_editor.handle_key(k.code, k.modifiers.contains(crossterm::event::KeyModifiers::SHIFT), false);
                    if submitted {
                        let cmd = self.command_editor.get_text().trim().to_string();
                        self.command_mode = false;
                        if !cmd.is_empty() {
                            self.command_editor.push_history(cmd.clone());
                            self.command_editor.clear();
                            if cmd == "reload" || cmd == "reload!" {
                                return YoutuiEffect { effect: AsyncTask::new_no_op(), callback: Some(AppCallback::ReloadConfig) };
                            }
                            if cmd.starts_with("http://") || cmd.starts_with("https://") || cmd.starts_with("youtu") {
                                return self.play_yt_url(cmd).into();
                            }
                            // Treat as raw search query
                            let encoded: String = cmd.split_whitespace().collect::<Vec<_>>().join("+");
                            let search_url = format!("https://music.youtube.com/search?q={}", encoded);
                            return self.play_yt_url(search_url).into();
                        }
                        self.command_editor.clear();
                    }
                }
            }
            return Into::<YoutuiEffect<Self>>::into(AsyncTask::new_no_op());
        }

        // Delete confirm screen intercepts all keys
        if self.delete_confirm.is_some() {
            if let Event::Key(k) = event {
                if k.modifiers == crossterm::event::KeyModifiers::NONE {
                    match k.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            let (pid, _) = self.delete_confirm.take().unwrap();
                            return YoutuiEffect {
                                effect: AsyncTask::new_no_op(),
                                callback: Some(AppCallback::DeletePlaylistFromLibrary(pid)),
                            };
                        }
                        KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                            self.delete_confirm = None;
                        }
                        _ => {}
                    }
                }
            }
            return AsyncTask::new_no_op().into();
        }

        // Route events to popup if one is active
        if self.lyrics_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.lyrics_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.lyrics_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.album_art_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.album_art_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.album_art_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.song_info_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.song_info_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.song_info_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.playlist_save_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.playlist_save_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.playlist_save_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.playlist_update_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.playlist_update_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.playlist_update_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.playlist_rename_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.playlist_rename_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.playlist_rename_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.playlist_edit_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.playlist_edit_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.playlist_edit_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.playlist_details_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.playlist_details_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.playlist_details_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if self.playlist_editor_popup.is_some() {
            if let Event::Key(k) = event {
                let popup = self.playlist_editor_popup.as_mut().unwrap();
                let (effect, callback) = popup.handle_key(k);
                let effect = effect.map_frontend(|this: &mut Self| {
                    this.playlist_editor_popup.as_mut().unwrap()
                });
                return YoutuiEffect { effect, callback };
            }
        }
        if let Some(effect) = self.try_handle_text(&event) {
            return effect.into();
        };
        match event {
            Event::Key(k) => return self.handle_key_event(k),
            Event::Mouse(m) => return self.handle_mouse_event(m).into(),
            other => tracing::warn!("Received unimplemented {:?} event", other),
        }
        AsyncTask::new_no_op().into()
    }
    pub async fn handle_media_controls_event(
        &mut self,
        event: souvlaki::MediaControlEvent,
    ) -> YoutuiEffect<Self> {
        // This conversion function is written here as this is expected to be the only
        // location it is used.
        let convert_dir = |dir| match dir {
            souvlaki::SeekDirection::Forward => SeekDirection::Forward,
            souvlaki::SeekDirection::Backward => SeekDirection::Back,
        };
        match event {
            souvlaki::MediaControlEvent::Play => return self.resume().into(),
            souvlaki::MediaControlEvent::Pause => return self.pause().into(),
            souvlaki::MediaControlEvent::Toggle => return self.pauseplay().into(),
            souvlaki::MediaControlEvent::Next => return self.handle_next().into(),
            souvlaki::MediaControlEvent::Previous => return self.handle_prev().into(),
            souvlaki::MediaControlEvent::Stop => return self.stop().into(),
            souvlaki::MediaControlEvent::Seek(seek_direction) => {
                return self
                    .handle_seek(SEEK_AMOUNT, convert_dir(seek_direction))
                    .into();
            }
            souvlaki::MediaControlEvent::SeekBy(seek_direction, duration) => {
                return self
                    .handle_seek(duration, convert_dir(seek_direction))
                    .into();
            }
            souvlaki::MediaControlEvent::SetPosition(media_position) => {
                return self.handle_seek_to(media_position.0).into();
            }
            souvlaki::MediaControlEvent::SetVolume(v) => {
                return self.handle_set_volume((v * 100.0) as u8).into();
            }
            souvlaki::MediaControlEvent::Quit => {
                return (AsyncTask::new_no_op(), Some(AppCallback::Quit)).into();
            }
            souvlaki::MediaControlEvent::OpenUri(_) => {
                tracing::info!("Received intentionally unhandled event {:?}", event)
            }
            souvlaki::MediaControlEvent::Raise => {
                tracing::info!("Received intentionally unhandled event {:?}", event)
            }
        }
        AsyncTask::new_no_op().into()
    }
    pub async fn handle_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.playlist.handle_tick().await;
    }
    pub fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) -> YoutuiEffect<Self> {
        use crossterm::event::KeyCode;
        // Count prefix: only active in scrollable list contexts
        let count_prefix_active = matches!(self.context, WindowContext::Playlist | WindowContext::Browser);
        if let KeyCode::Char(c) = key_event.code {
            if c.is_ascii_digit() && count_prefix_active {
                if !self.key_stack.is_empty() {
                    // A digit after already having keys — not a count (part of mode)
                    self.key_stack.push(key_event);
                } else {
                    // First key is a digit — accumulate as count
                    let digit = c.to_digit(10).unwrap_or(0) as usize;
                    self.count_prefix = self.count_prefix * 10 + digit;
                    tracing::debug!("Count prefix: {}", self.count_prefix);
                    return YoutuiEffect::new_no_op();
                }
            } else {
                // Non-digit key: apply accumulated count
                if self.count_prefix > 0 && !key_event.modifiers.intersects(crossterm::event::KeyModifiers::SHIFT | crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::ALT) {
                    self.key_stack.push(key_event);
                    return self.global_handle_key_stack_with_count();
                }
                self.count_prefix = 0;
                self.key_stack.push(key_event);
            }
        } else {
            self.count_prefix = 0;
            self.key_stack.push(key_event);
        }
        self.global_handle_key_stack()
    }
    fn handle_mouse_event(
        &mut self,
        mouse_event: crossterm::event::MouseEvent,
    ) -> ComponentEffect<Self> {
        tracing::warn!("Received unimplemented {:?} mouse event", mouse_event);
        AsyncTask::new_no_op()
    }
    pub fn handle_list_action(&mut self, action: ListAction) -> ComponentEffect<Self> {
        if self.is_scrollable() {
            match action {
                ListAction::Up => {
                    let count = self.count_prefix.max(1) as isize;
                    self.count_prefix = 0;
                    self.increment_list(-count)
                },
                ListAction::Down => {
                    let count = self.count_prefix.max(1) as isize;
                    self.count_prefix = 0;
                    self.increment_list(count)
                },
                ListAction::PageUp => self.increment_list(-PAGE_KEY_LINES),
                ListAction::PageDown => self.increment_list(PAGE_KEY_LINES),
                ListAction::First => {
                    if self.help.shown {
                        self.help.cur = 0;
                    } else {
                        match self.context {
                            WindowContext::Browser => self.browser.go_to_first(),
                            WindowContext::Playlist => self.playlist.go_to_first(),
                            WindowContext::Logs => self.browser.go_to_first(),
                            WindowContext::PlaylistSavePopup => {}
                            WindowContext::PlaylistUpdatePopup => {}
                            WindowContext::Lyrics => {}
                            WindowContext::SongInfo => {}
            WindowContext::PlaylistEditor => {
                if let Some(ref mut ed) = self.playlist_editor_popup {
                    ed.scroll_offset = 0;
                    ed.cursor = 0;
                }
            }
            WindowContext::PlaylistRenamePopup => {}
            WindowContext::PlaylistEditPopup => {}
            WindowContext::PlaylistDetailsPopup => {}
                        }
                    }
                }
                ListAction::Last => {
                    if self.help.shown {
                        self.help.cur = self.help.len.saturating_sub(1);
                    } else {
                        match self.context {
                            WindowContext::Browser => self.browser.go_to_last(),
                            WindowContext::Playlist => self.playlist.go_to_last(),
                            WindowContext::Logs => self.browser.go_to_last(),
                            WindowContext::PlaylistSavePopup => {}
                            WindowContext::PlaylistUpdatePopup => {}
                            WindowContext::Lyrics => {}
                            WindowContext::SongInfo => {}
            WindowContext::PlaylistEditor => {
                if let Some(ref mut ed) = self.playlist_editor_popup {
                    ed.cursor = ed.tracks.len().saturating_sub(1);
                    ed.scroll_offset = 0;
                }
            }
            WindowContext::PlaylistRenamePopup => {}
            WindowContext::PlaylistEditPopup => {}
            WindowContext::PlaylistDetailsPopup => {}
                        }
                    }
                }
            }
        }
        AsyncTask::new_no_op()
    }
    pub fn handle_text_entry_action(&mut self, action: TextEntryAction) -> ComponentEffect<Self> {
        if !self.is_text_handling() {
            return AsyncTask::new_no_op();
        }
        match self.context {
            WindowContext::Browser => self
                .browser
                .handle_text_entry_action(action)
                .map_frontend(|this: &mut Self| &mut this.browser),
            WindowContext::Playlist => {
                self.playlist.handle_text_entry_action(action);
                AsyncTask::new_no_op()
            }
            WindowContext::Logs => AsyncTask::new_no_op(),
            WindowContext::PlaylistSavePopup => AsyncTask::new_no_op(),
            WindowContext::PlaylistUpdatePopup => AsyncTask::new_no_op(),
            WindowContext::Lyrics => AsyncTask::new_no_op(),
            WindowContext::SongInfo => AsyncTask::new_no_op(),
            WindowContext::PlaylistEditor => AsyncTask::new_no_op(),
            WindowContext::PlaylistRenamePopup => AsyncTask::new_no_op(),
            WindowContext::PlaylistEditPopup => AsyncTask::new_no_op(),
            WindowContext::PlaylistDetailsPopup => AsyncTask::new_no_op(),
        }
    }
    pub fn pauseplay(&mut self) -> ComponentEffect<Self> {
        self.playlist
            .pauseplay()
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn resume(&mut self) -> ComponentEffect<Self> {
        self.playlist
            .resume()
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn pause(&mut self) -> ComponentEffect<Self> {
        self.playlist
            .pause()
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn stop(&mut self) -> ComponentEffect<Self> {
        self.playlist
            .stop()
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_next(&mut self) -> ComponentEffect<Self> {
        self.playlist
            .handle_next()
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_prev(&mut self) -> ComponentEffect<Self> {
        self.playlist
            .handle_previous()
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_increase_volume(&mut self, inc: i8) -> ComponentEffect<Self> {
        // Visually update the state first for instant feedback.
        self.increase_volume(inc);
        AsyncTask::new_future_option(
            IncreaseVolume(inc),
            HandleVolumeUpdate,
            Some(Constraint::new_block_same_type()),
        )
    }
    pub fn handle_set_volume(&mut self, new_vol: u8) -> ComponentEffect<Self> {
        // Visually update the state first for instant feedback.
        self.set_volume(new_vol);
        AsyncTask::new_future_option(
            SetVolume(new_vol),
            HandleVolumeUpdate,
            Some(Constraint::new_block_same_type()),
        )
    }
    pub fn handle_seek(
        &mut self,
        duration: Duration,
        direction: SeekDirection,
    ) -> ComponentEffect<Self> {
        self.playlist
            .handle_seek(duration, direction)
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_seek_to(&mut self, position: Duration) -> ComponentEffect<Self> {
        self.playlist
            .handle_seek_to(position)
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_volume_update(&mut self, update: VolumeUpdate) {
        self.playlist.handle_volume_update(update)
    }
    pub fn handle_create_playlist_from_popup(
        &mut self,
        title: String,
        description: Option<String>,
        privacy: Option<ytmapi_rs::query::playlist::PrivacyStatus>,
        video_ids: Vec<ytmapi_rs::common::VideoID<'static>>,
    ) -> ComponentEffect<Self> {
        use crate::app::server::CreatePlaylistWithVideos;
        use crate::app::ui::playlist::effect_handlers_playlist::{
            HandleCreatePlaylistOk, HandleCreatePlaylistError,
        };
        AsyncTask::new_future_try(
            CreatePlaylistWithVideos {
                title,
                description,
                video_ids,
                privacy,
            },
            HandleCreatePlaylistOk,
            HandleCreatePlaylistError,
            None,
        )
        .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_add_songs_to_playlist(
        &mut self,
        song_list: Vec<ListSong>,
    ) -> ComponentEffect<Self> {
        let (_, effect) = self.playlist.push_song_list(song_list);
        effect.map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_add_songs_to_playlist_and_play(
        &mut self,
        song_list: Vec<ListSong>,
    ) -> ComponentEffect<Self> {
        let effect = self.playlist.reset();
        let (id, next_effect) = self.playlist.push_song_list(song_list);
        effect
            .push(next_effect)
            .push(self.playlist.play_song_id(id))
            .map_frontend(|this: &mut Self| &mut this.playlist)
    }
    pub fn handle_insert_next(
        &mut self,
        song_list: Vec<ListSong>,
    ) -> ComponentEffect<Self> {
        let (_, effect) = self.playlist.insert_next_song_list(song_list);
        effect.map_frontend(|this: &mut Self| &mut this.playlist)
    }
    fn global_handle_key_stack(&mut self) -> YoutuiEffect<Self> {
        match handle_key_stack(self.get_active_keybinds(&self.config), &self.key_stack) {
            KeyHandleAction::Action(a) => {
                let effect = self.apply_action(a).into();
                self.key_stack.clear();
                effect
            }
            KeyHandleAction::Mode { .. } => AsyncTask::new_no_op().into(),
            KeyHandleAction::NoMap => {
                self.key_stack.clear();
                AsyncTask::new_no_op().into()
            }
        }
    }
    fn global_handle_key_stack_with_count(&mut self) -> YoutuiEffect<Self> {
        let count = self.count_prefix;
        self.count_prefix = 0;
        self.playlist.pending_count = count;
        self.global_handle_key_stack()
    }
    fn key_pending(&self) -> bool {
        !self.key_stack.is_empty()
    }
    pub fn toggle_help(&mut self) {
        if self.help.shown {
            self.help.shown = false;
        } else {
            self.help.shown = true;
            // Setup Help menu parameters
            self.help.cur = 0;
            // We have to get the keybind length this way as the help menu iterator is not
            // ExactSized
            self.help.len = self.get_help_list_items().len();
        }
    }
    /// Visually increment the volume, note, does not actually change the
    /// volume.
    fn increase_volume(&mut self, inc: i8) {
        self.playlist.increase_volume(inc);
    }
    /// Visually set the volume, note, does not actually change the volume.
    fn set_volume(&mut self, new_vol: u8) {
        self.playlist.set_volume(new_vol);
    }
    pub fn handle_change_context(&mut self, new_context: WindowContext) {
        std::mem::swap(&mut self.context, &mut self.prev_context);
        self.context = new_context;
    }
    pub fn handle_toggle_playlist(&mut self) {
        if self.context == WindowContext::Playlist {
            // Leave Playlist → restore where we were
            if matches!(self.prev_context, WindowContext::Lyrics | WindowContext::SongInfo | WindowContext::PlaylistSavePopup | WindowContext::PlaylistUpdatePopup | WindowContext::PlaylistEditor) {
                // prev_context is a stale popup context — go to Browser instead
                self.prev_context = WindowContext::Playlist;
                self.context = WindowContext::Browser;
            } else {
                std::mem::swap(&mut self.context, &mut self.prev_context);
            }
        } else {
            // Enter Playlist → save current as prev
            self.prev_context = self.context;
            self.context = WindowContext::Playlist;
        }
        self.dismiss_search();
    }
    fn dismiss_search(&mut self) {
        self.browser.dismiss_search();
    }
    pub fn open_playlist_save_popup(&mut self, video_ids: Vec<ytmapi_rs::common::VideoID<'static>>) {
        self.playlist_save_popup = Some(PlaylistSavePopup::new(video_ids));
        self.prev_context = self.context;
        self.context = WindowContext::PlaylistSavePopup;
    }
    pub fn open_playlist_update_popup(
        &mut self,
        video_ids: Vec<ytmapi_rs::common::VideoID<'static>>,
    ) -> ComponentEffect<Self> {
        use crate::app::server::GetAllLibraryPlaylists;
        use crate::app::ui::playlist::effect_handlers_playlist::{
            HandleGetAllLibraryPlaylistsOk, HandleGetAllLibraryPlaylistsError,
        };
        self.playlist_update_popup = Some(PlaylistUpdatePopup::new(video_ids));
        self.prev_context = self.context;
        self.context = WindowContext::PlaylistUpdatePopup;
        AsyncTask::new_future_try(
            GetAllLibraryPlaylists,
            HandleGetAllLibraryPlaylistsOk,
            HandleGetAllLibraryPlaylistsError,
            None,
        )
        .map_frontend(|this: &mut Self| {
            if this.playlist_update_popup.is_none() {
                this.playlist_update_popup = Some(PlaylistUpdatePopup::new(Vec::new()));
            }
            this.playlist_update_popup.as_mut().expect("just set")
        })
    }
    pub fn open_lyrics_popup(&mut self, artist: String, title: String) -> ComponentEffect<Self> {
        use crate::app::server::GetLyrics;
        use crate::app::ui::playlist::effect_handlers_playlist::{
            HandleGetLyricsOk, HandleGetLyricsErr,
            HandleGetAnnotationsOk, HandleGetAnnotationsErr,
        };
        let genius_token = self.config.scrobbling.genius_token.clone();
        let has_genius = !genius_token.is_empty();
        tracing::info!("open_lyrics_popup: artist={}, title={}, has_genius={}, token_len={}", artist, title, has_genius, genius_token.len());

        let cache_key = format!("{}||{}", artist, title);
        self.lyrics_viewing_idx = self.playlist.list.get_list_iter()
            .position(|s| s.title == title && s.artists.iter().any(|a| artist.contains(a.name.as_str()) || a.name.contains(&artist)));

        // Inflight dedup: skip if already fetching
        if self.lyrics_inflight.contains(&cache_key) {
            tracing::info!("Lyrics inflight dedup: skip duplicate request for {}", cache_key);
            return AsyncTask::new_no_op();
        }

        // LRU cache: skip fetch if cached
        if let Some(popup) = &self.lyrics_popup {
            if popup.lyrics_cache.peek(&cache_key).is_some() {
                tracing::info!("Lyrics cache hit for {}", cache_key);
                return AsyncTask::new_no_op();
            }
        }

        let inflight_key = cache_key.clone();
        let artist2 = artist.clone();
        let title2 = title.clone();
        self.lyrics_inflight.insert(cache_key.clone());
        let mut popup = LyricsPopup::new(artist.clone(), title.clone());
        popup.lyrics_cache_key = Some(cache_key.clone());
        self.lyrics_popup = Some(popup);
        self.prev_context = self.context;
        self.context = WindowContext::Lyrics;
        let effect: ComponentEffect<YoutuiWindow> = AsyncTask::new_future_try(
            GetLyrics(artist.clone(), title.clone(), genius_token.clone()),
            HandleGetLyricsOk,
            HandleGetLyricsErr,
            None,
        )
        .map_frontend(move |this: &mut Self| {
            this.lyrics_inflight.remove(&inflight_key);
            if this.lyrics_popup.is_none() {
                this.lyrics_popup = Some(LyricsPopup::new(artist2, title2));
            }
            this.lyrics_popup.as_mut().expect("just set")
        });

        if has_genius {
            use crate::app::server::GetAnnotations;
            let ann_artist = artist.clone();
            let ann_title = title.clone();
            let ann_effect: ComponentEffect<YoutuiWindow> = AsyncTask::new_future_try(
                GetAnnotations(artist, title, genius_token),
                HandleGetAnnotationsOk,
                HandleGetAnnotationsErr,
                None,
            )
            .map_frontend(move |this: &mut Self| {
                if this.lyrics_popup.is_none() {
                    this.lyrics_popup = Some(LyricsPopup::new(ann_artist, ann_title));
                }
                this.lyrics_popup.as_mut().expect("just set")
            });
            return effect.push(ann_effect);
        }
        effect
    }
    pub fn play_yt_url(&mut self, url: String) -> ComponentEffect<Self> {
        use ytmapi_rs::common::YoutubeID;
        tracing::info!("Playing URL: {}", url);
        self.prev_context = self.context;
        self.context = WindowContext::Playlist;

        // Check for playlist URL FIRST — before video extraction
        if url.contains("playlist?list=") {
            if let Some(list_id) = url.split("list=").nth(1).and_then(|s| s.split('&').next()).map(|s| s.to_string()) {
                if !list_id.is_empty() {
                    let pl_id = ytmapi_rs::common::PlaylistID::from_raw(format!("VL{}", list_id));
                    use crate::app::server::GetPlaylistTracks;
                    use crate::app::ui::playlist::effect_handlers_playlist::{
                        HandleGetPlaylistTracksOk, HandleGetPlaylistTracksErr,
                    };
                    return AsyncTask::new_future_try(
                        GetPlaylistTracks(pl_id),
                        HandleGetPlaylistTracksOk,
                        HandleGetPlaylistTracksErr,
                        None,
                    )
                    .map_frontend(|this: &mut Self| &mut this.playlist);
                }
            }
        }

        // Extract video ID for single video
        let video_id_str = if url.contains("watch?v=") {
            url.split("watch?v=").nth(1).unwrap_or(&url)
                .split('&').next().unwrap_or("")
                .to_string()
        } else if url.contains("youtu.be/") {
            url.split("youtu.be/").nth(1).unwrap_or(&url)
                .split('?').next().unwrap_or("")
                .to_string()
        } else {
            url.rsplit('/').next().unwrap_or(&url)
                .split('?').next().unwrap_or(&url)
                .to_string()
        };

        let effect;

        if video_id_str.len() >= 10 && video_id_str.len() <= 20 {
            let vid = ytmapi_rs::common::VideoID::from_raw(video_id_str.clone());
            self.playlist.url_added = true;
            effect = self.playlist.add_yt_video(vid, &url)
                .map_frontend(|this: &mut Self| &mut this.playlist);
        } else {
            tracing::warn!("Invalid video URL: {}", url);
            return AsyncTask::new_no_op();
        }

        effect
    }
    pub fn open_song_info_popup(&mut self, song: crate::app::structures::ListSong) -> ComponentEffect<Self> {
        self.song_info_popup = Some(SongInfoPopup::new(song));
        self.prev_context = self.context;
        self.context = WindowContext::SongInfo;
        AsyncTask::new_no_op()
    }

    pub fn close_popup(&mut self) {
        self.playlist_save_popup = None;
        self.playlist_update_popup = None;
        self.lyrics_popup = None;
        self.song_info_popup = None;
        self.album_art_popup = None;
        self.config_editor_popup = None;
        self.playlist_editor_popup = None;
        self.playlist_rename_popup = None;
        self.playlist_edit_popup = None;
        self.playlist_details_popup = None;
        self.delete_confirm = None;
        // Restore context from prev_context, but don't leave prev_context
        // as a stale popup context (would trap user on next toggle).
        self.context = self.prev_context;
        if matches!(self.context, WindowContext::Lyrics | WindowContext::SongInfo | WindowContext::PlaylistSavePopup | WindowContext::PlaylistUpdatePopup | WindowContext::PlaylistEditor | WindowContext::PlaylistRenamePopup | WindowContext::PlaylistEditPopup | WindowContext::PlaylistDetailsPopup) {
            // prev_context was also a popup (nested) — fall back to safe context
            self.context = WindowContext::Playlist;
        }
        self.prev_context = WindowContext::Browser;
    }
    pub fn open_playlist_rename_popup(&mut self, playlist_id: PlaylistID<'static>, current_title: String) {
        self.playlist_rename_popup = Some(PlaylistRenamePopup::new(playlist_id, current_title));
        self.prev_context = self.context;
        self.context = WindowContext::PlaylistRenamePopup;
    }

    pub fn open_playlist_edit_popup(&mut self, playlist_id: PlaylistID<'static>, title: String) {
        self.playlist_edit_popup = Some(PlaylistEditPopup::new(playlist_id, title));
        self.prev_context = self.context;
        self.context = WindowContext::PlaylistEditPopup;
    }

    pub fn open_playlist_details_popup(&mut self, playlist_id: PlaylistID<'static>, title: String) -> ComponentEffect<Self> {
        use crate::app::server::GetPlaylistDetailsMessage;
        use crate::app::ui::playlist::effect_handlers_playlist::{
            HandleFetchPlaylistDetailsOk, HandleFetchPlaylistDetailsError,
        };
        self.playlist_details_popup = Some(PlaylistDetailsPopup::new(Some(title)));
        self.prev_context = self.context;
        self.context = WindowContext::PlaylistDetailsPopup;
        AsyncTask::new_future_try(
            GetPlaylistDetailsMessage(playlist_id),
            HandleFetchPlaylistDetailsOk,
            HandleFetchPlaylistDetailsError,
            None,
        )
        .map_frontend(|this: &mut Self| {
            if this.playlist_details_popup.is_none() {
                this.playlist_details_popup = Some(PlaylistDetailsPopup::new(None));
            }
            this.playlist_details_popup.as_mut().expect("just set")
        })
    }

    pub fn open_config_editor(&mut self) {
        let config_dir = crate::get_config_dir().ok();
        let config_path = config_dir.map(|d| d.join("config.toml")).unwrap_or_else(|| PathBuf::from("config.toml"));
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        self.prev_context = self.context;
        self.config_editor_popup = Some(ConfigEditorPopup::new(config_path, content));
    }
    fn _revert_context(&mut self) {
        std::mem::swap(&mut self.context, &mut self.prev_context);
    }
    // The downside of this approach is that if draw_popup is calling this function,
    // it is gettign called every tick.
    // Consider a way to set this in the in state memory.
    fn get_cur_displayable_mode(
        &self,
    ) -> Option<DisplayableMode<'_, impl Iterator<Item = DisplayableKeyAction<'_>>>> {
        let KeyHandleAction::Mode { name, keys } =
            handle_key_stack(self.get_active_keybinds(&self.config), &self.key_stack)
        else {
            return None;
        };
        let displayable_commands = keys
            .iter()
            .map(|(kb, kt)| DisplayableKeyAction::from_keybind_and_action_tree(kb, kt));
        Some(DisplayableMode {
            displayable_commands,
            description: name.into(),
        })
    }
}

#[derive(Debug, PartialEq)]
struct HandleVolumeUpdate;

impl_youtui_task_handler!(
    HandleVolumeUpdate,
    VolumeUpdate,
    YoutuiWindow,
    |_, update| |this: &mut YoutuiWindow| {
        YoutuiWindow::handle_volume_update(this, update);
        AsyncTask::new_no_op()
    }
);
pub mod components;
