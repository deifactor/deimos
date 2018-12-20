use mpd::song::Song;
use tui;
use tui::widgets::Widget;

/// A widget displaying the now-playing queue.
pub struct Queue {
    queue: Vec<Song>,
    position: Option<u32>,
}

impl Queue {
    pub fn new(queue: Vec<Song>, position: Option<u32>) -> Queue {
        Queue { queue, position }
    }
}

impl Widget for Queue {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let texts = self.queue.iter().enumerate().map(|(index, song)| {
            let title = song.title.as_ref().unwrap_or(&"Unknown".into()).clone();
            let artist = song.tags.get("Artist").unwrap_or(&"Unknown".into()).clone();
            let album = song.tags.get("Album").unwrap_or(&"Unknown".into()).clone();
            let text = format!("{} - {} - {}\n", title, artist, album);
            if self.position == Some(index as u32) {
                tui::widgets::Text::Styled(
                    text.into(),
                    tui::style::Style::default().modifier(tui::style::Modifier::Invert),
                )
            } else {
                tui::widgets::Text::raw(text)
            }
        });
        tui::widgets::Paragraph::new(texts.collect::<Vec<_>>().iter()).draw(area, buf)
    }
}
