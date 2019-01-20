use crate::config;
use crate::events;
use crate::widgets;
use std::cell::RefCell;
use std::rc::Rc;
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
pub struct App<'a> {
    size: tui::layout::Rect,
    pub screen: Screen,
    queue: widgets::Queue<'a>,
    album_tree: widgets::AlbumTree,
    song: Option<mpd::Song>,
    status: mpd::Status,
    client: Rc<RefCell<mpd::Client>>,
    config: &'a config::Config
}

impl App<'_> {
    pub fn new(
        size: tui::layout::Rect,
        client: Rc<RefCell<mpd::Client>>,
        config: &config::Config
    ) -> App {
        let album_artists = client.borrow_mut()
            .list(&mpd::Term::Tag("AlbumArtist".into()), &mpd::Query::new())
            .expect("failed to list album artists");
        App {
            size,
            screen: Screen::Queue,
            queue: widgets::Queue::new(&config.format.playlist_song),
            album_tree: widgets::AlbumTree::new(album_artists, client.clone()),
            song: None,
            status: Default::default(),
            client,
            config
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

    pub fn set_song_queue(&mut self, queue: Vec<mpd::Song>) {
        self.queue.set_queue(queue);
    }

    pub fn set_song(&mut self, song: Option<mpd::Song>) {
        self.song = song;
        self.queue.set_position(self.song.as_ref().and_then(|song| Some(song.place?.pos)));
    }
    pub fn set_status(&mut self, status: mpd::Status) {
        self.status = status
    }
}

impl events::EventHandler for App<'_> {
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

impl Widget for App<'_> {
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
        widgets::NowPlaying {
            song: &self.song,
            status: &self.status,
            formatter: &self.config.format.now_playing
        }.draw(layout[1], buf);
    }
}
