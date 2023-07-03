use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use ratatui::{
    backend::Backend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap},
    Frame,
};

#[derive(Debug)]
pub struct ArtistItem {
    artist: String,
    albums: Vec<String>,
}

/// State stored between renders of the [`ArtistAlbumList`].
#[derive(Debug, Default)]
pub struct ArtistAlbumListState {
    /// Number of lines to scroll down when rendering.
    offset: usize,
    /// The offset of the selected item, if any.
    selected: Option<usize>,
    /// Whether or not the artist is expanded.
    expanded: HashSet<usize>,
}

/// By default, an [`ArtistAlbumList`] justs lists the artists; however, if an
/// artist is expanded, it also lists their albums. The list allows selecting
/// either an artist *or* an album.
#[derive(Debug, Default)]
pub struct ArtistAlbumList {
    artists: Vec<ArtistItem>,

    highlight_style: Style,
    state: ArtistAlbumListState,
}

impl ArtistAlbumList {
    pub fn new(artists: HashMap<String, Vec<String>>) -> Self {
        let mut artists = artists
            .into_iter()
            .map(|(artist, albums)| ArtistItem { artist, albums })
            .collect_vec();
        artists.sort_unstable_by_key(|item| item.artist.clone());
        Self {
            artists,
            highlight_style: Style::default().fg(Color::Green),
            state: ArtistAlbumListState::default(),
        }
    }

    fn rows<'a>(&'a self) -> impl Iterator<Item = Line<'a>> + 'a {
        self.artists
            .iter()
            .enumerate()
            .flat_map(move |(index, item)| {
                let mut rows = vec![item.artist.as_str()];
                if self.state.expanded.contains(&index) {
                    rows.extend(item.albums.iter().map(String::as_str));
                }
                rows.into_iter().map(Line::from)
            })
            .skip(self.state.offset)
    }

    pub fn next(&mut self) {
        if self.artists.is_empty() {
            return;
        }
        let i = match self.state.selected {
            Some(i) => (i + 1) % self.artists.len(),
            None => 0,
        };
        self.state.selected = Some(i);
    }

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

        for (i, row) in self.rows().enumerate().take(inner.height.into()) {
            let style = if self.state.selected == Some(i) {
                self.highlight_style
            } else {
                Style::default()
            };
            frame.render_widget(
                Paragraph::new(row).wrap(Wrap { trim: true }).style(style),
                Rect::new(inner.left(), inner.top() + i as u16, inner.width, 1),
            );
        }
    }
}
