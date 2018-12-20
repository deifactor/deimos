use mpd::song::Song;
use time::Duration;
use tui;
use tui::widgets::Widget;

/// A widget displaying information about what's currently playing. Can be
/// multiple lines tall.
pub struct NowPlaying {
    song: Option<Song>,
    elapsed: Option<Duration>,
    state: mpd::status::State,
}

impl NowPlaying {
    pub fn new(
        song: Option<Song>,
        elapsed: Option<Duration>,
        state: mpd::status::State,
    ) -> NowPlaying {
        NowPlaying {
            song,
            elapsed,
            state,
        }
    }
}

impl Widget for NowPlaying {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        if let Some(ref song) = self.song {
            let title = song.title.as_ref().unwrap_or(&"Unknown".into()).clone();
            let artist = song.tags.get("Artist").unwrap_or(&"Unknown".into()).clone();
            let album = song.tags.get("Album").unwrap_or(&"Unknown".into()).clone();
            let text = format!("{:?}: {} - {} - {}", self.state, title, artist, album);
            buf.set_stringn(
                area.left(),
                area.top(),
                text,
                area.width as usize,
                tui::style::Style::default(),
            );
        }
    }
}
