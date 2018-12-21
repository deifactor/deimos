/// Functionality for parsing a format string into the internal AST-ish representation mimi uses.

/// A node in the parse tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// A textual literal.
    Text(String),
    /// A variable whose name is given by the string.
    Variable(String),
    Formatted {
        style: Style,
        children: Vec<Node>,
    },
}

/// Any formatting information that isn't foreground or background color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Modifier {
    Bold,
}

/// Foreground or background color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    Reset,
    Black,
    White,
    Red,
}

/// Describes the foreground color, background color, and any additional
/// modifications (inverse, bold, etc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    foreground: Color,
    modifiers: Vec<Modifier>,
}
