use std::{cell::Cell, collections::HashSet};

use eyre::{anyhow, Result};
use itertools::Itertools;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    library::{AlbumName, ArtistName, Library},
    ui::Ui,
};

use super::ActiveState;

#[derive(Debug)]
struct ArtistItem {
    artist: ArtistName,
    albums: Vec<AlbumName>,
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
    offset: Cell<usize>,
    /// The offset of the selected item, if any.
    selected: Option<usize>,
    /// Whether or not the artist is expanded.
    expanded: HashSet<usize>,
    /// A flat list of all currently visible items.
    rows: Vec<RowIndex>,
}

/// Methods for manipulating the state
impl ArtistAlbumList {
    pub fn new(library: &Library) -> Self {
        let mut artists = library
            .artists()
            .map(|artist| {
                let mut albums = artist.albums.keys().cloned().collect_vec();
                albums.sort_unstable();
                ArtistItem {
                    artist: artist.name.clone(),
                    albums,
                }
            })
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

    pub fn artist(&self) -> Option<ArtistName> {
        let idx = self.selected?;
        Some(self.artists[self.rows[idx].artist].artist.clone())
    }

    pub fn album(&self) -> Option<AlbumName> {
        let idx = self.selected?;
        let artist = self.rows[idx].artist;
        let album = self.rows[idx].album?;
        Some(self.artists[artist].albums[album].clone())
    }

    /// Move to the previous selection.
    pub fn move_selection(&mut self, amount: isize) {
        if self.artists.is_empty() {
            return;
        }
        self.selected = match self.selected {
            Some(selected) => Some(selected.saturating_add_signed(amount).min(self.rows.len() - 1)),
            None if amount > 0 => Some(0),
            None => None,
        }
    }

    /// Toggles whether the currently selected artist is expanded. Adjusts the selection as necessary.
    pub fn toggle(&mut self) {
        let Some(selected) = self.selected else {
            return;
        };
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
        self.rows.clear();
        for (artist_idx, item) in self.artists.iter().enumerate() {
            self.rows.push(RowIndex {
                artist: artist_idx,
                album: None,
            });
            if self.expanded.contains(&artist_idx) {
                for album_idx in 0..item.albums.len() {
                    self.rows.push(RowIndex {
                        artist: artist_idx,
                        album: Some(album_idx),
                    });
                }
            }
        }
    }

    /// Move the selection to the given artist (and optionally album),
    /// expanding it if they aren't already. Errors if that artist/album does not exist.
    pub fn select(&mut self, artist: &ArtistName, album: Option<&AlbumName>) -> Result<()> {
        // XXX: linear scanning is inefficient!
        let (artist_index, item) = self
            .artists
            .iter()
            .find_position(|item| &item.artist == artist)
            .ok_or_else(|| anyhow!("couldn't find {artist}"))?;
        let album_index = album
            .map(|album| {
                item.albums
                    .iter()
                    .position(|val| val == album)
                    .ok_or_else(|| anyhow!("couldn't find {album} for {artist}"))
            })
            .transpose()?;
        self.expanded.insert(artist_index);
        self.recompute_rows();
        self.selected = self
            .rows
            .iter()
            .position(|row| row.artist == artist_index && row.album == album_index);
        Ok(())
    }
}

/// Drawing code
impl ArtistAlbumList {
    /// Text to use when drawing the given row.
    fn text(&self, row: RowIndex) -> String {
        let artist = &self.artists[row.artist];
        match row.album {
            Some(album) => format!("    {}", artist.albums[album]),
            None => format!("{}", artist.artist),
        }
    }

    pub fn draw(&self, state: ActiveState, ui: &Ui, frame: &mut Frame, area: Rect) -> Result<()> {
        let block = Block::default()
            .title("Artist / Album")
            .borders(Borders::ALL)
            .border_style(ui.border(state));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 1 || inner.height < 1 || self.artists.is_empty() {
            // nothing to do
            return Ok(());
        }

        if let Some(selected) = self.selected {
            self.offset.set(
                self.offset
                    .get()
                    .max(selected.saturating_sub(inner.height.saturating_sub(3) as usize)),
            );
        }

        for (index, row) in
            self.rows.iter().enumerate().skip(self.offset.get()).take(inner.height.into())
        {
            let style = if self.selected == Some(index) {
                self.highlight_style
            } else {
                Style::default()
            };
            let y = index - self.offset.get();
            let mut text = self.text(*row);
            // need to manually truncate; setting the wrap to `trim: true` will also trim leading whitespace
            text.truncate(inner.width as usize);
            frame.render_widget(
                Paragraph::new(text).style(style),
                Rect::new(inner.left(), inner.top() + y as u16, inner.width, 1),
            );
        }
        Ok(())
    }
}
