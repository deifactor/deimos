use crate::events;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Lists all of the albums in the user's library in tree form.
pub struct AlbumTree {
    /// All of the album artists in the library.
    album_artists: Vec<String>,

    /// The index of the currently-selected row. This may correspond to an album
    /// artist, or just an album.
    selected: Option<usize>,

    /// The album artists that we've expanded, and their individual albums.
    albums: HashMap<String, Vec<String>>,

    /// The rows are stored as a vec of (album artist, album) pairs. If the
    /// album is None, the row corresponds to an artist. XXX: this is
    /// memory-hungry. A more lean (but less safe) implementation would just
    /// store the index of the (artist, album_offset) pair and have rules for
    /// how to increment/decrement.
    rows: Vec<(String, Option<String>)>,

    client: Rc<RefCell<mpd::Client>>,
}

impl AlbumTree {
    pub fn new(album_artists: Vec<String>, client: Rc<RefCell<mpd::Client>>) -> Self {
        let mut album_tree = Self {
            album_artists,
            client,
            selected: None,
            rows: vec![],
            albums: HashMap::new(),
        };
        album_tree.compute_rows();
        album_tree
    }

    /// Moves the selection up.
    pub fn up(&mut self) {
        if self.album_artists.is_empty() {
            return;
        }
        self.selected = match self.selected {
            None => Some(self.rows.len() - 1),
            Some(n) => Some((n - 1).max(0)),
        }
    }

    /// Moves the selection down.
    pub fn down(&mut self) {
        if self.album_artists.is_empty() {
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(n) => Some((self.rows.len() - 1).min(n + 1)),
        }
    }

    /// Toggles whether the currently-selected album artist is expanded or not.
    /// Returns a failure if communicating with the client failed.
    pub fn toggle(&mut self) -> Result<(), mpd::error::Error> {
        if let Some(selected) = self.selected {
            // We allow toggling an album, treating it as if we'd toggled on the
            // parent album artist.
            let (album_artist, album) = &self.rows[selected].clone();
            let expand = !self.albums.contains_key(album_artist);
            if expand {
                let albums = self.client.borrow_mut().list(
                    &mpd::Term::Tag("Album".into()),
                    mpd::Query::new().and(mpd::Term::Tag("AlbumArtist".into()), album_artist),
                )?;
                self.albums.insert(album_artist.clone(), albums);
            } else {
                self.albums.remove(album_artist);
            }
            self.compute_rows();
            if album.is_some() && !expand {
                // We were on an album, but we removed it, so adjust our index
                // to the parent artist.
                self.selected = self.rows.iter().position(|ref row| &row.0 == album_artist)
            }
        }
        Ok(())
    }

    /// Populates the list of rows from `album_artists` and `albums`.
    fn compute_rows(&mut self) {
        self.rows = vec![];
        for album_artist in &self.album_artists {
            self.rows.push((album_artist.clone(), None));
            if let Some(albums) = self.albums.get(album_artist) {
                self.rows.extend(
                    albums
                        .iter()
                        .map(|album| (album_artist.clone(), Some(album.clone()))),
                );
            }
        }
    }

    /// Computes the row index to start drawing from. The offset is set so that
    /// the selected item is as close to the center as possible. `height` is the
    /// height of the region we're drawing into.
    fn draw_start_row(&self, height: usize) -> usize {
        if let Some(selected) = self.selected {
            (selected - (height / 2).min(selected)).min(self.rows.len() - height)
        } else {
            0
        }
    }
}

impl events::EventHandler for AlbumTree {
    fn handle_event(&mut self, event: &events::Event) {
        use termion::event::Key;
        if let Some(key) = event.key() {
            match key {
                Key::Up => self.up(),
                Key::Down => self.down(),
                Key::Char('\n') => self.toggle().expect("failed to talk to client"),
                _ => (),
            }
        }
    }
}

impl tui::widgets::Widget for AlbumTree {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let draw_start = self.draw_start_row(area.height as usize);
        for (i, (album_artist, album)) in self
            .rows
            .iter()
            .by_ref()
            .enumerate()
            .skip(self.draw_start_row(area.height as usize))
            .take(area.height as usize)
        {
            let style = if Some(i) == self.selected {
                tui::style::Style::default().modifier(tui::style::Modifier::Invert)
            } else {
                Default::default()
            };
            let text = match album {
                Some(album) => format!(" └──{}", album),
                None => album_artist.clone(),
            };
            self.background(&area, buf, tui::style::Color::Reset);
            buf.set_stringn(
                area.left(),
                (area.top() as usize + i - draw_start) as u16,
                text,
                area.width as usize,
                style,
            )
        }
    }
}
