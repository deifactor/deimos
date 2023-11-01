mod app;
mod audio;
mod library;
mod library_panel;
mod ui;

use std::{io, panic, path::PathBuf};

use anyhow::Result;
use app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use library::Library;
use ratatui::{backend::CrosstermBackend, Terminal};

use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    let library_path = PathBuf::from("library.json");
    let library = Library::load(&library_path).or_else(|_| {
        let library = Library::scan(PathBuf::from("/home/vector/music"))?;
        let _ = library.save(&library_path)?;
        anyhow::Ok(library)
    })?;

    let app = App::new(library);

    // do this late as we can so that errors won't get mangled
    let terminal = prepare_terminal()?;

    app.run(EventStream::new().filter_map(|ev| ev.ok()), terminal)
        .await?;

    restore_terminal()?;

    Ok(())
}

/// Sets up the terminal for full-screen application mode.
///
/// This also installs a panic hook that will (call the original panic hook
/// and) clean up the terminal state, meaning that even a panic should leave
/// your terminal workable.
fn prepare_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let original = panic::take_hook();
    // we have to restore the terminal *before* the original handler since any
    // messages it prints will get eaten
    panic::set_hook(Box::new(move |panic| {
        restore_terminal().unwrap();
        original(panic);
    }));
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

/// Cleans up the terminal state. prepare_terminal() will call this on panic,
/// but you still need to manually call it too.
fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}
