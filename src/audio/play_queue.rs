use std::sync::Arc;

use mpris_server::LoopStatus;

use crate::library::Track;

#[derive(Debug)]
pub struct PlayQueue {
    index: Option<usize>,
    tracks: Vec<Arc<Track>>,
    loop_status: LoopStatus,
    shuffled: bool,
    original_order: Vec<Arc<Track>>,
}

impl PlayQueue {
    pub fn new(tracks: Vec<Arc<Track>>) -> Self {
        let original_order = tracks.clone();
        Self {
            index: None,
            tracks,
            loop_status: LoopStatus::None,
            shuffled: false,
            original_order,
        }
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

    pub fn shuffle(&self) -> bool {
        self.shuffled
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        if shuffle == self.shuffle() {
            return;
        }
        self.shuffled = shuffle;
        let current_track = self.current_track();
        if shuffle {
            fastrand::shuffle(&mut self.tracks);
        } else {
            self.tracks = self.original_order.clone();
        }
        let Some(current_track) = current_track else {
            return;
        };
        let new_index = self
            .tracks
            .iter()
            .position(|track| track.id == current_track.id)
            .expect("couldn't find track after shuffling");
        // If we just shuffled, we need to move the current track to the front.
        if shuffle {
            self.tracks.swap(0, new_index);
            self.index = Some(0);
        } else {
            self.index = Some(new_index);
        }
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
        self.original_order.push(Arc::clone(&track));
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

    // Longer queue used for shuffle-related tests.
    fn shuffle_test_queue() -> PlayQueue {
        let mut queue = PlayQueue::default();
        for i in 0..50 {
            queue.push(Arc::new(Track::test_track(i)));
        }
        queue
    }

    #[test]
    fn first_after_shuffle() {
        for _ in 0..10 {
            let mut queue = shuffle_test_queue();
            queue.set_current(Some(20));
            let track = queue.current_track();
            queue.set_shuffle(true);
            assert_eq!(
                queue.current_track(),
                track,
                "shuffling the queue should leave the track the same"
            );
            assert_eq!(
                queue.current(),
                Some(0),
                "shuffling the queue should move the current track to the front"
            );
        }
    }

    #[test]
    fn shuffle_unshuffle() {
        for _ in 0..10 {
            let mut queue = shuffle_test_queue();
            let original = queue.tracks.clone();
            queue.set_shuffle(true);
            queue.set_shuffle(false);
            assert_eq!(queue.tracks, original);
        }
    }

    #[test]
    fn order_after_unshuffle() {
        for _ in 0..10 {
            let mut queue = shuffle_test_queue();
            queue.set_shuffle(true);
            queue.set_current(Some(20));
            let track = queue.current_track();
            dbg!(&queue);
            queue.set_shuffle(false);
            dbg!(&queue);
            assert_eq!(
                queue.current_track(),
                track,
                "unshuffling the queue should leave the track the same"
            );
        }
    }
}
