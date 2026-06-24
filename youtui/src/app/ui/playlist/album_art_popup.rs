use crate::app::component::actionhandler::ComponentEffect;
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use std::rc::Rc;

pub struct AlbumArtPopup {
    pub thumbnail: Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>,
}

impl_youtui_component!(AlbumArtPopup);

impl AlbumArtPopup {
    pub fn new(thumbnail: Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>) -> Self {
        Self { thumbnail }
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }
}
