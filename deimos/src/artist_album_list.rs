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

/// State stored between renders of the [`ArtistAlbumList`].
#[derive(Debug, Default)]
pub struct ArtistAlbumListState {
    /// Number of lines to scroll down when rendering.
    offset: usize,
    /// The offset of the selected item, if any.
    selected: Option<RowIndex>,
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

/// Methods for manipulating the state
impl ArtistAlbumList {
    pub fn new(artists: HashMap<String, Vec<String>>) -> Self {
        let mut artists = artists
            .into_iter()
            .map(|(artist, albums)| ArtistItem { artist, albums })
            .collect_vec();
        artists.sort_unstable_by_key(|item| item.artist.clone());
        Self {
            artists,
            highlight_style: Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 30)),
            state: ArtistAlbumListState::default(),
        }
    }

    pub fn next(&mut self) {
        if self.artists.is_empty() {
            return;
        }
        let Some(selected) = self.state.selected else {
            self.state.selected = Some(RowIndex { artist: 0, album: None });
            return;
        };

        if !self.state.expanded.contains(&selected.artist) {
            self.state.selected = Some(RowIndex {
                artist: (selected.artist + 1).min(self.artists.len()),
                album: None,
            });
            return;
        }

        let selection = match self.state.selected {
            Some(RowIndex {
                artist,
                album: None,
            }) => RowIndex {
                artist,
                album: Some(0),
            },
            Some(RowIndex {
                artist,
                album: Some(album),
            }) => {
                if album + 1 < self.artists[artist].albums.len() {
                    RowIndex {
                        artist,
                        album: Some(album + 1),
                    }
                } else if artist + 1 < self.artists.len() {
                    RowIndex {
                        artist: artist + 1,
                        album: None,
                    }
                } else {
                    RowIndex {
                        artist,
                        album: Some(album),
                    }
                }
            }
            None => RowIndex {
                artist: 0,
                album: None,
            },
        };
        self.state.selected = Some(selection);
    }

    pub fn toggle(&mut self) {
        let Some(RowIndex { artist, .. }) = self.state.selected else { return; };
        if self.state.expanded.contains(&artist) {
            self.state.expanded.remove(&artist);
            self.state.selected = Some(RowIndex {
                artist,
                album: None,
            });
        } else {
            self.state.expanded.insert(artist);
        }
    }
}

#[derive(Debug)]
struct Row {
    text: String,
    index: RowIndex,
}

/// Drawing code
impl ArtistAlbumList {
    fn rows(&self) -> impl Iterator<Item = Row> + '_ {
        self.artists
            .iter()
            .enumerate()
            .flat_map(move |(artist_index, item)| {
                let mut rows = vec![Row {
                    text: item.artist.clone(),
                    index: RowIndex {
                        artist: artist_index,
                        album: None,
                    },
                }];
                if self.state.expanded.contains(&artist_index) {
                    rows.extend(
                        item.albums
                            .iter()
                            .enumerate()
                            .map(|(album_idx, album)| Row {
                                text: format!("    {album}"),
                                index: RowIndex {
                                    artist: artist_index,
                                    album: Some(album_idx),
                                },
                            }),
                    );
                }
                rows.into_iter()
            })
            .skip(self.state.offset)
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

        for (y, Row { mut text, index }) in self.rows().take(inner.height.into()).enumerate() {
            let style = if self.state.selected == Some(index) {
                self.highlight_style
            } else {
                Style::default()
            };
            // need to manually truncate; setting the wrap to `trim: true` will also trim leading whitespace
            text.truncate(inner.width as usize);
            frame.render_widget(
                Paragraph::new(text).style(style),
                Rect::new(inner.left(), inner.top() + y as u16, inner.width, 1),
            );
        }
    }
}
