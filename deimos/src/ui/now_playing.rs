use std::time::Duration;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::Paragraph,
};

use crate::library::Track;

#[derive(Debug, PartialEq, Eq)]
pub struct PlayState {
    pub timestamp: Duration,
    pub track: Track,
}

#[derive(Debug, Default)]
pub struct NowPlaying {
    pub play_state: Option<PlayState>,
}

/// Drawing code
impl NowPlaying {
    pub fn draw(
        &mut self,
        _ui: &crate::ui::Ui,
        frame: &mut ratatui::Frame<crate::ui::DeimosBackend>,
        area: ratatui::layout::Rect,
    ) -> anyhow::Result<()> {
        let Some(play_state) = &self.play_state else {
            return Ok(());
        };

        let title = play_state.track.title.as_deref().unwrap_or("<unknown>");
        let artist = format!("{}", play_state.track.artist);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        frame.render_widget(
            Paragraph::new(format!("{title} - {artist}")).alignment(Alignment::Center),
            chunks[0],
        );
        let mins = play_state.timestamp.as_secs() / 60;
        let secs = play_state.timestamp.as_secs() % 60;

        let total_mins = (play_state.track.length / 60.0).floor() as u64;
        let total_secs = (play_state.track.length % 60.0).ceil() as u64;
        frame.render_widget(
            Paragraph::new(format!(
                "{mins:0>2}:{secs:0>2} / {total_mins:0>2}:{total_secs:0>2}"
            )),
            chunks[1],
        );

        Ok(())
    }
}
