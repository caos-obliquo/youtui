use crate::app::component::actionhandler::ComponentEffect;
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, Paragraph};
use ratatui_image::{Image, Resize};
use ratatui_image::picker::Picker;
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

    pub fn draw(&mut self, f: &mut Frame, area: Rect, picker: &Picker) {
        f.render_widget(Clear, area);
        let panel_w = u32::from(area.width.saturating_sub(2));
        let panel_h = u32::from(area.height.saturating_sub(2));
        let resized = image::DynamicImage::ImageRgba8(image::imageops::resize(
            &self.thumbnail.in_mem_image,
            panel_w,
            panel_h,
            image::imageops::FilterType::Lanczos3,
        ));
        let img = picker.new_protocol(
            resized,
            Rect {
                x: 0,
                y: 0,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(2),
            },
            Resize::Fit(None),
        );
        match img {
            Ok(protocol) => {
                let inner = Rect {
                    x: area.x + 1,
                    y: area.y + 1,
                    width: area.width.saturating_sub(2),
                    height: area.height.saturating_sub(2),
                };
                f.render_widget(Image::new(&protocol), inner);
            }
            Err(_) => {
                let fallback = Paragraph::new("Failed to load album art").centered();
                f.render_widget(fallback, area);
            }
        }
    }
}
