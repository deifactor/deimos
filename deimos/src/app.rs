use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{backend::Backend, Frame, Terminal};
use sqlx::{Pool, Sqlite};
use tokio::{pin, sync::mpsc::unbounded_channel};
use tokio_stream::{Stream, StreamExt};

use crate::{
    action::{Action, Command},
    artist_album_list::ArtistAlbumList,
};

#[derive(Debug)]
pub struct App {
    pub artist_album_list: ArtistAlbumList,
}

impl App {
    pub fn new() -> Self {
        App {
            artist_album_list: ArtistAlbumList::default(),
        }
    }

    pub async fn run<B: Backend>(
        mut self,
        pool: Pool<Sqlite>,
        terminal_events: impl Stream<Item = Event> + Send + Sync + 'static,
        mut terminal: Terminal<B>,
    ) -> Result<()> {
        let (tx_action, mut rx_action) = unbounded_channel::<Action>();
        let sender = Command::spawn_executor(pool.clone(), tx_action.clone());
        sender.send(Command::LoadLibrary)?;
        pin!(terminal_events);

        loop {
            tokio::select! {
                Some(ev) = terminal_events.next() =>
                if let Some(action) = self.terminal_to_action(ev) {
                    tx_action.send(action)?;
                },
                Some(action) = rx_action.recv() =>
                { action.dispatch(&mut self, &sender)?; }
            }
            terminal.draw(|f| self.draw(f))?;
        }
    }

    pub fn draw<B: Backend>(&mut self, f: &mut Frame<'_, B>) {
        self.artist_album_list.draw(f, f.size());
    }

    fn terminal_to_action(&self, ev: Event) -> Option<Action> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else { return None };
        use Action::*;
        let action = match code {
            KeyCode::Tab => NextFocus,
            KeyCode::Esc | KeyCode::Char('q') => Quit,
            KeyCode::Down => NextList,
            KeyCode::Char(' ') => ToggleExpansion,
            _ => return None,
        };
        Some(action)
    }
}