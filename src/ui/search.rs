use std::{
    cell::RefCell, cmp::Reverse, collections::HashSet, fmt::Display, ops::DerefMut, sync::Arc,
};

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
pub enum SearchItem {
    Artist(ArtistName),
    Album(AlbumName, ArtistName),
    Track(Arc<Track>),
}

static MATCHER: Lazy<Mutex<Matcher>> = Lazy::new(|| Mutex::new(Matcher::new(Config::DEFAULT)));

impl Display for SearchItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchItem::Artist(artist) => write!(f, "{}", artist),
            SearchItem::Album(album, artist) => write!(f, "{} - {}", artist, album),
            SearchItem::Track(track) => write!(
                f,
                "{} - {} - {}",
                track.title.as_deref().unwrap_or("<unknown>"),
                track.artist,
                track.album
            ),
        }
    }
}

impl SearchItem {
    pub fn album_artist(&self) -> &ArtistName {
        match self {
            SearchItem::Artist(artist) => artist,
            SearchItem::Album(_, artist) => artist,
            SearchItem::Track(track) => &track.artist,
        }
    }

    pub fn album(&self) -> Option<&AlbumName> {
        match self {
            SearchItem::Artist(_) => None,
            SearchItem::Album(album, _) => Some(album),
            SearchItem::Track(track) => Some(&track.album),
        }
    }

    pub fn track_title(&self) -> Option<&str> {
        match self {
            SearchItem::Track(track) => track.title.as_deref(),
            _ => None,
        }
    }
}

/// A slice of text used to display a search result.
#[derive(Debug)]
#[allow(dead_code)]
struct SearchTextSegment {
    text: String,
    matched: bool,
}

#[derive(Debug)]
pub struct SearchResult {
    item: SearchItem,
    score: u32,
    #[allow(dead_code)]
    segments: Vec<SearchTextSegment>,
}

impl SearchResult {
    pub fn try_from_item(item: SearchItem, pattern: &Pattern) -> Option<Self> {
        let mut buf = vec![];
        let displayed = item.to_string();
        let haystack = Utf32Str::new(&displayed, &mut buf);
        let mut indices = vec![];
        let score = pattern.indices(
            haystack,
            MATCHER.try_lock().unwrap().deref_mut(),
            &mut indices,
        )?;

        // XXX: do this better
        let indices: HashSet<u32> = HashSet::from_iter(indices);
        let segments = haystack
            .chars()
            .enumerate()
            .map(|(i, c)| SearchTextSegment {
                text: c.to_string(),
                matched: indices.contains(&(i as u32)),
            })
            .collect_vec();

        Some(Self {
            item,
            score,
            segments,
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

    pub fn selected_item(&self) -> Option<SearchItem> {
        self.state
            .borrow()
            .selected()
            .map(|i| self.results[i].item.clone())
    }

    pub fn run_query(&mut self, library: &Library, query: impl AsRef<str>) -> Result<()> {
        let query = query.as_ref();
        self.query = query.to_owned();

        let pattern = Pattern::parse(query, CaseMatching::Ignore);

        let artists = library
            .artists()
            .map(|a| a.name.clone())
            .map(SearchItem::Artist);

        let albums = library
            .albums_with_artist()
            .map(|(album, artist)| SearchItem::Album(album.name.clone(), artist.name.clone()));

        let tracks = library.tracks().map(SearchItem::Track);

        let mut results = artists
            .chain(albums)
            .chain(tracks)
            .filter_map(|item| SearchResult::try_from_item(item, &pattern))
            .collect_vec();
        results.sort_by_key(|result| Reverse(result.score));

        self.results = results;
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
                .map(|result| ListItem::new(result.item.to_string()))
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
