use crossterm::event::KeyCode;
use enum_iterator::{next_cycle, Sequence};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::{
    app::Action,
    ui::{
        artist_album_list::ArtistAlbumList, search::SearchResult, track_list::TrackList,
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
    focus: PanelItem,
    pub artist_album_list: ArtistAlbumList,
    pub track_list: TrackList,
}

impl LibraryPanel {
    pub(crate) fn select_entity(&mut self, result: &SearchResult) {
        self.artist_album_list
            .select(result.album_artist(), result.album())
            .unwrap();
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

    pub fn handle_keycode(&mut self, keycode: KeyCode) -> Option<Action> {
        match keycode {
            KeyCode::Tab => {
                self.focus = next_cycle(&self.focus).unwrap();
                None
            }
            _ => match self.focus {
                PanelItem::ArtistAlbumList => self.artist_album_list.handle_keycode(keycode),
                PanelItem::TrackList => self.track_list.handle_keycode(keycode),
            },
        }
    }
}
