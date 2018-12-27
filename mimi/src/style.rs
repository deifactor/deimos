/// Functionality for colors, modifiers (bold/underline), etc.
use std::collections::HashSet;

/// Any formatting information that isn't foreground or background color.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Modifier {
    Bold,
    Underline,
}

/// Foreground or background color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    Reset,
    Black,
    White,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
}

/// Describes the foreground color, background color, and any additional
/// modifications (inverse, bold, etc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub modifiers: HashSet<Modifier>,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            foreground: None,
            background: None,
            modifiers: HashSet::new(),
        }
    }
}
