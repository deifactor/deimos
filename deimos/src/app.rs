use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    backend::Backend,
    border,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame, Terminal,
};
use sqlx::{pool::PoolConnection, Connection, Pool, Sqlite};
use tokio::{
    pin,
    sync::mpsc::{unbounded_channel, UnboundedSender},
};
use tokio_stream::{Stream, StreamExt};

use crate::{
    action::{Action, Command},
    library,
};

#[derive(Debug)]
pub struct App {
    /// Currently-visible artists.
    pub artists: BrowseList,
    /// Albums for the current artist.
    pub albums: BrowseList,
    /// Tracks in the current album.
    pub tracks: BrowseList,
    pub focus: MainList,
}

#[derive(Debug)]
pub enum Message {
    TerminalEvent(Event),
    AppEvent(AppEvent),
}

#[derive(Debug)]
pub enum AppEvent {
    Refresh,
    LibraryLoaded { artists: Vec<String> },
    Quit,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum MainList {
    Artist,
    Album,
    Track,
}

impl MainList {
    pub fn next(self) -> Self {
        use MainList::*;
        match self {
            Artist => Album,
            Album => Track,
            Track => Artist,
        }
    }

    pub fn prev(self) -> Self {
        use MainList::*;
        match self {
            Artist => Track,
            Album => Artist,
            Track => Album,
        }
    }
}

impl App {
    pub fn new() -> Self {
        let artists: Vec<_> = (0..3).map(|x| format!("Artist {x}")).collect();
        let albums: Vec<_> = (0..3).map(|x| format!("Album {x}")).collect();
        let tracks: Vec<_> = (0..3).map(|x| format!("Track {x}")).collect();
        App {
            artists: BrowseList::new(artists),
            albums: BrowseList::new(albums),
            tracks: BrowseList::new(tracks),
            focus: MainList::Artist,
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
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 3); 3])
            .split(f.size());
        f.render_stateful_widget(
            self.artists.widget().block(
                Block::default()
                    .title("Artists")
                    .borders(border!(TOP, BOTTOM, LEFT))
                    .border_style(self.border_style(self.focus == MainList::Artist)),
            ),
            chunks[0],
            &mut self.artists.state,
        );
        f.render_stateful_widget(
            self.albums.widget().block(
                Block::default()
                    .title("Albums")
                    .borders(border!(TOP, BOTTOM, LEFT))
                    .border_style(self.border_style(self.focus == MainList::Album)),
            ),
            chunks[1],
            &mut self.albums.state,
        );
        f.render_stateful_widget(
            self.tracks.widget().block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Tracks")
                    .border_style(self.border_style(self.focus == MainList::Track)),
            ),
            chunks[2],
            &mut self.tracks.state,
        );
    }

    fn focused_list(&mut self) -> &mut BrowseList {
        match self.focus {
            MainList::Artist => &mut self.artists,
            MainList::Album => &mut self.albums,
            MainList::Track => &mut self.tracks,
        }
    }

    fn border_style(&self, is_focused: bool) -> Style {
        if is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        }
    }

    fn terminal_to_action(&self, ev: Event) -> Option<Action> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else { return None };
        use Action::*;
        let action = match code {
            KeyCode::Tab => NextFocus,
            KeyCode::Esc | KeyCode::Char('q') => Quit,
            KeyCode::Down => NextList,
            _ => return None,
        };
        Some(action)
    }
}

#[derive(Debug)]
pub struct BrowseList {
    pub items: Vec<String>,
    pub state: ListState,
}

impl BrowseList {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            state: ListState::default(),
        }
    }
    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + 1) % self.items.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            // separate to handle overflow
            Some(0) => self.items.len() - 1,
            Some(i) => (i - 1) % self.items.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn widget(&self) -> List<'static> {
        let items: Vec<_> = self
            .items
            .iter()
            .map(|text| ListItem::new(text.to_owned()))
            .collect();
        List::new(items).highlight_style(Style::default().fg(Color::Red))
    }
}
