use crate::events;
use std::collections::HashMap;

/// Lists all of the albums in the user's library in tree form.
pub struct AlbumTree {
    /// All of the album artists in the library.
    album_artists: Vec<String>,

    /// The index of the currently-selected album artist.
    selected: Option<usize>,

    /// The album artists that we've expanded, and their individual albums.
    albums: HashMap<String, Vec<String>>,
}

impl AlbumTree {
    pub fn new(album_artists: Vec<String>) -> Self {
        Self {
            album_artists,
            selected: None,
            albums: HashMap::new(),
        }
    }

    /// Moves the selection up.
    pub fn up(&mut self) {
        if self.album_artists.is_empty() {
            return;
        }
        self.selected = match self.selected {
            None => Some(self.album_artists.len() - 1),
            Some(n) => Some(std::cmp::max(n - 1, 0)),
        }
    }

    /// Moves the selection down.
    pub fn down(&mut self) {
        if self.album_artists.is_empty() {
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(n) => Some(std::cmp::min(n + 1, self.album_artists.len())),
        }
    }

    /// Toggles whether the currently-selected album artist is expanded or not.
    /// This needs a client so it can fetch the albums if necessary. Returns
    /// failure only if loading the albums fails. TODO: doing it this way is
    /// kind of annoying.
    pub fn toggle(&mut self, client: &mut mpd::Client) -> Result<(), mpd::error::Error> {
        if let Some(selected) = self.selected {
            let album_artist = &self.album_artists[selected];
            if self.albums.contains_key(album_artist) {
                self.albums.remove(album_artist);
            } else {
                let albums = client.list(
                    &mpd::Term::Tag("Album".into()),
                    mpd::Query::new().and(mpd::Term::Tag("AlbumArtist".into()), album_artist),
                )?;
                self.albums.insert(album_artist.clone(), albums);
            }
        }
        Ok(())
    }
}

impl events::EventHandler for AlbumTree {
    fn handle_event(&mut self, events: &events::Event) {}
}

impl tui::widgets::Widget for AlbumTree {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        for (i, album_artist) in self
            .album_artists
            .iter()
            .by_ref()
            .enumerate()
            .take(area.height as usize)
        {
            self.background(&area, buf, tui::style::Color::Reset);
            buf.set_stringn(
                area.left(),
                area.top() + i as u16,
                album_artist,
                area.width as usize,
                Default::default(),
            )
        }
    }
}
