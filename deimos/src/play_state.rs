use std::time::Duration;

use crate::library::Track;

/// The player's current state: what song are we playing, and how far in it are we?
#[derive(Debug, PartialEq, Eq)]
pub struct PlayState {
    /// How far into the current song we are.
    pub timestamp: Duration,
    /// Index into the current `Playlist`.
    pub track_index: usize,
}

/// All the tracks that are currently playing.
pub struct PlayQueue {
    pub tracks: Vec<Track>,
    /// This is `None` either before we've started playing or after the last song finishes.
    current: Option<PlayState>,
}

impl PlayQueue {
    pub fn current_track(&self) -> Option<&Track> {
        self.current.map(|c| &self.tracks[c.track_index])
    }

    pub fn current(&self) -> Option<&PlayState> {
        self.current.as_ref()
    }

    /// Advances to the next track. If there's no current track, moves to the first track. If the
    /// current track is the last track, clears the current track (since you're off the end of the
    /// queue).
    pub fn next_track(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        match self.current.map(|c| c.track_index) {
            Some(index) if index == self.tracks.len() - 1 => self.current = None,
            Some(index) => todo!(),
            None => todo!(),
        }
    }

    pub fn stop(&mut self) {
        self.current = None;
    }
}
