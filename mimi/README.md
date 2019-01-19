[![Build Status](https://travis-ci.org/deifactor/catgirl.svg?branch=mistress)](https://travis-ci.org/deifactor/catgirl)
[![Crate version](https://img.shields.io/crates/v/mimi.svg)](https://crates.io/crates/mimi)

`mimi` is a library for allowing the user to control how part of a terminal
program is formatted. The main usecase is for the `catgirl` command-line `mpd`
client, but of course other uses are welcome.

![A demo of mimi formatting](/mimi/example.png?raw=true)

## Syntax

Variables are included using shell-like `$foo` syntax. Variable names can
contain `a-zA-Z0-9_` (ASCII-only). `${foo}bar` is valid syntax, and is parsed as
a variable named 'foo' followed immediately by the literal `bar`.

A styled section looks like `%[bold]{blah $foo blah}`. The style information
goes between the square brackets. Valid style names are:
* the colors `black`, `white`, `red`, `green`, `yellow`, `blue`, `magenta`,
  `cyan`, as well as `light_black`, `light_white`, etc., which indicate the
  color of the corresponding text.
* any color with `bg_` prefixed (for example, `bg_yellow`, `bg_light_blue`),
  which sets the background color.
* `reset` and `bg_reset`, which set the foreground/background color to the
  terminal's default.
* `bold`, `underline`, `reverse`. Note that if you have two `reverse` styles,
  they will *not* cancel each other out.

You can have multiple styles in a style section, so `%[bold, red, bg_blue]{foo
bar baz}` is valid, if eye-searing. Style sections can nest.

## Output

Mimi has support for outputting xterm-compatible ANSI codes using
[termion](https://crates.io/crates/termion), and if the `to_tui` feature is
enabled (it's disabled by default), you'll be able to call `style.into()` to get
an instance of
[tui::style::Style](https://docs.rs/tui/0.3.0/tui/style/struct.Style.html).


## Demo

The demo binary in `src/examples/demo.rs` allows you to play around with mimi formatting. Run it like

    cargo run --example demo -- -f "foo is %[bold]{\$foo}" foo=bar

## What's in a name?

'Nekomimi' is the Japanese word for 'person with cat ears', with 'neko' meaning
'cat' and 'mimi' meaning 'ears'.
