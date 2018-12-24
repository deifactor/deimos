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

/// Converts the string specified in the pest grammar into a color. Panics on an
/// invalid color.
fn parse_color(s: &str) -> Color {
    match s {
        "red" => Color::Red,
        "black" => Color::Black,
        "white" => Color::White,
        _ => panic!("bad parse color {}", s),
    }
}

/// Describes the foreground color, background color, and any additional
/// modifications (inverse, bold, etc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    foreground: Option<Color>,
    modifiers: Vec<Modifier>,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            foreground: None,
            modifiers: vec![],
        }
    }
}

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct MimiParser;

/// Builds a `Style` from the pair corresponding to a `style` rule.
fn build_style(style: pest::iterators::Pair<Rule>) -> Style {
    assert_eq!(style.as_rule(), Rule::style);
    let mut foreground = None;
    for attribute in style.into_inner() {
        match attribute.as_rule() {
            Rule::fg_color => foreground = Some(parse_color(attribute.as_str())),
            _ => panic!("Unexpected pair {:?}", attribute),
        }
    }
    Style {
        foreground,
        modifiers: vec![],
    }
}

/// Parses the format string into an output suitable for transformation via
/// mimi's formatting methods. In the `Err` cse, the value can be
/// Display-formatted for a nice, user-readable error message.
///
/// On success, the root is guaranteed to be a `Node::Formatted` variant with
/// `Style::default()` as its style.
pub fn parse(input: &str) -> Result<Node, pest::error::Error<Rule>> {
    let tokens = MimiParser::parse(Rule::format_string_entire, input)?;
    Ok(Node::Formatted {
        style: Style::default(),
        children: build_nodes(tokens),
    })
}

fn build_nodes(pairs: pest::iterators::Pairs<Rule>) -> Vec<Node> {
    pairs
        .filter_map(|pair| match pair.as_rule() {
            Rule::literal => Some(Node::Literal(pair.as_str())),
            Rule::variable => Some(Node::Variable(pair.into_inner().next().unwrap().as_str())),
            Rule::styled => Some({
                let mut pairs = pair.into_inner();
                let style = build_style(pairs.next().unwrap());
                let children = build_nodes(pairs);
                Node::Formatted { style, children }
            }),
            Rule::EOI => None,
            _ => panic!("Unexpected pair {:?}", pair),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn children(input: &str) -> Vec<Node> {
        let result = parse(input);
        if let Ok(Node::Formatted { children, .. }) = result {
            return children;
        } else {
            panic!("bad parse result {:?}", result);
        }
    }

    #[test]
    fn no_identifier() {
        assert!(parse("foo$").is_err());
        assert!(parse("$ ").is_err());
    }
    #[test]
    fn literal_and_variable() {
        assert_eq!(
            children("foo$bar"),
            vec![Node::Literal("foo"), Node::Variable("bar")]
        )
    }
    #[test]
    fn variable_then_literal() {
        assert_eq!(
            children("$foo!bar"),
            vec![Node::Variable("foo"), Node::Literal("!bar")],
        )
    }
    #[test]
    fn consecutive_variables() {
        assert_eq!(
            children("$foo$bar"),
            vec![Node::Variable("foo"), Node::Variable("bar")],
        )
    }

    #[test]
    fn format_string() {
        let style = Style {
            foreground: Some(Color::Red),
            modifiers: vec![],
        };
        assert_eq!(
            children("%[red]{text}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("text")],
            }],
        );
    }
}
