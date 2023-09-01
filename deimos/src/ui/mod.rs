pub mod artist_album_list;
pub mod now_playing;
pub mod search;
pub mod spectrogram;
pub mod track_list;

use crossterm::event::KeyCode;
use ratatui::{
    backend::CrosstermBackend,
    style::{Color, Modifier, Style},
};
use std::io::Stdout;

use crate::action::Action;

#[derive(Debug, Default)]
pub struct Ui {
    pub theme: Theme,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub focused_border: Style,
    pub unfocused_border: Style,
    pub section_header: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            focused_border: Style::default().fg(Color::Blue),
            unfocused_border: Default::default(),
            section_header: Style::default()
                .bg(Color::Rgb(0, 0, 60))
                .add_modifier(Modifier::BOLD | Modifier::ITALIC),
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
    #[allow(unused_variables)]
    fn handle_keycode(&mut self, keycode: KeyCode) -> Option<Action> {
        None
    }
}
