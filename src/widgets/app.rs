use crate::events;
use crate::widgets;
use tui;
use tui::layout;
use tui::widgets::Widget;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Screen {
    Queue,
    Albums,
}

/// This is the top-level widget that renders the entire app. `main.rs` handles
/// all of the terminal and connection setup.
pub struct App {
    size: tui::layout::Rect,
    pub screen: Screen,
    queue: widgets::Queue,
    album_tree: widgets::AlbumTree,
    now_playing: widgets::NowPlaying,
}

impl App {
    pub fn new(
        size: tui::layout::Rect,
        queue: widgets::Queue,
        album_tree: widgets::AlbumTree,
        now_playing: widgets::NowPlaying,
    ) -> App {
        App {
            size,
            screen: Screen::Queue,
            queue,
            album_tree,
            now_playing,
        }
    }

    fn active_widget(&mut self) -> Box<&mut dyn Widget> {
        match self.screen {
            Screen::Queue => Box::new(&mut self.queue),
            Screen::Albums => Box::new(&mut self.album_tree),
        }
    }

    fn active_handler(&mut self) -> Box<&mut dyn events::EventHandler> {
        match self.screen {
            Screen::Queue => Box::new(&mut self.queue),
            Screen::Albums => Box::new(&mut self.album_tree),
        }
    }
}

impl events::EventHandler for App {
    fn handle_event(&mut self, event: &events::Event) {
        if let Some(termion::event::Key::Char(c)) = event.key() {
            match c {
                '1' => self.screen = Screen::Queue,
                '2' => self.screen = Screen::Albums,
                _ => (),
            }
        }
        self.active_handler().handle_event(event);
    }
}

impl Widget for App {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let layout = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(vec![
                layout::Constraint::Min(4),
                layout::Constraint::Length(1),
            ])
            .split(area);
        let mut queue_block = tui::widgets::Block::default()
            .title("Queue")
            .borders(tui::widgets::Borders::ALL);
        queue_block.draw(layout[0], buf);
        self.active_widget().draw(queue_block.inner(layout[0]), buf);
        self.now_playing.draw(layout[1], buf);
    }
}
