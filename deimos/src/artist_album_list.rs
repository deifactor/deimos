use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use ratatui::{
    backend::Backend,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Debug)]
struct ArtistItem {
    artist: String,
    albums: Vec<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct RowIndex {
    artist: usize,
    /// if None, we're selecting the artist themselves.
    album: Option<usize>,
}

/// By default, an [`ArtistAlbumList`] justs lists the artists; however, if an
/// artist is expanded, it also lists their albums. The list allows selecting
/// either an artist *or* an album.
#[derive(Debug, Default)]
pub struct ArtistAlbumList {
    artists: Vec<ArtistItem>,

    highlight_style: Style,

    /// Number of lines to scroll down when rendering.
    offset: usize,
    /// The offset of the selected item, if any.
    selected: Option<usize>,
    /// Whether or not the artist is expanded.
    expanded: HashSet<usize>,
    /// A flat list of all currently visible items.
    rows: Vec<RowIndex>,
}

/// Methods for manipulating the state
impl ArtistAlbumList {
    pub fn new(artists: HashMap<String, Vec<String>>) -> Self {
        let mut artists = artists
            .into_iter()
            .map(|(artist, albums)| ArtistItem { artist, albums })
            .collect_vec();
        artists.sort_unstable_by_key(|item| item.artist.clone());
        let mut list = Self {
            artists,
            highlight_style: Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 30)),
            ..Default::default()
        };
        list.recompute_rows();
        list
    }

    pub fn next(&mut self) {
        if self.artists.is_empty() {
            return;
        }
        self.selected = match self.selected {
            Some(selected) => Some((selected + 1).min(self.rows.len() - 1)),
            None => Some(0),
        };
    }

    pub fn toggle(&mut self) {
        let Some(selected) = self.selected else { return; };
        let RowIndex { artist, .. } = self.rows[selected];
        if self.expanded.contains(&artist) {
            self.expanded.remove(&artist);
            self.recompute_rows();
            // move the selection to point at the artist, since we just closed it
            self.selected = self.rows.iter().position(|row| row.artist == artist);
        } else {
            self.expanded.insert(artist);
            self.recompute_rows();
        }
    }

    fn recompute_rows(&mut self) {
        self.rows = self
            .artists
            .iter()
            .enumerate()
            .flat_map(|(artist_index, item)| {
                let mut rows = vec![RowIndex {
                    artist: artist_index,
                    album: None,
                }];
                if self.expanded.contains(&artist_index) {
                    rows.extend((0..item.albums.len()).map(|album_idx| RowIndex {
                        artist: artist_index,
                        album: Some(album_idx),
                    }));
                }
                rows.into_iter()
            })
            .collect();
    }
}

/// Drawing code
impl ArtistAlbumList {
    pub fn draw<B: Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        let block = Block::default()
            .title("Artist / Album")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 1 || inner.height < 1 || self.artists.is_empty() {
            // nothing to do
            return;
        }

        for (index, row) in self
            .rows
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(inner.height.into())
        {
            let style = if self.selected == Some(index) {
                self.highlight_style
            } else {
                Style::default()
            };
            let y = index - self.offset;
            let mut text = self.text(*row);
            // need to manually truncate; setting the wrap to `trim: true` will also trim leading whitespace
            text.truncate(inner.width as usize);
            frame.render_widget(
                Paragraph::new(text).style(style),
                Rect::new(inner.left(), inner.top() + y as u16, inner.width, 1),
            );
        }
    }

    /// Text to use when drawing the given row.
    fn text(&self, row: RowIndex) -> String {
        let artist = &self.artists[row.artist];
        match row.album {
            Some(album) => format!("    {}", artist.albums[album]),
            None => artist.artist.clone(),
        }
    }
}
