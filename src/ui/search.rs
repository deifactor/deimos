use std::{cell::RefCell, cmp::Reverse, collections::HashSet, ops::DerefMut, sync::Arc};

use eyre::Result;
use itertools::Itertools;
use nucleo_matcher::{
    pattern::{CaseMatching, Pattern},
    Config, Matcher, Utf32String,
};
use once_cell::sync::Lazy;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
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

    /// The search haystack that this matches against.
    fn haystack(&self) -> Option<String> {
        match self {
            SearchItem::Artist(artist) => Some(artist.to_string()),
            SearchItem::Album(album, _) => Some(album.to_string()),
            SearchItem::Track(track) => track.title.clone(),
        }
    }

    /// If this matches the pattern, returns a result containing this as well as metadata about
    /// the match.
    pub fn match_against(self, pattern: &Pattern) -> Option<SearchResult> {
        let haystack = Utf32String::from(self.haystack()?);
        let mut indices = vec![];
        let score = pattern.indices(
            haystack.slice(..),
            MATCHER.try_lock().unwrap().deref_mut(),
            &mut indices,
        )?;

        // XXX: do this better
        let indices: HashSet<u32> = HashSet::from_iter(indices);
        let segments = haystack
            .slice(..)
            .chars()
            .enumerate()
            .map(|(i, c)| SearchTextSegment {
                text: c.to_string(),
                matched: indices.contains(&(i as u32)),
            })
            .collect_vec();

        Some(SearchResult { item: self, score, segments })
    }
}

/// A slice of text used to display a search result.
#[derive(Debug)]
struct SearchTextSegment {
    text: String,
    matched: bool,
}

#[derive(Debug)]
pub struct SearchResult {
    item: SearchItem,
    score: u32,
    segments: Vec<SearchTextSegment>,
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
        self.state.borrow().selected().map(|i| self.results[i].item.clone())
    }

    pub fn run_query(&mut self, library: &Library, query: impl AsRef<str>) -> Result<()> {
        let query = query.as_ref();
        self.query = query.to_owned();

        let pattern = Pattern::parse(query, CaseMatching::Ignore);

        let artists = library.artists().map(|a| a.name.clone()).map(SearchItem::Artist);

        let albums = library
            .albums_with_artist()
            .map(|(album, artist)| SearchItem::Album(album.name.clone(), artist.name.clone()));

        let tracks = library.tracks().map(SearchItem::Track);

        let mut results = artists
            .chain(albums)
            .chain(tracks)
            .filter_map(|item| item.match_against(&pattern))
            .collect_vec();
        results.sort_by_key(|result| Reverse(result.score));

        self.results = results;
        *self.state.borrow_mut().selected_mut() =
            if self.results.is_empty() { None } else { Some(0) };

        Ok(())
    }

    fn render_result(&self, result: &SearchResult) -> ListItem<'static> {
        // render the portion of the result with the match in it
        let segments = &result.segments;
        let match_style = Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD);
        let mut spans = segments
            .iter()
            .map(|segment| {
                Span::styled(
                    segment.text.clone(),
                    if segment.matched { match_style } else { Style::default() },
                )
            })
            .collect_vec();

        match &result.item {
            // artist
            SearchItem::Artist(_) => (),
            // album - artist
            SearchItem::Album(_, artist) => spans.push(Span::raw(format!(" - {}", artist))),
            // track - album - artist
            SearchItem::Track(track) => {
                spans.push(Span::raw(format!("- {} - {}", track.album, track.artist)))
            }
        }
        ListItem::new(Line { spans, alignment: None })
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

        let results =
            List::new(self.results.iter().map(|result| self.render_result(result)).collect_vec())
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
