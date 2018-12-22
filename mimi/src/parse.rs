/// Functionality for parsing a format string into the internal AST-ish representation mimi uses.
use nom::*;
use nom::types::CompleteStr;

/// A node in the parse tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node<'a> {
    /// A textual literal.
    Literal(&'a str),
    /// A variable whose name is given by the string.
    Variable(&'a str),
    Formatted {
        style: Style,
        children: Vec<Node<'a>>,
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

named!(
    literal<CompleteStr, Node>,
    map!(
        take_while!(|c| { c != '$' && c != '%' }),
        |text| { Node::Literal(text.0) }
    )
);

/// Parses a single variable, of the form $foobar. TODO: Support an expression
/// like ${foo}bar.
named!(
    variable<CompleteStr, Node>,
    preceded!(tag!("$"), map!(
        take_while1!(char::is_alphanumeric),
        |name| { Node::Variable(name.0) }
    ))
);

named!(
    format_string<CompleteStr, Vec<Node>>,
    many0!(alt!(variable | literal ))
);

#[cfg(test)]
mod tests {
    use super::*;

    mod variable {
        use super::*;
        #[test]
        fn alphanumeric_variable() {
            assert_eq!(
                variable(CompleteStr("$foo ")),
                Ok((CompleteStr(" "), Node::Variable("foo")))
            );
        }

        #[test]
        fn empty_variable() {
            assert!(variable(CompleteStr("$ ")).is_err())
        }
    }

    mod format_string {
        use super::*;
        #[test]
        fn literal_and_variable() {
            assert_eq!(
                format_string(CompleteStr("foo$bar")),
                Ok((
                    CompleteStr(""),
                    vec![Node::Literal("foo"), Node::Variable("bar")]
                ))
            )
        }
        #[test]
        fn variable_then_literal() {
            assert_eq!(
                format_string(CompleteStr("$foo!bar")),
                Ok((
                    CompleteStr(""),
                    vec![Node::Variable("foo"), Node::Literal("!bar")]
                ))
            )
        }
        #[test]
        fn consecutive_variables() {
            assert_eq!(
                format_string(CompleteStr("$foo$bar")),
                Ok((
                    CompleteStr(""),
                    vec![Node::Variable("foo"), Node::Variable("bar")]
                ))
            )
        }
    }
}
