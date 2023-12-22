use std::{cell::RefCell, cmp::Reverse, fmt::Display, ops::DerefMut, sync::Arc};

use eyre::Result;
use itertools::Itertools;
use nucleo_matcher::{
    pattern::{CaseMatching, Pattern},
    Config, Matcher, Utf32Str,
};
use once_cell::sync::Lazy;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::sync::Mutex;

use crate::library::{AlbumName, ArtistName, Library, Track};

use super::ActiveState;

/// Searches the library. Searches in album names, artist names, and track names.

/// Things that match the search.
#[derive(Debug, Clone)]
pub enum SearchResult {
    Artist(ArtistName),
    Album(AlbumName, ArtistName),
    Track(Arc<Track>),
}

static MATCHER: Lazy<Mutex<Matcher>> = Lazy::new(|| Mutex::new(Matcher::new(Config::DEFAULT)));

impl Display for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchResult::Artist(artist) => write!(f, "{}", artist),
            SearchResult::Album(album, artist) => write!(f, "{} - {}", artist, album),
            SearchResult::Track(track) => write!(
                f,
                "{} - {} - {}",
                track.title.as_deref().unwrap_or("<unknown>"),
                track.artist,
                track.album
            ),
        }
    }
}

impl SearchResult {
    pub fn album_artist(&self) -> &ArtistName {
        match self {
            SearchResult::Artist(artist) => artist,
            SearchResult::Album(_, artist) => artist,
            SearchResult::Track(track) => &track.artist,
        }
    }

    pub fn album(&self) -> Option<&AlbumName> {
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

    /// Matches the given pattern against this. If success, returns (score, indices). Indices are
    /// guaranteed to be sorted.
    pub fn matches(&self, pattern: &Pattern) -> Option<(u32, Vec<u32>)> {
        let mut buf = vec![];
        let displayed = self.to_string();
        let haystack = Utf32Str::new(&displayed, &mut buf);
        let mut indices = vec![];
        pattern
            .indices(
                haystack,
                MATCHER.try_lock().unwrap().deref_mut(),
                &mut indices,
            )
            .map(|s| {
                indices.sort_unstable();
                indices.dedup();
                (s, indices)
            })
    }
}

#[derive(Debug, Default)]
pub struct Search {
    query: String,
    results: Vec<SearchResult>,
    state: RefCell<ListState>,
}

impl Search {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected_result(&self) -> Option<SearchResult> {
        self.state
            .borrow()
            .selected()
            .map(|i| self.results[i].clone())
    }

    pub fn run_query(&mut self, library: &Library, query: impl AsRef<str>) -> Result<()> {
        let query = query.as_ref();
        self.query = query.to_owned();

        let pattern = Pattern::parse(query, CaseMatching::Ignore);

        let artists = library
            .artists()
            .map(|a| a.name.clone())
            .map(SearchResult::Artist);

        let albums = library
            .albums_with_artist()
            .map(|(album, artist)| SearchResult::Album(album.name.clone(), artist.name.clone()));

        let tracks = library.tracks().map(SearchResult::Track);

        let mut scored = artists
            .chain(albums)
            .chain(tracks)
            .filter_map(|result| result.matches(&pattern).map(|score| (result, score)))
            .collect_vec();
        scored.sort_by_key(|(_, (score, _))| Reverse(*score));
        scored.reverse();

        self.results = scored.into_iter().map(|(result, _)| result).collect_vec();
        *self.state.borrow_mut().selected_mut() = if self.results.is_empty() {
            None
        } else {
            Some(0)
        };

        Ok(())
    }

    pub fn draw(&self, ui: &super::Ui, frame: &mut Frame, area: Rect) -> Result<()> {
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
                .map(|track| ListItem::new(track.to_string()))
                .collect_vec(),
        )
        .highlight_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 30)))
        .block(block);
        frame.render_stateful_widget(results, root[1], &mut self.state.borrow_mut());
        Ok(())
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if let Some(s) = self.state.borrow_mut().selected_mut().as_mut() {
            *s = s.saturating_add_signed(delta).min(self.results.len() - 1);
        }
    }
}
