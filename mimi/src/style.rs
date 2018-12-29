/// Functionality for colors, modifiers (bold/underline), etc.
use std::collections::HashSet;
use termion;

/// Any formatting information that isn't foreground or background color.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Modifier {
    Bold,
    Underline,
}

impl Modifier {
    /// The corresponding ANSI control code.
    fn ansi(self) -> String {
        match self {
            Modifier::Bold => format!("{}", termion::style::Bold),
            Modifier::Underline => format!("{}", termion::style::Underline),
        }
    }
}

impl From<Modifier> for tui::style::Modifier {
    fn from(modifier: Modifier) -> tui::style::Modifier {
        match modifier {
            Modifier::Bold => tui::style::Modifier::Bold,
            Modifier::Underline => tui::style::Modifier::Underline,
        }
    }
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

impl Color {
    fn termion(self) -> Box<termion::color::Color> {
        match self {
            Color::Reset => Box::new(termion::color::Reset),
            Color::Black => Box::new(termion::color::Black),
            Color::White => Box::new(termion::color::White),
            Color::Red => Box::new(termion::color::Red),
            Color::Green => Box::new(termion::color::Green),
            Color::Yellow => Box::new(termion::color::Yellow),
            Color::Blue => Box::new(termion::color::Blue),
            Color::Magenta => Box::new(termion::color::Magenta),
            Color::Cyan => Box::new(termion::color::Cyan),
        }
    }
}

impl From<Color> for tui::style::Color {
    fn from(color: Color) -> tui::style::Color {
        match color {
            Color::Reset => tui::style::Color::Reset,
            Color::Black => tui::style::Color::Black,
            Color::White => tui::style::Color::White,
            Color::Red => tui::style::Color::Red,
            Color::Green => tui::style::Color::Green,
            Color::Yellow => tui::style::Color::Yellow,
            Color::Blue => tui::style::Color::Blue,
            Color::Magenta => tui::style::Color::Magenta,
            Color::Cyan => tui::style::Color::Cyan,
        }
    }
}

/// Describes the foreground color, background color, and any additional
/// modifications (inverse, bold, etc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub modifiers: HashSet<Modifier>,
}

impl From<Style> for tui::style::Style {
    fn from(style: Style) -> tui::style::Style {
        assert!(
            style.modifiers.len() <= 1,
            "tui can't stack modifiers, got {:?}",
            style.modifiers
        );
        tui::style::Style {
            fg: style.foreground.unwrap_or(Color::Reset).into(),
            bg: style.background.unwrap_or(Color::Reset).into(),
            modifier: style
                .modifiers
                .into_iter()
                .next()
                .map_or(tui::style::Modifier::Reset, |m| m.into()),
        }
    }
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

impl Style {
    /// Combines the two styles. If `other` has a foreground color specified,
    /// the combined style uses that color; else, `self`'s color is used. The
    /// same logic applies to background colors. The modifier sets are unioned.
    ///
    /// This operation is associative (i.e., `foo.combine(bar.combine(baz)) ==
    /// (foo.combine(bar)).combine(baz)` but not commutative (i.e.,
    /// `foo.combine(bar) != bar.combine(foo)`), and
    /// `foo.combine(Style::default()) == Style::default().combine(foo) == foo`.
    /// If you're a math nerd, `Style::combine` forms a monoid.
    pub fn combine(&self, other: &Style) -> Style {
        Style {
            foreground: other.foreground.or(self.foreground),
            background: other.background.or(self.background),
            modifiers: &other.modifiers | &self.modifiers,
        }
    }

    /// An ANSI control code sequence that will cause text to be formatted in
    /// the given style. This assumes that the old state has no foreground
    /// color, no background color, etc.
    pub fn ansi(&self) -> String {
        let mut s = "".to_owned();
        if let Some(color) = self.foreground {
            s.push_str(&format!("{}", termion::color::Fg(&*color.termion())));
        }
        if let Some(color) = self.background {
            s.push_str(&format!("{}", termion::color::Bg(&*color.termion())));
        }
        for modifier in &self.modifiers {
            s.push_str(&modifier.ansi())
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_override() {
        let red_foreground = Style {
            foreground: Some(Color::Red),
            ..Style::default()
        };
        let green_background = Style {
            background: Some(Color::Green),
            ..Style::default()
        };

        assert_eq!(
            red_foreground.combine(&green_background),
            Style {
                foreground: Some(Color::Red),
                background: Some(Color::Green),
                ..Style::default()
            }
        );
        assert_eq!(
            red_foreground.combine(&green_background),
            green_background.combine(&red_foreground)
        )
    }
}
