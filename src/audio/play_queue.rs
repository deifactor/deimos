use std::sync::Arc;

use mpris_server::LoopStatus;

use crate::library::Track;

#[derive(Debug)]
pub struct PlayQueue {
    index: Option<usize>,
    tracks: Vec<Arc<Track>>,
    loop_status: LoopStatus,
}

impl PlayQueue {
    pub fn new(tracks: Vec<Arc<Track>>) -> Self {
        Self { index: None, tracks, loop_status: LoopStatus::None }
    }

    /// Sets the tracks to the given list. Also clears the currently playing track, since in
    /// general there's nothing sensible to do there.
    pub fn set_tracks(&mut self, tracks: Vec<Arc<Track>>) {
        self.tracks = tracks;
        self.index = None;
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn current(&self) -> Option<usize> {
        self.index
    }

    pub fn set_current(&mut self, current: Option<usize>) {
        self.index = current;
    }

    pub fn loop_status(&self) -> LoopStatus {
        self.loop_status
    }

    pub fn set_loop_status(&mut self, loop_status: LoopStatus) {
        self.loop_status = loop_status;
    }

    pub fn current_track(&self) -> Option<Arc<Track>> {
        self.index.map(|i| Arc::clone(&self.tracks[i]))
    }

    /// Index of the previous track. `None` if this is the first track.
    pub fn previous(&self) -> Option<usize> {
        match self.loop_status {
            LoopStatus::None => self.index?.checked_sub(1),
            LoopStatus::Track => self.index,
            LoopStatus::Playlist => Some(self.index?.checked_sub(1).unwrap_or(self.len() - 1)),
        }
    }

    /// Index of the next track. `None` if this would go off the end.
    pub fn next(&self) -> Option<usize> {
        match self.loop_status {
            LoopStatus::None => self.index.map(|i| i + 1).filter(|i| *i < self.tracks.len()),
            LoopStatus::Track => self.index,
            LoopStatus::Playlist => {
                Some(self.index?.checked_add(1).map_or(0, |i| i % self.tracks.len()))
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_queue() -> PlayQueue {
        PlayQueue::new(vec![
            Arc::new(Track::test_track(0)),
            Arc::new(Track::test_track(1)),
            Arc::new(Track::test_track(2)),
        ])
    }

    #[test]
    fn test_next() {
        let mut queue = sample_queue();
        queue.set_current(None);
        assert_eq!(queue.next(), None, "next() on a stopped queue should return None");
        queue.set_current(Some(1));
        assert_eq!(queue.next(), Some(2));
        queue.set_current(Some(2));
        assert_eq!(queue.next(), None, "next() on the last track should return None");
    }

    #[test]
    fn test_previous() {
        let mut queue = sample_queue();
        queue.set_current(None);
        assert_eq!(queue.previous(), None, "previous() on a stopped queue should return None");
        queue.set_current(Some(1));
        assert_eq!(queue.previous(), Some(0));
        queue.set_current(Some(2));
        assert_eq!(queue.previous(), Some(1));
    }

    #[test]
    fn track_looping() {
        let mut queue = sample_queue();
        queue.set_current(Some(0));
        queue.set_loop_status(LoopStatus::Track);
        assert_eq!(
            queue.previous(),
            queue.current(),
            "previous track should be the same with track looping"
        );
        assert_eq!(
            queue.next(),
            queue.current(),
            "next track should be the same with track looping"
        );
    }
}
