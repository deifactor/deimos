use std::sync::Arc;

use crate::library::Track;

#[derive(Debug)]
pub struct PlayQueue {
    index: Option<usize>,
    tracks: Vec<Arc<Track>>,
}

impl PlayQueue {
    pub fn new(tracks: Vec<Arc<Track>>) -> Self {
        Self { index: None, tracks }
    }

    pub fn current(&self) -> Option<usize> {
        self.index
    }

    pub fn set_current(&mut self, current: Option<usize>) {
        self.index = current;
    }

    pub fn current_track(&self) -> Option<Arc<Track>> {
        self.index.map(|i| Arc::clone(&self.tracks[i]))
    }

    /// Index of the previous track. `None` if this is the first track.
    pub fn previous(&self) -> Option<usize> {
        self.index.and_then(|i| i.checked_sub(1))
    }

    /// Index of the next track. `None` if this would go off the end.
    pub fn next(&self) -> Option<usize> {
        self.index.map(|i| i + 1).filter(|i| *i < self.tracks.len())
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn push(&mut self, track: Arc<Track>) {
        self.tracks.push(track);
    }
}

impl Default for PlayQueue {
    fn default() -> Self {
        Self::new(vec![])
    }
}