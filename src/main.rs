mod config;
mod events;
mod widgets;

use failure;
use mpd;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use structopt::StructOpt;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui;
use tui::layout;
use tui::layout::Constraint;
use tui::widgets as tui_widgets;
use tui::widgets::Widget;

#[derive(StructOpt, Debug)]
#[structopt(name = "catgirl")]
struct Opt {
    /// The host to connect to.
    #[structopt(short = "h", long = "host", default_value = "localhost")]
    host: String,

    /// The port to connect on.
    #[structopt(short = "p", long = "port", default_value = "6600")]
    port: u16,
}

fn main() -> Result<(), failure::Error> {
    let opt = Opt::from_args();
    let config: config::Config = {
        let path = config::config_path().expect("Couldn't determine path to the config file");
        let mut buf = String::new();
        File::open(path)?.read_to_string(&mut buf)?;
        buf.parse()?
    };

    let mut conn =
        mpd::Client::connect((opt.host.as_str(), opt.port)).expect("failed to connect to MPD");

    let stdout = io::stdout().into_raw_mode()?;
    let stdout = termion::input::MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = tui::backend::TermionBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    let mut size = terminal.size()?;
    terminal.hide_cursor()?;

    let mut screen = widgets::app::Screen::Queue;

    let receiver = events::EventReceiver::new(events::Config::default());
    loop {
        {
            let new_size = terminal.size()?;
            if size != new_size {
                terminal.resize(new_size)?;
                size = new_size;
            }
        }
        let song = conn.currentsong().expect("failed to get song");
        let status = conn.status().expect("failed to get status");
        let queue = conn.queue().expect("failed to get queue");
        let pos = song.as_ref().and_then(|song| Some(song.place?.pos));
        let queue = widgets::Queue::new(queue, pos, config.format.playlist_song.clone());

        let album_artists = conn
            .list(&mpd::Term::Tag("AlbumArtist".into()), &mpd::Query::new())
            .expect("failed to list album artists");
        let album_tree = widgets::AlbumTree::new(album_artists);
        let now_playing = widgets::NowPlaying::new(
            song,
            status.elapsed,
            status.state,
            config.format.now_playing.clone(),
        );
        terminal
            .draw(|mut f| {
                let mut app = widgets::App::new(size, queue, album_tree, now_playing);
                app.screen = screen;
                app.render(&mut f, size)
            })
            .expect("failed to draw");
        if let Some(termion::event::Key::Char(c)) = receiver.next()?.key() {
            match c {
                'q' => break,
                '1' => screen = widgets::app::Screen::Queue,
                '2' => screen = widgets::app::Screen::Albums,
                _ => (),
            }
        }
    }
    Ok(())
}
