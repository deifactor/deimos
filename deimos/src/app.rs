use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    backend::Backend,
    border,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

#[derive(Debug)]
pub struct App {
    /// Currently-visible artists.
    artists: BrowseList,
    /// Albums for the current artist.
    albums: BrowseList,
    /// Tracks in the current album.
    tracks: BrowseList,
    focus: Focus,
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
        App {
            artists: BrowseList::new(artists),
            albums: BrowseList::new(albums),
            tracks: BrowseList::new(tracks),
            focus: Focus::ArtistList,
        }
    }

    pub fn handle_event(&mut self, event: Event) {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = event else { return };
        match code {
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            _ => (),
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

    fn border_style(&self, is_focused: bool) -> Style {
        if is_focused {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        }
    }
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

    fn widget(&self) -> List<'static> {
        let items: Vec<_> = self
            .items
            .iter()
            .map(|text| ListItem::new(text.to_owned()))
            .collect();
        List::new(items).highlight_style(Style::default().fg(Color::Red))
    }
}
