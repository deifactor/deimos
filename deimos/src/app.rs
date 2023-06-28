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
                    .borders(border!(TOP, BOTTOM, LEFT)),
            ),
            chunks[0],
            &mut self.artists.state,
        );
        f.render_stateful_widget(
            self.albums.widget().block(
                Block::default()
                    .borders(border!(TOP, BOTTOM, LEFT))
                    .title("Albums"),
            ),
            chunks[1],
            &mut self.albums.state,
        );
        f.render_stateful_widget(
            self.tracks
                .widget()
                .block(Block::default().borders(Borders::ALL).title("Tracks")),
            chunks[2],
            &mut self.tracks.state,
        );
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
