use crate::parse;
use maplit::hashset;
use std::collections::HashSet;

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
