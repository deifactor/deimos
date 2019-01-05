//! This crate provides run-time string interpolation using key/value pairs,
//! oriented towards support for terminal applications. For documentation on the
//! format of mimi strings, see the README.md or the `parse` module.
//!
//! # Examples
//!
//! ```
//! use std::collections::HashMap;
//! let formatter: mimi::format::Formatter = "foo is %[red]{$foo}".parse().unwrap();
//! let mut values = HashMap::new();
//! values.insert("foo", "value".to_owned());
//! println!("{}", formatter.ansi(&values));
//! ```

#![warn(missing_docs)]
#[macro_use]
extern crate pest_derive;

pub mod format;
pub mod parse;
pub mod style;
