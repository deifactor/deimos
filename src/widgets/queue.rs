use crate::events;
use crate::widgets;
use mpd::song::Song;
use std::iter;
use tui;
use tui::widgets::Widget;

/// A widget displaying the now-playing queue.
pub struct Queue {
    queue: Vec<Song>,
    position: Option<u32>,
    formatter: mimi::Formatter,
}

impl Queue {
    pub fn new(queue: Vec<Song>, position: Option<u32>, formatter: mimi::Formatter) -> Queue {
        Queue {
            queue,
            position,
            formatter,
        }
    }
}

// TODO
impl events::EventHandler for Queue {
    fn handle_event(&mut self, events: &events::Event) {}
}

impl Widget for Queue {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let texts = self
            .queue
            .iter()
            .enumerate()
            .flat_map(|(index, song)| {
                let now_playing_display = if Some(index as u32) == self.position {
                    Some((("> ").to_owned(), mimi::Style::default()))
                } else {
                    None
                };
                now_playing_display
                    .into_iter()
                    .chain(self.formatter.spans(&widgets::song_values(song)))
                    .chain(iter::once(("\n".into(), mimi::Style::default())))
            })
            .map(|(text, style)| tui::widgets::Text::styled(text, style.into()));
        tui::widgets::Paragraph::new(texts.collect::<Vec<_>>().iter()).draw(area, buf)
    }
}
