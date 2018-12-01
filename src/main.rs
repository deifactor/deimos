#![feature(nll)]
mod events;
mod widgets;

use failure;
use mpd;
use std::io;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui;
use tui::layout::Rect;
use tui::widgets as tui_widgets;
use tui::widgets::Widget;

struct App {
    size: Rect,
}

fn main() -> Result<(), failure::Error> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = termion::input::MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = tui::backend::TermionBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    let mut app = App {
        size: terminal.size()?,
    };
    terminal.hide_cursor()?;

    let mut conn = mpd::Client::connect("127.0.0.1:6600").expect("failed to connect to MPD");

    let receiver = events::EventReceiver::new(events::Config::default());
    loop {
        let size = terminal.size()?;
        if size != app.size {
            terminal.resize(size)?;
            app.size = size;
        }
        let song = conn.currentsong().expect("failed to get song");
        let status = conn.status().expect("failed to get status");
        terminal.draw(|mut f| {
            let mut block = tui_widgets::Block::default().borders(tui_widgets::Borders::ALL);

            let mut now_playing =
                widgets::now_playing::NowPlaying::new(song, status.elapsed, status.state)
                    .block(block);
            now_playing.render(&mut f, app.size);
        })?;
        match receiver.next()? {
            events::Event::Input(termion::event::Event::Key(termion::event::Key::Char('q'))) => {
                break
            }
            _ => {}
        }
    }
    Ok(())
}
