use crossterm::event::KeyCode;
use enum_iterator::{next_cycle, Sequence};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::{
    action::Command,
    ui::{artist_album_list::ArtistAlbumList, track_list::TrackList, Component, DeimosBackend, Ui},
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

impl Component for LibraryPanel {
    fn draw(
        &mut self,
        ui: &Ui,
        frame: &mut ratatui::Frame<DeimosBackend>,
        area: Rect,
    ) -> anyhow::Result<()> {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(area);
        self.artist_album_list.draw(ui, frame, layout[0])?;
        self.track_list.draw(ui, frame, layout[1])?;
        Ok(())
    }

    fn handle_keycode(&mut self, keycode: KeyCode) -> Option<Command> {
        match keycode {
            KeyCode::Tab => self.focus = next_cycle(&self.focus).unwrap(),
            _ => (),
        }
        None
    }
}
