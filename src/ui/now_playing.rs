use std::{sync::Arc, time::Duration};

use ratatui::{style::Stylize, widgets::Paragraph};

use crate::library::Track;

/// Widget that displays the current song and timestamp within that song.
#[derive(Debug, Default)]
pub struct NowPlaying {
    pub timestamp: Option<Duration>,
    pub track: Option<Arc<Track>>,
}

/// Drawing code
impl NowPlaying {
    pub fn draw(
        self,
        _ui: &crate::ui::Ui,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
    ) -> eyre::Result<()> {
        let (Some(timestamp), Some(track)) = (self.timestamp.as_ref(), self.track.as_ref()) else {
            return Ok(());
        };

        let title = track.title.as_deref().unwrap_or("<unknown>");
        let album = &track.album;
        let artist = &track.artist;
        let mins = timestamp.as_secs() / 60;
        let secs = timestamp.as_secs() % 60;

        let total_mins = (track.length / 60.0).floor() as u64;
        let total_secs = (track.length % 60.0).ceil() as u64;

        frame.render_widget(
            Paragraph::new(format!(
                "{artist}\n{album}\n{title}\n\
                    {mins:0>2}:{secs:0>2} / {total_mins:0>2}:{total_secs:0>2}"
            ))
            .bold(),
            area,
        );

        Ok(())
    }
}
