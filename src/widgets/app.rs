use crate::widgets;
use tui;
use tui::layout;
use tui::widgets::Widget;

/// This is the top-level widget that renders the entire app. `main.rs` handles
/// all of the terminal and connection setup.
pub struct App {
    size: tui::layout::Rect,
    queue: widgets::Queue,
    now_playing: widgets::NowPlaying,
}

impl App {
    pub fn new(size: tui::layout::Rect, queue: widgets::Queue,
               now_playing: widgets::NowPlaying) -> App {
        App { size, queue, now_playing }
    }
}

impl Widget for App {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let layout = layout::Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(vec![layout::Constraint::Min(4), layout::Constraint::Length(1)])
            .split(area);
        let mut queue_block = tui::widgets::Block::default()
            .title("Queue")
            .borders(tui::widgets::Borders::ALL);
        queue_block.draw(layout[0], buf);
        self.queue.draw(queue_block.inner(layout[0]), buf);
        self.now_playing.draw(layout[1], buf);
    }
}
