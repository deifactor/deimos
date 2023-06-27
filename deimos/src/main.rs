mod library;
mod player;

use anyhow::Result;
use cursive::{
    theme::Palette,
    views::{Button, LinearLayout},
};
use library::initialize_db;
use player::Player;
use rodio::OutputStream;
use sqlx::Connection;

fn palette() -> Palette {
    use cursive::theme::{Color::*, PaletteColor::*};
    let mut palette = Palette::default();
    palette.extend(vec![(Background, TerminalDefault)]);
    palette
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut conn = initialize_db("songs.sqlite").await?;
    let count = sqlx::query!("SELECT COUNT(*) AS count FROM songs")
        .fetch_one(&mut conn)
        .await?
        .count;
    // only reinitialize db if there are no songs
    if count == 0 {
        conn.transaction(|conn| {
            Box::pin(async move { library::find_music("/home/vector/music", conn).await })
        })
        .await?;
    }

    let mut siv = cursive::default();
    siv.with_theme(|theme| theme.palette = palette());

    let (_stream, stream_handle) = OutputStream::try_default()?;
    let player = Player::new(stream_handle)?;

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
