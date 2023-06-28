mod app;
mod library;
mod player;

use std::{io, panic};

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use library::initialize_db;

use ratatui::{backend::CrosstermBackend, Terminal};

use sqlx::Connection;

#[tokio::main]
async fn main() -> Result<()> {
    let mut terminal = prepare_terminal()?;
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

    let mut app = App::new();

    loop {
        terminal.draw(|f| {
            app.draw(f);
        })?;
        let event = event::read()?;
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('q') | KeyCode::Esc,
                ..
            }) => break,
            _ => app.handle_event(event),
        }
    }

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
