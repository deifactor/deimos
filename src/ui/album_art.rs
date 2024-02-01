use std::sync::Arc;

use eyre::{eyre, Result};
use image::DynamicImage;
use lofty::TaggedFileExt;
use ratatui::{prelude::Rect, Frame};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize, StatefulImage};

use crate::library::Track;

use super::Ui;

pub struct AlbumArt {
    picker: Picker,
    last_track: Option<Arc<Track>>,
    image_protocol: Option<Box<dyn StatefulProtocol>>,
}

impl AlbumArt {
    pub fn new() -> Result<Self> {
        let mut picker =
            Picker::from_termios().map_err(|e| eyre!("couldn't pick image protocol: {e}"))?;
        picker.guess_protocol();
        Ok(Self { picker, last_track: None, image_protocol: None })
    }

    fn set_track(&mut self, track: Option<Arc<Track>>) -> Result<()> {
        let Some(track) = track else {
            self.last_track = None;
            self.image_protocol = None;
            return Ok(());
        };
        if self.last_track.as_ref() == Some(&track) {
            return Ok(());
        }
        self.last_track = Some(Arc::clone(&track));
        let tagged_file = lofty::read_from_path(&track.path)?;
        let picture = tagged_file.primary_tag().and_then(|tag| tag.pictures().first());
        let image = picture
            .map_or(Ok(DynamicImage::new_rgb8(0, 0)), |p| image::load_from_memory(p.data()))?;
        self.image_protocol = Some(self.picker.new_resize_protocol(image));
        Ok(())
    }

    pub fn draw(
        &mut self,
        _ui: &Ui,
        frame: &mut Frame,
        area: Rect,
        track: Option<Arc<Track>>,
    ) -> Result<()> {
        self.set_track(track)?;
        if let Some(protocol) = self.image_protocol.as_mut() {
            frame.render_stateful_widget(
                StatefulImage::new(None).resize(Resize::Fit),
                area,
                protocol,
            );
        }
        Ok(())
    }
}
