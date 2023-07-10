use std::time::Duration;

use ratatui::widgets::{Block, Borders};

use crate::ui::{Component, FocusTarget};

#[derive(Debug, PartialEq, Eq)]
pub struct Track {
    pub number: i32,
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
}

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
impl Component for NowPlaying {
    fn draw(
        &mut self,
        ui: &crate::ui::Ui,
        frame: &mut ratatui::Frame<crate::ui::DeimosBackend>,
        area: ratatui::layout::Rect,
    ) -> anyhow::Result<()> {
        let block = Block::default()
            .title("Now Playing")
            .borders(Borders::ALL)
            // can't ever receive
            .border_style(ui.border(false));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 3 || inner.height < 1 {
            return Ok(());
        }

        let Some(play_state) = &self.play_state else {
            return Ok(());
        };

        Ok(())
    }
}
