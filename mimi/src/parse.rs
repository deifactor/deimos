/// Functionality for parsing a format string into the internal AST-ish representation mimi uses.
use pest::Parser;

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

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct MimiParser;

/// Parses the format string into an output suitable for transformation via
/// mimi's formatting methods. In the `Err` case, the value can be
/// Display-formatted for a nice, user-readable error message.
pub fn parse(input: &str) -> Result<Vec<Node>, pest::error::Error<Rule>> {
    let tokens = MimiParser::parse(Rule::format_string, input)?
        .filter_map(|pair| match pair.as_rule() {
            Rule::literal => Some(Node::Literal(pair.as_str())),
            Rule::variable => Some(Node::Variable(pair.into_inner().next().unwrap().as_str())),
            Rule::EOI => None,
            _ => panic!("Unexpected pair {:?}", pair),
        })
        .collect();
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_variable() {
        assert_eq!(
            parse("foo$bar"),
            Ok(vec![Node::Literal("foo"), Node::Variable("bar")])
        )
    }
    #[test]
    fn variable_then_literal() {
        assert_eq!(
            parse("$foo!bar"),
            Ok(vec![Node::Variable("foo"), Node::Literal("!bar")])
        )
    }
    #[test]
    fn consecutive_variables() {
        assert_eq!(
            parse("$foo$bar"),
            Ok(vec![Node::Variable("foo"), Node::Variable("bar")])
        )
    }

    #[test]
    fn no_identifier() {
        assert!(parse("foo$").is_err());
        assert!(parse("$ ").is_err());
    }
}
