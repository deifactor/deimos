use itertools::Itertools;
use ratatui::{
    backend::Backend,
    layout::Rect,
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

#[derive(Debug, PartialEq, Eq)]
pub struct Track {
    pub song_id: i64,
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
impl TrackList {
    pub fn draw<B: Backend>(&mut self, frame: &mut Frame<B>, area: Rect) {
        let block = Block::default().title("Tracks").borders(Borders::ALL);
        let list = List::new(
            self.tracks
                .iter()
                .map(|track| ListItem::new(track.title.as_deref().unwrap_or("<unknown>")))
                .collect_vec(),
        )
        .block(block);
        frame.render_widget(list, area);
    }
}
