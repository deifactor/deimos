use maplit::hashmap;
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

impl Widget for Queue {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let texts = self
            .queue
            .iter()
            .enumerate()
            .flat_map(|(index, song)| {
                let values = hashmap![
                    "title" => song.title.clone().unwrap_or("Unknown".to_owned()),
                    "artist" => song.tags.get("Artist").cloned().unwrap_or("Unknown".to_owned()),
                    "album" => song.tags.get("Album").cloned().unwrap_or("Unknown".to_owned()),
                ];
                let now_playing_display = if Some(index as u32) == self.position {
                    vec![((("> ").to_owned(), mimi::Style::default()))].into_iter()
                } else {
                    vec![].into_iter()
                };
                now_playing_display
                    .chain(self.formatter.spans(&values))
                    .chain(iter::once(("\n".into(), mimi::Style::default())))
            })
            .map(|(text, style)| tui::widgets::Text::styled(text, style.into()));
        tui::widgets::Paragraph::new(texts.collect::<Vec<_>>().iter()).draw(area, buf)
    }
}
