use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};
use rodio::Sink;
use sqlx::{Pool, Sqlite};
use tokio::{pin, sync::mpsc::unbounded_channel};
use tokio_stream::{Stream, StreamExt};

use crate::{
    action::{Action, Command},
    artist_album_list::ArtistAlbumList,
    now_playing::NowPlaying,
    spectrogram::Visualizer,
    track_list::TrackList,
    ui::{Component, DeimosBackend, Ui},
};

#[derive(Debug, Default)]
pub struct App {
    pub artist_album_list: ArtistAlbumList,
    pub track_list: TrackList,
    pub now_playing: NowPlaying,
    pub visualizer: Visualizer,
    pub ui: Ui,
}

impl App {
    pub async fn run(
        mut self,
        pool: Pool<Sqlite>,
        sink: Sink,
        terminal_events: impl Stream<Item = Event> + Send + Sync + 'static,
        mut terminal: Terminal<DeimosBackend>,
    ) -> Result<()> {
        let (tx_action, mut rx_action) = unbounded_channel::<Action>();
        let sender = Command::spawn_executor(pool.clone(), sink, tx_action.clone());
        sender.send(Command::LoadLibrary)?;
        pin!(terminal_events);

        loop {
            tokio::select! {
                Some(ev) = terminal_events.next() =>
                if let Some(action) = self.terminal_to_action(ev) {
                    tx_action.send(action)?;
                },
                Some(action) = rx_action.recv() => {
                    if matches!(action, Action::Quit) {
                        return Ok(())
                    } else {
                        action.dispatch(&mut self, &sender)?;
                    }
                }
            }
            terminal.draw(|f| self.draw(f).expect("failed to rerender app"))?;
        }
    }

    pub fn draw(&mut self, f: &mut Frame<'_, DeimosBackend>) -> Result<()> {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Max(6)])
            .split(f.size());
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(root[0]);
        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(root[1]);
        self.artist_album_list.draw(&self.ui, f, top[0])?;
        self.track_list.draw(&self.ui, f, top[1])?;
        self.now_playing.draw(&self.ui, f, bottom[0])?;
        self.visualizer.draw(&self.ui, f, bottom[1])?;
        Ok(())
    }

    fn terminal_to_action(&self, ev: Event) -> Option<Action> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else { return None };
        use Action::*;
        let action = match code {
            KeyCode::Tab => NextFocus,
            KeyCode::Esc | KeyCode::Char('q') => Quit,
            KeyCode::Up => MoveSelection(-1),
            KeyCode::Down => MoveSelection(1),
            KeyCode::Char(' ') => ToggleExpansion,
            KeyCode::Enter => PlaySelectedTrack,
            _ => return None,
        };
        Some(action)
    }
}
