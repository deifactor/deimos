use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::ui::{Component, DeimosBackend, FocusTarget, Ui};

#[derive(Debug, PartialEq, Eq)]
pub struct Track {
    pub song_id: i64,
    pub number: Option<i64>,
    pub title: Option<String>,
}

#[derive(Debug, Default)]
pub struct TrackList {
    tracks: Vec<Track>,
    state: ListState,
}

/// Methods for manipulating the state
impl TrackList {
    pub fn new(tracks: Vec<Track>) -> Self {
        Self {
            tracks,
            state: ListState::default(),
        }
    }
}

/// Drawing code
impl Component for TrackList {
    fn draw(&mut self, ui: &Ui, frame: &mut Frame<DeimosBackend>, area: Rect) -> Result<()> {
        let block = Block::default()
            .title("Tracks")
            .borders(Borders::ALL)
            .border_style(ui.border(ui.is_focused(FocusTarget::TrackList)));

        let list = List::new(
            self.tracks
                .iter()
                .map(|track| ListItem::new(track.title.as_deref().unwrap_or("<unknown>")))
                .collect_vec(),
        )
        .block(block);
        frame.render_widget(list, area);
        Ok(())
    }
}
