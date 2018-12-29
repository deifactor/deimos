use crate::parse;
use crate::parse::Node;
use crate::style::Style;
use maplit::hashset;
use std::collections::{HashMap, HashSet};
use std::iter;

#[derive(Clone, Debug)]
pub struct Formatter {
    root: parse::Node,
    keys: HashSet<String>,
}

#[derive(Clone, Debug)]
pub enum ParseFormatterError {
    FormatStringError(pest::error::Error<parse::Rule>),
}

impl Formatter {
    /// The names of all the variables in the format string.
    pub fn keys(&self) -> &HashSet<String> {
        &self.keys
    }

    fn ansi(&self, values: &HashMap<String, String>) -> String {
        styled_leaves(&self.root, values, Style::default())
            .map(|(text, style)| format!("{}{}{}", style.ansi(), text, termion::style::Reset))
            .collect()
    }
}

/// Gets the name of each variable inside the node, recursively.
fn get_keys(node: &parse::Node) -> HashSet<String> {
    match node {
        parse::Node::Literal(_) => HashSet::new(),
        parse::Node::Variable(key) => hashset![key.clone()],
        parse::Node::Formatted { children, .. } => children.iter().flat_map(get_keys).collect(),
    }
}

impl std::str::FromStr for Formatter {
    type Err = ParseFormatterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match parse::parse(s) {
            Ok(root) => {
                let keys = get_keys(&root);
                Ok(Formatter { root, keys })
            }
            Err(err) => Err(ParseFormatterError::FormatStringError(err)),
        }
    }
}

/// Yields the text of each leaf node under `root` with variables substituted by
/// looking up in `values` and using `base` as the base of each style.
///
/// Returns an iterator over (text, style) pairs. We do *not* guarantee that the
/// representation is minimal (in that it's possible for there to be adjacent
/// pairs with identical styles).
fn styled_leaves(
    root: &parse::Node,
    values: &HashMap<String, String>,
    base: Style,
) -> Box<Iterator<Item = (String, Style)>> {
    match root {
        Node::Literal(s) => Box::new(iter::once((s.clone(), base.clone()))),
        Node::Variable(key) => Box::new(iter::once((values[key].clone(), base.clone()))),
        Node::Formatted { style, children } => Box::new(
            children
                .iter()
                .flat_map(|child| styled_leaves(child, values, base.combine(style)))
                .collect::<Vec<_>>()
                .into_iter(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod keys {
        use super::*;

        #[test]
        fn nested() {
            assert_eq!(
                "$foo %[red]{$bar %[black]{$baz} %[white]{$quux}}"
                    .parse::<Formatter>()
                    .unwrap()
                    .keys(),
                &hashset![
                    "foo".to_owned(),
                    "bar".to_owned(),
                    "baz".to_owned(),
                    "quux".to_owned()
                ]
            )
        }

        #[test]
        fn repeated_variable() {
            assert_eq!(
                "$foo $foo".parse::<Formatter>().unwrap().keys(),
                &hashset!["foo".to_owned()]
            )
        }

        #[test]
        fn none() {
            assert_eq!(
                "foo bar %[red]{baz}".parse::<Formatter>().unwrap().keys(),
                &hashset![]
            )
        }
    }
}
