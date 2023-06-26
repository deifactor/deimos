mod library;

use cursive::{
    theme::Palette,
    views::{Dialog, TextView},
};

fn palette() -> Palette {
    use cursive::theme::{Color::*, PaletteColor::*};
    let mut palette = Palette::default();
    palette.extend(vec![(Background, TerminalDefault)]);
    palette
}

fn main() {
    library::find_music("/home/vector/music").unwrap();
}
