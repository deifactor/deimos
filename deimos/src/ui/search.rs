use anyhow::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::{
    action::Action,
    library::{AlbumId, ArtistId, Library, Track},
};

use super::{ActiveState, Component, DeimosBackend};

/// Searches the library. Searches in album names, artist names, and track names.

/// Things that match the search.
#[derive(Debug, Clone)]
pub enum SearchResult {
    Artist(ArtistId),
    Album(AlbumId, ArtistId),
    Track(Track),
}

impl SearchResult {
    pub fn album_artist(&self) -> &ArtistId {
        match self {
            SearchResult::Artist(artist) => artist,
            SearchResult::Album(_, artist) => artist,
            SearchResult::Track(track) => &track.artist,
        }
    }

    pub fn album(&self) -> Option<&AlbumId> {
        match self {
            SearchResult::Artist(_) => None,
            SearchResult::Album(album, _) => Some(album),
            SearchResult::Track(track) => Some(&track.album),
        }
    }

    pub fn track_title(&self) -> Option<&str> {
        match self {
            SearchResult::Track(track) => track.title.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Search {
    query: String,
    results: Vec<SearchResult>,
    state: ListState,
}
impl Search {
    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.results = results;
    }

    fn render_result(&self, track: &SearchResult) -> ListItem<'static> {
        match track {
            SearchResult::Artist(artist) => ListItem::new(format!("{}", artist)),
            SearchResult::Album(album, artist) => ListItem::new(format!("{} - {}", album, artist)),
            SearchResult::Track(track) => ListItem::new(format!(
                "{} - {} - {}",
                track.title.as_deref().unwrap_or("<unknown>"),
                track.album,
                track.artist,
            )),
        }
    }

    pub fn run_search_query(
        library: &Library,
        query: impl AsRef<str>,
    ) -> Result<Vec<SearchResult>> {
        let query = query.as_ref();
        // XXX: this isn't right. use regex
        let is_match = |haystack: &String| haystack.to_lowercase().contains(&query.to_lowercase());
        let artists = library
            .artists()
            .map(|a| &a.id)
            .filter(|id| match id {
                ArtistId::Unknown => false,
                ArtistId::Artist(name) => is_match(name),
            })
            .cloned()
            .map(SearchResult::Artist);

        let albums = library
            .albums_with_artist()
            .filter(|(album, _)| album.id.0.as_ref().map_or(false, is_match))
            .map(|(album, artist)| SearchResult::Album(album.id.clone(), artist.id.clone()));

        let tracks = library
            .tracks()
            .filter(|track| track.title.as_ref().map_or(false, is_match))
            .cloned()
            .map(SearchResult::Track);

        let results = artists.chain(albums).chain(tracks).collect_vec();

        Ok(results)
    }

    pub fn draw(
        &mut self,
        ui: &super::Ui,
        frame: &mut Frame<DeimosBackend>,
        area: Rect,
    ) -> Result<()> {
        let block = Block::default()
            .title("Search")
            .borders(Borders::ALL)
            .border_style(ui.border(ActiveState::Focused));

        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(1), Constraint::Min(1)])
            .split(area);

        let query = Paragraph::new(self.query.as_str());
        frame.render_widget(query, root[0]);

        let results = List::new(
            self.results
                .iter()
                .map(|track| self.render_result(track))
                .collect_vec(),
        )
        .highlight_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 30)))
        .block(block);
        frame.render_stateful_widget(results, root[1], &mut self.state);
        Ok(())
    }
}

impl Component for Search {
    fn handle_keycode(&mut self, keycode: KeyCode) -> Option<Action> {
        let old_query = self.query.clone();
        match keycode {
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(c) => self.query.push(c),
            KeyCode::Enter => return Some(Action::SelectEntity(self.results[0].clone())),
            _ => (),
        };
        if old_query != self.query {
            Some(Action::RunSearch(self.query.clone()))
        } else {
            None
        }
    }
}
