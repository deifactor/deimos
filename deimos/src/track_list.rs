use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
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

    pub fn move_selection(&mut self, amount: isize) {
        if self.tracks.is_empty() {
            return;
        }
        self.state.select(match self.state.selected() {
            Some(selected) => Some(
                selected
                    .saturating_add_signed(amount)
                    .min(self.tracks.len() - 1),
            ),
            None if amount > 0 => Some(0),
            None => None,
        });
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
        .highlight_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 30)))
        .block(block);
        frame.render_stateful_widget(list, area, &mut self.state);
        Ok(())
    }
}
