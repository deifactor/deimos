use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::{
    library::Track,
    ui::{DeimosBackend, Ui},
};

use super::ActiveState;

/// Corresponds to a single row in the track list.
#[derive(Debug)]
pub enum TrackListItem {
    /// An actual track.
    Track(Track),
    /// Section heading. This is not selectable.
    Section(String),
}

impl TrackListItem {
    fn as_list_item(&self, ui: &Ui) -> ListItem {
        match self {
            TrackListItem::Track(track) => {
                ListItem::new(track.title.as_deref().unwrap_or("<unknown>"))
            }
            TrackListItem::Section(title) => {
                ListItem::new(title.clone()).style(ui.theme.section_header)
            }
        }
    }

    fn selectable(&self) -> bool {
        matches!(self, TrackListItem::Track(..))
    }
}

#[derive(Debug, Default)]
pub struct TrackList {
    items: Vec<TrackListItem>,
    state: ListState,
}

/// Methods for manipulating the state
impl TrackList {
    pub fn new(items: Vec<TrackListItem>) -> Self {
        Self {
            items,
            state: ListState::default(),
        }
    }

    /// Move the selection by `amount`, which must either be -1 or 1. If the selection would move
    /// to a section header, keep moving. If that would take us off the edge, do nothing.
    pub fn move_selection(&mut self, amount: isize) {
        assert!(amount.abs() == 1);
        if self.items.is_empty() {
            return;
        }
        let candidate = match self.state.selected() {
            Some(selected) => Some(
                selected
                    .saturating_add_signed(amount)
                    .min(self.items.len() - 1),
            ),
            None if amount > 0 => Some(0),
            None => None,
        };
        self.state
            .select(candidate.and_then(|start| self.next_valid_selection(start, amount)));
    }

    /// If `i` is selectable, returns it. If not, moves in the direction given by the signum of
    /// `direction` until a selectable item is found. If there are none, returns None.
    fn next_valid_selection(&self, start: usize, direction: isize) -> Option<usize> {
        assert!(direction != 0);
        if direction > 0 {
            (start..).find(|i| self.items[*i].selectable())
        } else {
            (0..=start).rev().find(|i| self.items[*i].selectable())
        }
    }

    pub fn select(&mut self, title: &str) {
        self.state
            .select(self.items.iter().position(|track| match track {
                TrackListItem::Track(Track { title: Some(t), .. }) => t == title,
                _ => false,
            }))
    }

    pub fn selected(&self) -> Option<&Track> {
        self.state.selected().map(|i| match &self.items[i] {
            TrackListItem::Track(track) => track,
            _ => panic!("Somehow selected a non-track"),
        })
    }

    pub fn draw(
        &mut self,
        state: ActiveState,
        ui: &Ui,
        frame: &mut Frame<DeimosBackend>,
        area: Rect,
    ) -> Result<()> {
        let block = Block::default()
            .title("Tracks")
            .borders(Borders::ALL)
            .border_style(ui.border(state));

        let list = List::new(
            self.items
                .iter()
                .map(|item| item.as_list_item(ui))
                .collect_vec(),
        )
        .highlight_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(30, 30, 30)))
        .block(block);
        frame.render_stateful_widget(list, area, &mut self.state);
        Ok(())
    }
}
