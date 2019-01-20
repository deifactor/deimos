use crate::widgets;
use mpd::{Song, Status};
use time::Duration;
use tui;
use tui::widgets::Widget;

/// A widget displaying information about what's currently playing.
pub struct NowPlaying<'a> {
    pub song: &'a Option<Song>,
    pub status: &'a mpd::status::Status,
    pub formatter: &'a mimi::Formatter,
}

impl Widget for NowPlaying<'_> {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        if let Some(ref song) = self.song {
            if let Some(ref elapsed) = self.status.elapsed {
                let seconds = elapsed.num_seconds() - elapsed.num_minutes() * 60;
                let text = format!("{}:{:02}", elapsed.num_minutes(), seconds);
                let len = text.len();
                buf.set_stringn(
                    area.left() + area.width - (len as u16),
                    area.top(),
                    text,
                    len,
                    tui::style::Style::default(),
                )
            }
            let texts: Vec<_> = self
                .formatter
                .spans(&widgets::song_values(&song))
                .map(|(text, style)| tui::widgets::Text::styled(text, style.into()))
                .collect();
            tui::widgets::Paragraph::new(texts.iter()).draw(area, buf);
        }
    }
}
