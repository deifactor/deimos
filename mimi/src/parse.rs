/// Functionality for parsing a format string into the internal AST-ish representation mimi uses.
use crate::style::{Color, Modifier, Style};
use pest::Parser;

/// A node in the parse tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// A textual literal.
    Literal(String),
    /// A variable whose name is given by the string.
    Variable(String),
    Formatted {
        style: Style,
        children: Vec<Node>,
    },
}

/// Converts the string specified in the pest grammar into a modifier. Panics on
/// an invalid modifier.
fn parse_modifier(s: &str) -> Modifier {
    match s {
        "bold" => Modifier::Bold,
        "underline" => Modifier::Underline,
        _ => panic!("bad modifier {}", s),
    }
}

/// Converts the string specified in the pest grammar into a color. Panics on an
/// invalid color.
fn parse_color(s: &str) -> Color {
    match s {
        "black" => Color::Black,
        "white" => Color::White,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "light_black" => Color::LightBlack,
        "light_white" => Color::LightWhite,
        "light_red" => Color::LightRed,
        "light_green" => Color::LightGreen,
        "light_yellow" => Color::LightYellow,
        "light_blue" => Color::LightBlue,
        "light_magenta" => Color::LightMagenta,
        "light_cyan" => Color::LightCyan,
        _ => panic!("bad parse color {}", s),
    }
}

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct MimiParser;

/// Builds a `Style` from the pair corresponding to a `style` rule.
fn build_style(style: pest::iterators::Pair<Rule>) -> Style {
    assert_eq!(style.as_rule(), Rule::style);
    let mut built = Style::default();
    for attribute in style.into_inner() {
        match attribute.as_rule() {
            Rule::fg_color => built.foreground = Some(parse_color(attribute.as_str())),
            Rule::bg_color => {
                built.background =
                    Some(parse_color(attribute.into_inner().next().unwrap().as_str()))
            }
            Rule::modifier => {
                built.modifiers.insert(parse_modifier(attribute.as_str()));
            }
            _ => panic!("Unexpected pair {:?}", attribute),
        }
    }
    built
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
            Rule::literal => Some(Node::Literal(unescape_literal(pair))),
            Rule::variable => Some(Node::Variable(
                pair.into_inner().next().unwrap().as_str().to_owned(),
            )),
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

/// Takes the `Pair` corresponding to the `literal` rule and removes any
/// included escape sequences.
fn unescape_literal(pair: pest::iterators::Pair<Rule>) -> String {
    pair.into_inner()
        .map(|pair| match pair.as_rule() {
            Rule::raw_literal => pair.as_str(),
            Rule::needs_escape => pair.as_str(),
            _ => panic!("Unexpected pair {:?}", pair),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
            vec![Node::Literal("foo".into()), Node::Variable("bar".into())]
        )
    }

    #[test]
    fn variable_then_literal() {
        assert_eq!(
            children("$foo!bar"),
            vec![Node::Variable("foo".into()), Node::Literal("!bar".into())],
        )
    }

    #[test]
    fn consecutive_variables() {
        assert_eq!(
            children("$foo$bar"),
            vec![Node::Variable("foo".into()), Node::Variable("bar".into())],
        )
    }

    #[test]
    fn braced_variable_name() {
        assert_eq!(
            children("${foo}bar"),
            vec![Node::Variable("foo".into()), Node::Literal("bar".into())]
        )
    }

    #[test]
    fn square_braces_dont_need_escape() {
        assert_eq!(children("[foo]"), vec![Node::Literal("[foo]".into())])
    }

    #[test]
    fn curly_braces_need_escape() {
        assert!(parse("{foo}").is_err());
    }

    #[test]
    fn escaped_variable() {
        assert_eq!(children("\\$foo"), vec![Node::Literal("$foo".into())])
    }

    #[test]
    fn escaped_closing_brace() {
        let style = Style {
            foreground: Some(Color::Red),
            ..Style::default()
        };
        assert_eq!(
            children("%[red]{foo\\}bar}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("foo}bar".into())]
            }]
        );
    }

    #[test]
    fn format_string() {
        let style = Style {
            foreground: Some(Color::Red),
            ..Style::default()
        };
        assert_eq!(
            children("%[red]{text}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("text".into())],
            }],
        );
    }

    #[test]
    fn background() {
        let style = Style {
            background: Some(Color::White),
            ..Style::default()
        };
        assert_eq!(
            children("%[bg_white]{text}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("text".into())]
            }]
        );
    }

    #[test]
    fn light_color() {
        let style = Style {
            foreground: Some(Color::LightRed),
            background: Some(Color::LightGreen),
            ..Style::default()
        };
        assert_eq!(
            children("%[light_red, bg_light_green]{foo}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("foo".into())]
            }])
    }

    #[test]
    fn multiple_colors() {
        let style = Style {
            foreground: Some(Color::Black),
            ..Style::default()
        };
        assert_eq!(
            children("%[red, black]{text}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("text".into())]
            }]
        );
    }

    #[test]
    fn attribute_spaces() {
        let style = Style {
            foreground: Some(Color::Black),
            background: Some(Color::White),
            ..Style::default()
        };
        assert_eq!(
            children("%[     bg_white,    black   ]{text}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("text".into())]
            }]
        );
    }

    #[test]
    fn modifiers() {
        let mut modifiers = HashSet::new();
        modifiers.insert(Modifier::Bold);
        let style = Style {
            modifiers,
            ..Style::default()
        };
        assert_eq!(
            children("%[bold]{text}"),
            vec![Node::Formatted {
                style,
                children: vec![Node::Literal("text".into())]
            }]
        );
    }

}
