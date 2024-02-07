use std::{
    fs::{self, File},
    io,
    ops::{Deref, DerefMut},
};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use deimos::app::App;
use deimos::library::Library;
use directories::{ProjectDirs, UserDirs};
use eyre::{eyre, Result};
use log::debug;
use ratatui::{backend::CrosstermBackend, Terminal};

use tokio_stream::StreamExt;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Causes deimos to rescan the library from disk, overwriting the existing one.
    #[arg(long)]
    rescan_library: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    // when running with backtrace capture enabled, constructing the first error variant in a
    // program is more expensive (on the order of milliseconds). see
    // https://github.com/eyre-rs/color-eyre/issues/148.
    let _ = eyre::eyre!("unused");
    let project_dirs = ProjectDirs::from("ai", "ext0l", "deimos").unwrap();
    let args = Args::parse();

    // set up logging
    let log_target = project_dirs.data_local_dir().join("deimos.log");
    fs::create_dir_all(log_target.parent().unwrap())?;
    env_logger::builder()
        .target(env_logger::Target::Pipe(Box::new(File::create(log_target)?)))
        .init();

    // load library
    let cache_path = project_dirs.cache_dir().join("library.json");
    let library = if args.rescan_library {
        Err(eyre!("forcing rescan because of --rescan-library"))
    } else {
        Library::load(&cache_path)
    };
    let library = library.or_else(|_| {
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

    let mut terminal = AppTerminal::new()?;
    app.run(EventStream::new().filter_map(|ev| ev.ok()), terminal.deref_mut()).await?;

    Ok(())
}

/// Wrapper around a [`Terminal`] that automatically sets it up and restores it.
struct AppTerminal(Terminal<CrosstermBackend<io::Stdout>>);

impl Deref for AppTerminal {
    type Target = Terminal<CrosstermBackend<io::Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AppTerminal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AppTerminal {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let original_hook = std::panic::take_hook();

        // during panic, Drop gets called *after* the panic handler. restoring the terminal twice
        // is harmless, so we don't need to track if this was already called.
        std::panic::set_hook(Box::new(move |panic| {
            AppTerminal::restore_terminal();
            original_hook(panic);
        }));

        Ok(Self(terminal))
    }

    fn restore_terminal() {
        disable_raw_mode().unwrap();
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
    }
}

impl Drop for AppTerminal {
    fn drop(&mut self) {
        AppTerminal::restore_terminal()
    }
}
