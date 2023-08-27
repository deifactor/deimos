/// Functionality for colors, modifiers (bold/underline), etc.
use std::collections::HashSet;

/// Any formatting information that isn't foreground or background color.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum Modifier {
    Bold,
    Underline,
    Reverse,
}

impl Modifier {
    /// The corresponding ANSI control code.
    fn ansi(self) -> String {
        match self {
            Modifier::Bold => format!("{}", termion::style::Bold),
            Modifier::Underline => format!("{}", termion::style::Underline),
            Modifier::Reverse => format!("{}", termion::style::Invert),
        }
    }
}

/// Foreground or background color. Colors are parsed as `snake_case`, so
/// `light_red` becomes `LightRed`, etc.
///
/// The names `LightBlack` seems nonsensical, but is often rendered as a sort of
/// dark gray. Similarly, `White` is often a light gray, and `LightWhite` is a
/// 'true' white, brighter than the normal text color in white-on-black terminal schemes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Color {
    /// Indicates that the color should be the terminal's default
    /// foreground/background color. This may be black or white depending on the
    /// user's terminal theme.
    Reset,
    Black,
    White,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    LightBlack,
    LightWhite,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
}

impl Color {
    fn termion(self) -> Box<dyn termion::color::Color> {
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
            Color::LightBlack => Box::new(termion::color::LightBlack),
            Color::LightWhite => Box::new(termion::color::LightWhite),
            Color::LightRed => Box::new(termion::color::LightRed),
            Color::LightGreen => Box::new(termion::color::LightGreen),
            Color::LightYellow => Box::new(termion::color::LightYellow),
            Color::LightBlue => Box::new(termion::color::LightBlue),
            Color::LightMagenta => Box::new(termion::color::LightMagenta),
            Color::LightCyan => Box::new(termion::color::LightCyan),
        }
    }
}

/// Describes the foreground color, background color, and any additional
/// modifications (inverse, bold, etc).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Style {
    /// The color used to render the text. If `None`, uses whatever the
    /// terminal's default color is.
    pub foreground: Option<Color>,
    /// The color used to render the background. If `None`, uses whatever the
    /// terminal's default color is.
    pub background: Option<Color>,
    /// Any extra formatting information, such as bold/italic.
    pub modifiers: HashSet<Modifier>,
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

#[cfg(feature = "to_tui")]
impl From<Modifier> for tui::style::Modifier {
    fn from(modifier: Modifier) -> tui::style::Modifier {
        match modifier {
            Modifier::Bold => tui::style::Modifier::BOLD,
            Modifier::Underline => tui::style::Modifier::UNDERLINED,
            Modifier::Reverse => tui::style::Modifier::REVERSED,
        }
    }
}

#[cfg(feature = "to_tui")]
impl From<Color> for tui::style::Color {
    fn from(color: Color) -> tui::style::Color {
        match color {
            // tui's naming scheme is a bit weird for black/white, corresponding
            // more to what the color *looks* like.
            Color::Reset => tui::style::Color::Reset,
            Color::Black => tui::style::Color::Black,
            Color::White => tui::style::Color::Gray,
            Color::Red => tui::style::Color::Red,
            Color::Green => tui::style::Color::Green,
            Color::Yellow => tui::style::Color::Yellow,
            Color::Blue => tui::style::Color::Blue,
            Color::Magenta => tui::style::Color::Magenta,
            Color::Cyan => tui::style::Color::Cyan,
            Color::LightBlack => tui::style::Color::DarkGray,
            Color::LightWhite => tui::style::Color::White,
            Color::LightRed => tui::style::Color::LightRed,
            Color::LightGreen => tui::style::Color::LightGreen,
            Color::LightYellow => tui::style::Color::LightYellow,
            Color::LightBlue => tui::style::Color::LightBlue,
            Color::LightMagenta => tui::style::Color::LightMagenta,
            Color::LightCyan => tui::style::Color::LightCyan,
        }
    }
}

#[cfg(feature = "to_tui")]
impl From<Style> for tui::style::Style {
    fn from(style: Style) -> tui::style::Style {
        tui::style::Style {
            fg: Some(style.foreground.unwrap_or(Color::Reset).into()),
            bg: Some(style.background.unwrap_or(Color::Reset).into()),
            add_modifier: style
                .modifiers
                .into_iter()
                .fold(tui::style::Modifier::empty(), |a, b| a | b.into()),
            sub_modifier: tui::style::Modifier::empty(),
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
