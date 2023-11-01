pub mod artist_album_list;
pub mod now_playing;
pub mod search;
pub mod spectrogram;
pub mod track_list;

use ratatui::style::{Color, Modifier, Style};

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
