mod app;
mod audio;
mod library;
mod library_panel;
mod ui;

use std::{
    fs::{self, File},
    io, panic,
};

use app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use directories::{ProjectDirs, UserDirs};
use eyre::Result;
use library::Library;
use log::debug;
use ratatui::{backend::CrosstermBackend, Terminal};

use tokio_stream::StreamExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    // when running with backtrace capture enabled, constructing the first error variant in a
    // program is more expensive (on the order of milliseconds). see
    // https://github.com/eyre-rs/color-eyre/issues/148.
    let _ = eyre::eyre!("unused");
    let project_dirs = ProjectDirs::from("ai", "ext0l", "deimos").unwrap();

    // set up logging
    let log_target = project_dirs.data_local_dir().join("deimos.log");
    fs::create_dir_all(log_target.parent().unwrap())?;
    env_logger::builder()
        .target(env_logger::Target::Pipe(Box::new(File::create(
            log_target,
        )?)))
        .init();

    // load library
    let cache_path = project_dirs.cache_dir().join("library.json");
    let library = Library::load(&cache_path).or_else(|_| {
        let library_path = UserDirs::new().unwrap().home_dir().join("music");
        debug!(
            "Library not found at {}, rescanning {}",
            cache_path.display(),
            library_path.display()
        );
        let library = Library::scan(&library_path)?;
        fs::create_dir_all(cache_path.parent().unwrap())?;
        library.save(&cache_path)?;
        eyre::Ok(library)
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
