mod action;
mod app;
mod artist_album_list;
mod decoder;
mod library;
mod now_playing;
mod track_list;
mod ui;
mod spectrogram;

use std::{io, panic};

use anyhow::Result;
use app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use library::initialize_db;

use ratatui::{backend::CrosstermBackend, Terminal};

use rodio::{OutputStream, Sink};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    let terminal = prepare_terminal()?;
    let pool = initialize_db("songs.sqlite").await?;

    let app = App::default();

    let (_output_stream, output_stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&output_stream_handle)?;

    app.run(
        pool,
        sink,
        EventStream::new().filter_map(|ev| ev.ok()),
        terminal,
    )
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
