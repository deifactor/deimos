mod library;
mod player;

use std::{fs::File, io::BufReader};

use anyhow::Result;
use cursive::{
    theme::Palette,
    views::{Button, LinearLayout},
};
use player::Player;
use rodio::{Decoder, OutputStream};

fn palette() -> Palette {
    use cursive::theme::{Color::*, PaletteColor::*};
    let mut palette = Palette::default();
    palette.extend(vec![(Background, TerminalDefault)]);
    palette
}

fn main() -> Result<()> {
    let song_path = library::find_music("/home/vector/music")?;
    let mut siv = cursive::default();
    siv.with_theme(|theme| theme.palette = palette());

    let (_stream, stream_handle) = OutputStream::try_default()?;
    let player = Player::new(stream_handle)?;
    let source = Decoder::new(BufReader::new(File::open(song_path)?))?;
    player.append(source);

    siv.add_layer(
        LinearLayout::horizontal()
            .child(Button::new("play", cc!(player, |_s| player.play())))
            .child(Button::new("pause", cc!(player, |_s| player.pause())))
            .child(Button::new("quit", |s| s.quit())),
    );

    siv.run();
    Ok(())
}

/// Utility macro for cloning some variables and moving them into a
/// closure. Write it like `cc!(some_var, || some_var.do_thing())`. This uses
/// the `,` separator since that makes it look 'function-like' enough that
/// autoformatters will behave properly.
#[macro_export]
macro_rules! cc {
    ($($n:ident),+, || $body:block) => (
        {
            $( let $n = $n.clone(); )+
            move || { $body }
        }
    );
    ($($n:ident),+, || $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move || $body
        }
    );
    ($($n:ident),+, |$($p:ident),+| $body:block) => (
        {
            $( let $n = $n.clone(); )+
            move |$($p),+| { $body }
        }
    );
    ($($n:ident),+, |$($p:ident),+| $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move |$($p),+| $body
        }
    );
}
