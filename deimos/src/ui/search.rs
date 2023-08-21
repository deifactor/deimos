use anyhow::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use sqlx::{Sqlite, Transaction};

use crate::action::{Action, Command};

use super::{ActiveState, Component, DeimosBackend};

/// Searches the library. Searches in album names, artist names, and track names.

/// Things that match the search.
#[derive(Debug, Clone)]
pub enum SearchResult {
    Artist(String),
    Album {
        name: String,
        album_artist: String,
    },
    Track {
        name: String,
        album_artist: String,
        album: String,
    },
}
impl SearchResult {
    pub fn album_artist(&self) -> &str {
        match self {
            SearchResult::Artist(artist) => artist,
            SearchResult::Album { album_artist, .. } => album_artist.as_str(),
            SearchResult::Track { album_artist, .. } => album_artist.as_str(),
        }
    }

    pub fn album(&self) -> Option<&str> {
        match self {
            SearchResult::Artist(_) => None,
            SearchResult::Album { name, .. } => Some(name.as_str()),
            SearchResult::Track { album, .. } => Some(album.as_str()),
        }
    }

    pub fn track_title(&self) -> Option<&str> {
        match self {
            SearchResult::Track { name, .. } => Some(name.as_str()),
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
            SearchResult::Artist(artist) => ListItem::new(artist.clone()),
            SearchResult::Album { name, album_artist } => {
                ListItem::new(format!("{name} - {album_artist}"))
            }
            SearchResult::Track {
                name,
                album_artist,
                album,
            } => ListItem::new(format!("{name} - {album_artist} - {album}")),
        }
    }

    pub async fn run_search_query(
        query: impl AsRef<str>,
        conn: &mut Transaction<'_, Sqlite>,
    ) -> Result<Vec<SearchResult>> {
        let query = format!("%{}%", query.as_ref());
        let mut results = vec![];
        results.extend(
            sqlx::query_scalar!(
                r#"SELECT DISTINCT artist AS "artist!" FROM songs
                WHERE artist LIKE ? AND artist IS NOT NULL
                ORDER BY artist"#,
                query,
            )
            .fetch_all(&mut **conn)
            .await?
            .into_iter()
            .map(SearchResult::Artist),
        );
        results.extend(
            sqlx::query!(
                r#"SELECT DISTINCT artist AS "artist!", album AS "album!" FROM songs
                WHERE album LIKE ? AND artist IS NOT NULL AND album IS NOT NULL
                ORDER BY album, artist"#,
                query
            )
            .fetch_all(&mut **conn)
            .await?
            .into_iter()
            .map(|rec| SearchResult::Album {
                name: rec.album,
                album_artist: rec.artist,
            }),
        );
        results.extend(
            sqlx::query!(
                r#"SELECT DISTINCT artist AS "artist!", album AS "album!", title AS "title!" FROM songs
                WHERE title LIKE ? 
                AND artist IS NOT NULL AND album IS NOT NULL AND title IS NOT NULL
                ORDER BY title, album, artist"#,
                query
            )
            .fetch_all(&mut **conn)
            .await?
            .into_iter()
            .map(|rec| SearchResult::Track {
                name: rec.title,
                album_artist: rec.artist,
                album: rec.album,
            }));
        Ok(results)
    }
}

impl Component for Search {
    fn draw(
        &mut self,
        state: ActiveState,
        ui: &super::Ui,
        frame: &mut Frame<DeimosBackend>,
        area: Rect,
    ) -> Result<()> {
        let block = Block::default()
            .title("Search")
            .borders(Borders::ALL)
            .border_style(ui.border(state));

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

    fn handle_keycode(&mut self, keycode: KeyCode) -> Option<Command> {
        let old_query = self.query.clone();
        match keycode {
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(c) => self.query.push(c),
            KeyCode::Enter => {
                return Some(Command::RunAction(Action::SelectEntity(
                    self.results[0].clone(),
                )))
            }
            _ => (),
        };
        if old_query != self.query {
            Some(Command::Search {
                query: self.query.clone(),
            })
        } else {
            None
        }
    }
}
