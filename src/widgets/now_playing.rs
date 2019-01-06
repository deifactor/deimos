use maplit::hashmap;
use mpd::song::Song;
use time::Duration;
use tui;
use tui::widgets::Widget;

/// A widget displaying information about what's currently playing.
pub struct NowPlaying {
    song: Option<Song>,
    elapsed: Option<Duration>,
    state: mpd::status::State,
    formatter: mimi::Formatter,
}

impl NowPlaying {
    pub fn new(
        song: Option<Song>,
        elapsed: Option<Duration>,
        state: mpd::status::State,
        formatter: mimi::Formatter,
    ) -> NowPlaying {
        NowPlaying {
            song,
            elapsed,
            state,
            formatter,
        }
    }
}

impl Widget for NowPlaying {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        if let Some(ref song) = self.song {
            if let Some(ref elapsed) = self.elapsed {
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
            let values = hashmap![
                "title" => song.title.clone().unwrap_or("Unknown".to_owned()),
                "artist" => song.tags.get("Artist").cloned().unwrap_or("Unknown".to_owned()),
                "album" => song.tags.get("Album").cloned().unwrap_or("Unknown".to_owned()),
            ];
            let texts: Vec<_> = self
                .formatter
                .spans(&values)
                .map(|(text, style)| tui::widgets::Text::styled(text, style.into()))
                .collect();
            tui::widgets::Paragraph::new(texts.iter()).draw(area, buf);
        }
    }
}
