# mimi, a simple markup language for TUIs

`mimi` is a library for allowing the user to control how part of a terminal
program is formatted. The main usecase is for the `catgirl` command-line `mpd`
client, but of course other uses are welcome.

## Syntax

Variables are included using shell-like `$foo` syntax. Variable names can
contain `a-zA-Z0-9_` (ASCII-only). There's currently no way to do something like
`${foo}bar` to end the name of a variable early.

A styled section looks like `%[bold]{blah $foo blah}`. The style information
goes between the square brackets. Valid style names are:
* the colors `black`, `white`, `red`, `green`, `yellow`, `blue`, `magenta`,
  `cyan`, which indicate the color of the corresponding text.
* any color with `bg_` prefixed (for example, `bg_yellow`), which sets the
  background color.
* `bold` and `underline`.

You can have multiple styles in a style section, so `%[bold, red, bg_blue]{foo
bar baz}` is valid, if eye-searing. Style sections can nest.

## Output

Mimi has support for outputting xterm-compatible ANSI codes using
[termion](https://crates.io/crates/termion), and if the `to_tui` feature is
enabled (it's disabled by default), you'll be able to call `style.into()` to get
an instance of
[tui::style::Style](https://docs.rs/tui/0.3.0/tui/style/struct.Style.html).


## What's in a name?

'Nekomimi' is the Japanese word for 'person with cat ears', with 'neko' meaning
'cat' and 'mimi' meaning 'ears'.
