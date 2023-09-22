use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use enum_iterator::next_cycle;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};

use tokio::{
    pin,
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
};
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};

use crate::{
    audio::{Player, PlayerMessage},
    library::Library,
    library_panel::{LibraryPanel, PanelItem},
    ui::{
        artist_album_list::ArtistAlbumList,
        now_playing::{NowPlaying, PlayState},
        search::Search,
        spectrogram::Visualizer,
        DeimosBackend, Ui,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Library,
    Search,
}

pub struct App {
    library: Library,
    player: Player,
    library_panel: LibraryPanel,
    now_playing: NowPlaying,
    visualizer: Visualizer,
    search: Search,
    active_panel: Panel,
    ui: Ui,
    should_quit: bool,

    rx_message: Option<UnboundedReceiver<Message>>,
}

impl App {
    pub fn new(library: Library) -> Self {
        let (_tx_message, rx_message) = unbounded_channel::<Message>();
        Self {
            library,
            player: Player::new(_tx_message).unwrap(),
            library_panel: LibraryPanel::default(),
            now_playing: NowPlaying::default(),
            visualizer: Visualizer::default(),
            search: Search::default(),
            active_panel: Panel::Library,
            ui: Ui::default(),
            should_quit: false,

            rx_message: Some(rx_message),
        }
    }

    pub async fn run(
        mut self,
        terminal_events: impl Stream<Item = Event> + Send + Sync + 'static,
        mut terminal: Terminal<DeimosBackend>,
    ) -> Result<()> {
        self.library_panel.artist_album_list = ArtistAlbumList::new(&self.library);

        pin!(terminal_events);

        let mut event_stream = AppEvent::stream(terminal_events, self.rx_message.take().unwrap());

        while let Some(event) = event_stream.next().await {
            let message = match event {
                AppEvent::Terminal(terminal_event) => self.lookup_binding(terminal_event),
                AppEvent::Message(message) => Some(message),
            };
            if let Some(message) = message {
                self.dispatch(message)?;
            }
            terminal.draw(|f| self.draw(f).expect("failed to rerender app"))?;
            if self.should_quit {
                return Ok(());
            }
        }

        Ok(())
    }

    pub fn draw(&mut self, f: &mut Frame<'_, DeimosBackend>) -> Result<()> {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Max(6)])
            .split(f.size());
        match self.active_panel {
            Panel::Library => self.library_panel.draw(&self.ui, f, root[0])?,
            Panel::Search => self.search.draw(&self.ui, f, root[0])?,
        }
        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(root[1]);
        self.now_playing.draw(&self.ui, f, bottom[0])?;
        self.visualizer.draw(&self.ui, f, bottom[1])?;
        Ok(())
    }

    fn lookup_binding(&self, ev: Event) -> Option<Message> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else { return None };
        self.key_to_command(code).map(Message::Command)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Motion {
    Up,
    Down,
}

/// An [`message`] corresponds to a mutation of the application state. All mutation of application
/// state should be done through messages.
pub enum Message {
    Command(Command),
    Player(PlayerMessage),
}

/// A [`Command`] corresponds to a single user input. The translation of keys to commands is done
/// by a match statement on (active panel, keycode).
pub enum Command {
    /// Start a new search query.
    StartSearch,
    /// Move focus to the next item in the panel.
    NextFocus,
    /// Perform an message on the currently-selected item.
    Activate,
    /// Move the current selection.
    MoveCursor(Motion),
    /// User typed someething into the search input.
    SearchInput(char),
    /// Deletes the most recent character in the search input.
    SearchBackspace,
    /// Seeks the current song by the given amount.
    Seek(i64),
    Quit,
}

impl App {
    fn key_to_command(&self, key: KeyCode) -> Option<Command> {
        let message = match (self.active_panel, key) {
            (Panel::Library, KeyCode::Char('/')) => Command::StartSearch,
            (Panel::Library, KeyCode::Char('q')) => Command::Quit,
            (Panel::Library, KeyCode::Tab) => Command::NextFocus,
            (Panel::Search, KeyCode::Char(c)) => Command::SearchInput(c),
            (Panel::Search, KeyCode::Backspace) => Command::SearchBackspace,
            (_, KeyCode::Up) => Command::MoveCursor(Motion::Up),
            (_, KeyCode::Down) => Command::MoveCursor(Motion::Down),
            (_, KeyCode::Enter) => Command::Activate,
            (_, KeyCode::Char(',')) => Command::Seek(-5),
            (_, KeyCode::Char('.')) => Command::Seek(5),
            _ => return None,
        };
        Some(message)
    }

    fn dispatch(&mut self, message: Message) -> Result<()> {
        use Message::*;
        match message {
            Command(command) => {
                self.dispatch_command(command)?;
            }

            Player(PlayerMessage::AudioFragment {
                buffer,
                timestamp,
                track,
            }) => {
                self.now_playing.play_state = Some(PlayState { timestamp, track });
                self.visualizer.update_spectrum(buffer)?;
            }
        }
        Ok(())
    }

    fn dispatch_command(&mut self, command: Command) -> Result<()> {
        use Command::*;
        match command {
            StartSearch => {
                self.active_panel = Panel::Search;
                self.search = Search::default();
            }
            SearchInput(c) => {
                self.search
                    .run_query(&self.library, format!("{}{}", self.search.query(), c))?;
            }
            SearchBackspace => {
                let mut chars = self.search.query().chars();
                chars.next_back();
                let query = chars.as_str().to_owned();
                self.search.run_query(&self.library, query)?;
            }
            Activate => {
                self.activate_item()?;
            }
            Quit => self.should_quit = true,
            MoveCursor(motion) => {
                let delta = match motion {
                    Motion::Up => -1,
                    Motion::Down => 1,
                };
                match self.active_panel {
                    Panel::Library => {
                        self.library_panel.move_selection(&self.library, delta)?;
                    }
                    Panel::Search => todo!(),
                }
            }
            NextFocus => {
                self.library_panel.focus = next_cycle(&self.library_panel.focus).unwrap();
            }
            Seek(seconds) => {
                let Some(now) = self.now_playing.play_state.as_ref().map(|s| s.timestamp) else { return Ok(()) };
                let target = if seconds > 0 {
                    now + Duration::from_secs(seconds.unsigned_abs())
                } else {
                    now - Duration::from_secs(seconds.unsigned_abs())
                };
                self.player.seek(target)?;
            }
        }
        Ok(())
    }

    fn activate_item(&mut self) -> Result<()> {
        match self.active_panel {
            Panel::Library => match self.library_panel.focus {
                PanelItem::ArtistAlbumList => {
                    self.library_panel.artist_album_list.toggle();
                }
                PanelItem::TrackList => {
                    let Some(selected) = self.library_panel.track_list.selected() else { return Ok(()) };
                    self.player.play_track(selected)?;
                }
            },
            Panel::Search => {
                let Some(selected) = self.search.selected_result() else { return Ok(()) };
                self.active_panel = Panel::Library;
                self.library_panel.select_entity(&self.library, &selected)?;
            }
        }
        Ok(())
    }
}

enum AppEvent {
    Terminal(Event),
    Message(Message),
}

impl AppEvent {
    fn stream(
        terminal_events: impl Stream<Item = Event>,
        rx_message: UnboundedReceiver<Message>,
    ) -> impl Stream<Item = Self> {
        UnboundedReceiverStream::new(rx_message)
            .map(AppEvent::Message)
            .merge(terminal_events.map(AppEvent::Terminal))
    }
}
