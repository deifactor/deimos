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
