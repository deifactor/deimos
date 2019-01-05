use crate::parse;
use crate::parse::Node;
use crate::style::Style;
use maplit::hashset;
use std::collections::HashSet;
use std::{error, fmt, iter};

/// A `Formatter` takes a bunch of key/value pairs and interpolates them into a
/// mimi format string.
///
/// # Examples
#[derive(Clone, Debug)]
pub struct Formatter {
    root: parse::Node,
    keys: HashSet<String>,
}

/// An error that occurred while parsing a format string. The [`std::fmt::Display`]
/// implementation for `ParseFormatterError` is guaranteed to produce something
/// human-readable (i.e., not just dump a struct), but the format may change.
/// Currently it uses pest's errors.
#[derive(Clone, Debug)]
pub struct ParseFormatterError(pest::error::Error<parse::Rule>);

impl fmt::Display for ParseFormatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl error::Error for ParseFormatterError {
    fn description(&self) -> &str {
        "format string error"
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        Some(&self.0)
    }
}

impl Formatter {
    /// The names of all the variables in the format string.
    pub fn keys(&self) -> &HashSet<String> {
        &self.keys
    }

    /// Formats the given values using ANSI terminal codes.
    pub fn ansi<'a, M: std::ops::Index<&'a str, Output = String>>(&'a self, values: &M) -> String {
        self.spans(values)
            .map(|(text, style)| format!("{}{}{}", style.ansi(), text, termion::style::Reset))
            .collect()
    }

    /// Yields the text of each leaf node under `root` with variables
    /// substituted by looking up in `values` and using `base` as the base of
    /// each style. Combined with a function that can take a `String` and a
    /// `Style` and format them, this gives you the ability to work with
    /// arbitrary output formats.
    ///
    /// Returns an iterator over (text, style) pairs. We do *not* guarantee that the
    /// representation is minimal (in that it's possible for there to be adjacent
    /// pairs with identical styles).
    pub fn spans<'a, M: std::ops::Index<&'a str, Output = String>>(
        &'a self,
        values: &M,
    ) -> Box<dyn Iterator<Item = (String, Style)>> {
        Formatter::spans_impl(&self.root, values, Style::default())
    }

    fn spans_impl<'a, M: std::ops::Index<&'a str, Output = String>>(
        root: &'a parse::Node,
        values: &M,
        base: Style,
    ) -> Box<dyn Iterator<Item = (String, Style)>> {
        match root {
            Node::Literal(s) => Box::new(iter::once((s.clone(), base.clone()))),
            Node::Variable(key) => Box::new(iter::once((values[key].clone(), base.clone()))),
            Node::Formatted { style, children } => Box::new(
                children
                    .iter()
                    .flat_map(|child| Formatter::spans_impl(child, values, base.combine(style)))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
        }
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
            Err(err) => Err(ParseFormatterError(err)),
        }
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
