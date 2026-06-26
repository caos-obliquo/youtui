use crate::app::component::actionhandler::ComponentEffect;
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use std::rc::Rc;

pub struct AlbumArtPopup {
    pub thumbnails: Vec<Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>>,
    pub index: usize,
}

impl_youtui_component!(AlbumArtPopup);

impl AlbumArtPopup {
    pub fn new(
        thumbnails: Vec<Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>>,
        index: usize,
    ) -> Self {
        Self { thumbnails, index }
    }

    pub fn total(&self) -> usize {
        self.thumbnails.len()
    }

    pub fn current_thumbnail(&self) -> Option<&Rc<crate::app::server::song_thumbnail_downloader::SongThumbnail>> {
        self.thumbnails.get(self.index)
    }

    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> (ComponentEffect<Self>, Option<AppCallback>) {
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                (AsyncTask::new_no_op(), Some(AppCallback::ClosePopup))
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.thumbnails.len() > 1 {
                    self.index = if self.index == 0 {
                        self.thumbnails.len() - 1
                    } else {
                        self.index - 1
                    };
                }
                (AsyncTask::new_no_op(), None)
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.thumbnails.len() > 1 {
                    self.index = (self.index + 1) % self.thumbnails.len();
                }
                (AsyncTask::new_no_op(), None)
            }
            _ => (AsyncTask::new_no_op(), None),
        }
    }
}
