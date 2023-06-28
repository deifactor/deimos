use anyhow::{bail, Result};
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
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};

use crate::library;

#[derive(Debug)]
pub struct App {
    /// Currently-visible artists.
    artists: BrowseList,
    /// Albums for the current artist.
    albums: BrowseList,
    /// Tracks in the current album.
    tracks: BrowseList,
    focus: Focus,

    tx_event: UnboundedSender<AppEvent>,
    rx_event: Option<UnboundedReceiver<AppEvent>>,
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
pub enum Focus {
    ArtistList,
    AlbumList,
    TrackList,
}

impl Focus {
    fn next(self) -> Self {
        use Focus::*;
        match self {
            ArtistList => AlbumList,
            AlbumList => TrackList,
            TrackList => ArtistList,
        }
    }

    fn prev(self) -> Self {
        use Focus::*;
        match self {
            ArtistList => TrackList,
            AlbumList => ArtistList,
            TrackList => AlbumList,
        }
    }
}

impl App {
    pub fn new() -> Self {
        let artists: Vec<_> = (0..3).map(|x| format!("Artist {x}")).collect();
        let albums: Vec<_> = (0..3).map(|x| format!("Album {x}")).collect();
        let tracks: Vec<_> = (0..3).map(|x| format!("Track {x}")).collect();
        let (tx_event, rx_event) = unbounded_channel();
        App {
            artists: BrowseList::new(artists),
            albums: BrowseList::new(albums),
            tracks: BrowseList::new(tracks),
            focus: Focus::ArtistList,
            tx_event,
            rx_event: Some(rx_event),
        }
    }

    pub async fn run<B: Backend>(
        mut self,
        pool: Pool<Sqlite>,
        terminal_events: impl Stream<Item = Event>,
        mut terminal: Terminal<B>,
    ) -> Result<()> {
        let event_stream = terminal_events.map(Message::TerminalEvent).merge(
            UnboundedReceiverStream::new(self.rx_event.take().unwrap()).map(Message::AppEvent),
        );

        let tx_event = self.tx_event.clone();
        tokio::spawn(async move {
            let conn = pool.acquire().await?;
            load_library(conn, tx_event).await?;
            anyhow::Ok(())
        });

        pin!(event_stream);
        while let Some(ev) = (event_stream).next().await {
            match ev {
                Message::TerminalEvent(ev) => self.handle_terminal(ev)?,
                Message::AppEvent(AppEvent::Quit) => break,
                Message::AppEvent(ev) => self.handle_app(ev)?,
            };
            terminal.draw(|f| self.draw(f))?;
        }
        Ok(())
    }

    pub fn handle_terminal(&mut self, event: Event) -> Result<()> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = event else { return Ok(()) };
        match code {
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            KeyCode::Down => self.focused_list().next(),
            KeyCode::Up => self.focused_list().prev(),
            KeyCode::Esc | KeyCode::Char('q') => self.tx_event.send(AppEvent::Quit)?,
            _ => (),
        };
        Ok(())
    }

    fn handle_app(&mut self, ev: AppEvent) -> Result<()> {
        match ev {
            AppEvent::Refresh => (),
            AppEvent::LibraryLoaded { artists } => self.artists.items = artists,
            AppEvent::Quit => bail!("we should have quit already"),
        };
        Ok(())
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
                    .border_style(self.border_style(self.focus == Focus::ArtistList)),
            ),
            chunks[0],
            &mut self.artists.state,
        );
        f.render_stateful_widget(
            self.albums.widget().block(
                Block::default()
                    .title("Albums")
                    .borders(border!(TOP, BOTTOM, LEFT))
                    .border_style(self.border_style(self.focus == Focus::AlbumList)),
            ),
            chunks[1],
            &mut self.albums.state,
        );
        f.render_stateful_widget(
            self.tracks.widget().block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Tracks")
                    .border_style(self.border_style(self.focus == Focus::TrackList)),
            ),
            chunks[2],
            &mut self.tracks.state,
        );
    }

    fn focused_list(&mut self) -> &mut BrowseList {
        match self.focus {
            Focus::ArtistList => &mut self.artists,
            Focus::AlbumList => &mut self.albums,
            Focus::TrackList => &mut self.tracks,
        }
    }

    fn border_style(&self, is_focused: bool) -> Style {
        if is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        }
    }
}

async fn load_library(
    mut conn: PoolConnection<Sqlite>,
    tx: UnboundedSender<AppEvent>,
) -> Result<()> {
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
    let artists = sqlx::query_scalar!(
        r#"SELECT DISTINCT artist AS "artist!" FROM songs WHERE artist IS NOT NULL"#
    )
    .fetch_all(&mut conn)
    .await?;
    tx.send(AppEvent::LibraryLoaded { artists })?;
    Ok(())
}

#[derive(Debug)]
struct BrowseList {
    items: Vec<String>,
    state: ListState,
}

impl BrowseList {
    fn new(items: Vec<String>) -> Self {
        Self {
            items,
            state: ListState::default(),
        }
    }
    fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + 1) % self.items.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn prev(&mut self) {
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

    fn widget(&self) -> List<'static> {
        let items: Vec<_> = self
            .items
            .iter()
            .map(|text| ListItem::new(text.to_owned()))
            .collect();
        List::new(items).highlight_style(Style::default().fg(Color::Red))
    }
}
