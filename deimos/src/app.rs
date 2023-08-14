use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use enum_iterator::next_cycle;
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
    ui::{
        artist_album_list::ArtistAlbumList, now_playing::NowPlaying, search::Search,
        spectrogram::Visualizer, track_list::TrackList, Component, DeimosBackend, FocusTarget, Ui,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Play,
    Search,
}

#[derive(Debug, Default)]
pub struct App {
    pub artist_album_list: ArtistAlbumList,
    pub track_list: TrackList,
    pub now_playing: NowPlaying,
    pub visualizer: Visualizer,
    pub search: Search,
    pub mode: Mode,
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
                if let Some(command) = self.handle_event(ev) {
                    sender.send(command)?;
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
        match self.mode {
            Mode::Play => {
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
            }

            Mode::Search => {
                self.search.draw(&self.ui, f, f.size())?;
            }
        }
        Ok(())
    }

    fn focused(&mut self) -> &mut dyn Component {
        match self.mode {
            Mode::Play => match self.ui.focus {
                FocusTarget::ArtistAlbumList => &mut self.artist_album_list,
                FocusTarget::TrackList => &mut self.track_list,
            },
            Mode::Search => &mut self.search,
        }
    }

    fn handle_event(&mut self, ev: Event) -> Option<Command> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else { return None };
        use Action::*;
        match code {
            KeyCode::Tab => self.ui.focus = next_cycle(&self.ui.focus).unwrap(),
            KeyCode::Esc | KeyCode::Char('q') => return Some(Command::RunAction(Quit)),
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.search = Search::default();
            }
            _ => return self.focused().handle_keycode(code),
        }
        None
    }
}
