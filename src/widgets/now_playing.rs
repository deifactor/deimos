use mpd::song::Song;
use time::Duration;
use tui;
use tui::widgets::{Block, Widget};

/// A widget displaying information about what's currently playing. Can be
/// multiple lines tall.
pub struct NowPlaying<'a> {
    song: Option<Song>,
    elapsed: Option<Duration>,
    state: mpd::status::State,
    block: Option<Block<'a>>,
}

impl<'a> NowPlaying<'a> {
    pub fn new(
        song: Option<Song>,
        elapsed: Option<Duration>,
        state: mpd::status::State,
    ) -> NowPlaying<'a> {
        NowPlaying {
            song,
            elapsed,
            state,
            block: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> NowPlaying<'a> {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for NowPlaying<'a> {
    fn draw(&mut self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        if let Some(ref song) = self.song {
            let title = song.title.as_ref().unwrap_or(&"Unknown".into()).clone();
            let artist = song.tags.get("Artist").unwrap_or(&"Unknown".into()).clone();
            let album = song.tags.get("Album").unwrap_or(&"Unknown".into()).clone();
            let text = [tui::widgets::Text::raw(format!(
                "{:?}\n{}\n{} - {}",
                self.state, title, artist, album
            ))];
            tui::widgets::Paragraph::new(text.iter())
                .block(self.block.unwrap())
                .wrap(false)
                .draw(area, buf);
        }
    }
}
