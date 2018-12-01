#![feature(nll)]
use failure;
use std::io;
use termion::raw::IntoRawMode;
use tui;
use tui::layout::Rect;
use tui::widgets;
use tui::widgets::Widget;

struct App {
    size: Rect,
}

fn main() -> Result<(), failure::Error> {
    let stdout = io::stdout().into_raw_mode()?;
    let backend = tui::backend::TermionBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    let app = App {
        size: terminal.size()?,
    };
    println!("{}", termion::clear::All);
    loop {
        terminal.draw(|mut f| {
            widgets::Block::default()
                .title("Now Playing")
                .borders(widgets::Borders::ALL)
                .render(&mut f, app.size);
        })?;
    }
    Ok(())
}
