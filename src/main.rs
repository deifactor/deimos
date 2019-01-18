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

struct App {
    size: layout::Rect,
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
    let mut app = App {
        size: terminal.size()?,
    };
    terminal.hide_cursor()?;

    let receiver = events::EventReceiver::new(events::Config::default());
    loop {
        let size = terminal.size()?;
        if size != app.size {
            terminal.resize(size)?;
            app.size = size;
        }
        let song = conn.currentsong().expect("failed to get song");
        let status = conn.status().expect("failed to get status");
        let queue = conn.queue().expect("failed to get queue");
        let pos = song.as_ref().and_then(|song| Some(song.place?.pos));
        terminal.draw(|mut f| {
            let layout = layout::Layout::default()
                .direction(layout::Direction::Vertical)
                .constraints(vec![Constraint::Min(4), Constraint::Length(1)])
                .split(app.size);
            let mut now_playing = widgets::NowPlaying::new(
                song,
                status.elapsed,
                status.state,
                config.format.now_playing.clone(),
            );
            now_playing.render(&mut f, layout[1]);

            let mut queue_block = tui_widgets::Block::default()
                .title("Queue")
                .borders(tui_widgets::Borders::ALL);
            queue_block.render(&mut f, layout[0]);
            widgets::Queue::new(queue, pos, config.format.playlist_song.clone())
                .render(&mut f, queue_block.inner(layout[0]));
        })?;
        if let events::Event::Input(termion::event::Event::Key(termion::event::Key::Char('q'))) =
            receiver.next()?
        {
            break;
        }
    }
    Ok(())
}
