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
    // Creates the cursive root - required for every application.
    let mut siv = cursive::default();
    siv.with_theme(|theme| theme.palette = palette());

    // Creates a dialog with a single "Quit" button
    siv.add_layer(
        Dialog::around(TextView::new("Hello Dialog!"))
            .title("Cursive")
            .button("Quit", |s| s.quit()),
    );

    // Starts the event loop.
    siv.run();
}
