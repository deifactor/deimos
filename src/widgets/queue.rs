use crate::events;
use crate::widgets;
use mpd::song::Song;
use std::iter;
use tui;
use tui::widgets::Widget;

/// A widget displaying the now-playing queue.
pub struct Queue<'a> {
    queue: Option<Vec<Song>>,
    position: Option<u32>,
    formatter: &'a mimi::Formatter,
}

impl<'a> Queue<'a> {
    pub fn new(formatter: &'a mimi::Formatter) -> Self {
        Queue {
            queue: None,
            position: None,
            formatter,
        }
    }

    pub fn set_queue(&mut self, queue: Vec<Song>) {
        self.queue = Some(queue)
    }

    pub fn set_position(&mut self, position: Option<u32>) {
        self.position = position
    }
}

// TODO
impl events::EventHandler for Queue<'_> {
    fn handle_event(&mut self, events: &events::Event) {}
}

impl Widget for Queue<'_> {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let empty: Vec<Song> = vec![];
        let texts = self
            .queue
            .as_ref()
            .unwrap_or(&empty)
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
