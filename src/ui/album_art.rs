use eyre::{eyre, Result};
use image::DynamicImage;
use ratatui::{prelude::Rect, Frame};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};

use crate::library::Track;

use super::Ui;

pub struct AlbumArt {
    picker: Picker,
    image_protocol: Option<Box<dyn StatefulProtocol>>,
}

impl AlbumArt {
    pub fn new() -> Result<Self> {
        let mut picker =
            Picker::from_termios().map_err(|e| eyre!("couldn't pick image protocol: {e}"))?;
        picker.guess_protocol();
        Ok(Self { picker, image_protocol: None })
    }

    pub fn set_track(&mut self, track: Option<&Track>) -> Result<()> {
        let Some(track) = track else {
            self.image_protocol = None;
            return Ok(());
        };
        let album_art = track.album_art()?.unwrap_or_else(|| DynamicImage::new_rgba8(0, 0));
        self.image_protocol = Some(self.picker.new_resize_protocol(album_art));
        Ok(())
    }

    pub fn draw(&mut self, _ui: &Ui, frame: &mut Frame, area: Rect) -> Result<()> {
        if let Some(protocol) = self.image_protocol.as_mut() {
            frame.render_stateful_widget(StatefulImage::new(None), area, protocol);
        }
        Ok(())
    }
}
