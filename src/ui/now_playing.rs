use std::{sync::Arc, time::Duration};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::Paragraph,
};

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
        let artist = format!("{}", track.artist);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        frame.render_widget(
            Paragraph::new(format!("{title} - {artist}")).alignment(Alignment::Center),
            chunks[0],
        );
        let mins = timestamp.as_secs() / 60;
        let secs = timestamp.as_secs() % 60;

        let total_mins = (track.length / 60.0).floor() as u64;
        let total_secs = (track.length % 60.0).ceil() as u64;
        frame.render_widget(
            Paragraph::new(format!("{mins:0>2}:{secs:0>2} / {total_mins:0>2}:{total_secs:0>2}")),
            chunks[1],
        );

        Ok(())
    }
}
