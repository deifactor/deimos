pub mod artist_album_list;
pub mod now_playing;
pub mod search;
pub mod spectrogram;
pub mod track_list;

use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Style},
    Frame,
};
use std::io::Stdout;

use crate::action::Action;

#[derive(Debug, Default)]
pub struct Ui {
    pub theme: Theme,
    pub focus: FocusTarget,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub focused_border: Style,
    pub unfocused_border: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            focused_border: Style::default().fg(Color::Blue),
            unfocused_border: Default::default(),
        }
    }
}

impl Ui {
    pub fn border(&self, is_focused: bool) -> Style {
        if is_focused {
            self.theme.focused_border
        } else {
            self.theme.unfocused_border
        }
    }

    pub fn is_focused(&self, target: FocusTarget) -> bool {
        self.focus == target
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum FocusTarget {
    #[default]
    ArtistAlbumList,
    TrackList,
}

impl FocusTarget {
    pub fn next(self) -> Self {
        use FocusTarget::*;
        match self {
            ArtistAlbumList => TrackList,
            TrackList => ArtistAlbumList,
        }
    }
}

/// The type of the ratatui backend we use. We use a fixed backend so that [`Component`] doesn't have any generics, making it object-safe.
pub type DeimosBackend = CrosstermBackend<Stdout>;

/// Generic component trait. Components are expected to contain their own state.
pub trait Component {
    /// Draw the component inside the given area of the frame.
    fn draw(&mut self, ui: &Ui, frame: &mut Frame<DeimosBackend>, area: Rect) -> Result<()>;

    #[allow(unused_variables)]
    fn handle_keycode(&mut self, keycode: KeyCode) -> Option<Action> {
        None
    }
}
