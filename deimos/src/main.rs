mod config;

use std::io::{self, Read};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use structopt::StructOpt;

use tui::backend::CrosstermBackend;
use tui::widgets::{Block, Borders};
use tui::{self, Terminal};

#[derive(StructOpt, Debug)]
#[structopt(name = "deimos")]
struct Opt {
    /// The host to connect to.
    #[structopt(short = "h", long = "host", default_value = "localhost")]
    host: String,

    /// The port to connect on.
    #[structopt(short = "p", long = "port", default_value = "6600")]
    port: u16,
}

fn main() -> Result<(), failure::Error> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|f| {
        let size = f.size();
        let block = Block::default().title("nya").borders(Borders::ALL);
        f.render_widget(block, size);
    })?;

    // Start a thread to discard any input events. Without handling events, the
    // stdin buffer will fill up, and be read into the shell when the program exits.
    thread::spawn(|| loop {
        let _ = event::read();
    });

    thread::sleep(Duration::from_millis(5000));

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
