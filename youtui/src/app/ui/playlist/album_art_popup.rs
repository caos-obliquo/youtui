use crate::app::component::actionhandler::ComponentEffect;
use crate::app::ui::AppCallback;
use async_callback_manager::AsyncTask;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
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

    fn centered_rect_fixed(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, picker: &Picker) {
        f.render_widget(Clear, area);
        let centered = Self::centered_rect_fixed(90, 90, area);
        if centered.width < 4 || centered.height < 4 {
            f.render_widget(Paragraph::new("Terminal too small").centered(), area);
            return;
        }
        match picker.new_protocol(self.thumbnail.in_mem_image.clone(), centered, Resize::Fit(None)) {
            Ok(protocol) => f.render_widget(Image::new(&protocol), centered),
            Err(_) => f.render_widget(Paragraph::new("Failed to load album art").centered(), area),
        }
    }
}
