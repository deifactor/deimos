use anyhow::Result;
use enum_iterator::Sequence;
use itertools::Itertools;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::{
    library::Library,
    ui::{
        artist_album_list::ArtistAlbumList,
        search::SearchResult,
        track_list::{TrackList, TrackListItem},
        ActiveState, DeimosBackend, Ui,
    },
};

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Sequence)]
pub enum PanelItem {
    #[default]
    ArtistAlbumList,
    TrackList,
}

#[derive(Debug, Default)]
pub struct LibraryPanel {
    pub focus: PanelItem,
    pub artist_album_list: ArtistAlbumList,
    pub track_list: TrackList,
}

impl LibraryPanel {
    pub(crate) fn select_entity(&mut self, library: &Library, result: &SearchResult) -> Result<()> {
        let artist = result.album_artist();
        let album = result.album();
        self.artist_album_list.select(artist, album)?;
        self.update_track_list(library)?;
        if let Some(title) = result.track_title() {
            self.track_list.select(title);
        }
        Ok(())
    }

    pub fn move_selection(&mut self, library: &Library, amount: isize) -> Result<()> {
        match self.focus {
            PanelItem::ArtistAlbumList => {
                self.artist_album_list.move_selection(amount);
                self.update_track_list(library)
            }
            PanelItem::TrackList => {
                self.track_list.move_selection(amount);
                Ok(())
            }
        }
    }

    fn update_track_list(&mut self, library: &Library) -> Result<()> {
        let Some(artist) = self.artist_album_list.artist() else {
            return Ok(())
        };

        self.track_list = match self.artist_album_list.album() {
            Some(album) => {
                let tracks = &library.artists[&artist].albums[&album].tracks;
                TrackList::new(tracks.iter().cloned().map(TrackListItem::Track).collect())
            }
            None => {
                let mut albums = library.artists[&artist]
                    .albums
                    .iter()
                    .map(|(id, album)| (format!("{}", id), album.tracks.clone()))
                    .collect_vec();
                albums.sort_unstable_by_key(|(id, _)| id.clone());
                TrackList::new(
                    albums
                        .into_iter()
                        .flat_map(|(title, tracks)| {
                            std::iter::once(TrackListItem::Section(title))
                                .chain(tracks.into_iter().map(TrackListItem::Track))
                        })
                        .collect(),
                )
            }
        };
        Ok(())
    }

    pub fn draw(
        &mut self,
        ui: &Ui,
        frame: &mut ratatui::Frame<DeimosBackend>,
        area: Rect,
    ) -> anyhow::Result<()> {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(area);
        self.artist_album_list.draw(
            ActiveState::focused_if(self.focus == PanelItem::ArtistAlbumList),
            ui,
            frame,
            layout[0],
        )?;
        self.track_list.draw(
            ActiveState::focused_if(self.focus == PanelItem::TrackList),
            ui,
            frame,
            layout[1],
        )?;
        Ok(())
    }
}
