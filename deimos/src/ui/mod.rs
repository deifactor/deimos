pub mod artist_album_list;
pub mod now_playing;
pub mod search;
pub mod spectrogram;
pub mod track_list;

use anyhow::Result;
use crossterm::event::KeyCode;
use enum_iterator::Sequence;
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Style},
    Frame,
};
use std::io::Stdout;

use crate::action::Command;

#[derive(Debug, Default)]
pub struct Ui {
    pub theme: Theme,
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
    pub fn border(&self, state: ActiveState) -> Style {
        match state {
            ActiveState::Focused => self.theme.focused_border,
            ActiveState::Inactive => self.theme.unfocused_border,
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Sequence)]
pub enum FocusTarget {
    #[default]
    ArtistAlbumList,
    TrackList,
}

/// The type of the ratatui backend we use. We use a fixed backend so that [`Component`] doesn't have any generics, making it object-safe.
pub type DeimosBackend = CrosstermBackend<Stdout>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ActiveState {
    Focused,
    Inactive,
}

impl ActiveState {
    pub fn focused_if(cond: bool) -> Self {
        if cond {
            Self::Focused
        } else {
            Self::Inactive
        }
    }
}

/// Generic component trait. Components are expected to contain their own state.
pub trait Component {
    /// Draw the component inside the given area of the frame.
    fn draw(
        &mut self,
        state: ActiveState,
        ui: &Ui,
        frame: &mut Frame<DeimosBackend>,
        area: Rect,
    ) -> Result<()>;

    #[allow(unused_variables)]
    fn handle_keycode(&mut self, keycode: KeyCode) -> Option<Command> {
        None
    }
}
