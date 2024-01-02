use std::{io::Stdout, sync::Arc, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use enum_iterator::next_cycle;
use eyre::Result;
use itertools::Itertools;
use log::debug;
use mpris_server::Server;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::Backend,
    Frame, Terminal,
};

use tokio::{
    pin,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver},
        RwLock,
    },
};
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};

use crate::{
    audio::{Player, PlayerMessage},
    library::Library,
    library_panel::{LibraryPanel, PanelItem},
    mpris::MprisAdapter,
    ui::{
        artist_album_list::ArtistAlbumList, now_playing::NowPlaying, search::Search,
        spectrogram::Visualizer, Ui,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Library,
    Search,
}

pub struct App {
    mpris: Option<MprisAdapter>,
    library: Library,
    player: Arc<RwLock<Player>>,
    library_panel: LibraryPanel,
    visualizer: Visualizer,
    search: Search,
    active_panel: Panel,
    ui: Ui,
    should_quit: bool,

    rx_message: Option<UnboundedReceiver<Message>>,
}

impl App {
    pub fn new(library: Library) -> Self {
        let (tx_message, rx_message) = unbounded_channel::<Message>();

        let player = Arc::new(RwLock::new(Player::new(tx_message.clone()).unwrap()));
        let mpris = MprisAdapter::new(tx_message.clone(), Arc::clone(&player));

        Self {
            mpris: Some(mpris),
            library,
            player,
            library_panel: LibraryPanel::default(),
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
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        self.library_panel.artist_album_list = ArtistAlbumList::new(&self.library);

        pin!(terminal_events);

        let mut event_stream = AppEvent::stream(terminal_events, self.rx_message.take().unwrap());

        let _server = Server::new("deimos", self.mpris.take().unwrap()).await?;

        self.draw(terminal).await?;
        while let Some(event) = event_stream.next().await {
            let message = match event {
                AppEvent::Terminal(terminal_event) => self.lookup_binding(terminal_event),
                AppEvent::Message(message) => Some(message),
            };
            if let Some(message) = message {
                debug!("Received message {message:?}");
                self.dispatch(message).await?;
            }
            self.draw(terminal).await?;
            if self.should_quit {
                return Ok(());
            }
        }

        Ok(())
    }

    pub async fn draw<T: Backend>(&mut self, terminal: &mut Terminal<T>) -> Result<()> {
        let player = self.player.read().await;
        let mut cb = |f: &mut Frame| {
            let root = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Max(6)])
                .split(f.size());
            match self.active_panel {
                Panel::Library => {
                    self.library_panel.draw(&self.ui, f, root[0], player.current())?
                }
                Panel::Search => self.search.draw(&self.ui, f, root[0])?,
            }
            let bottom = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(root[1]);
            NowPlaying { timestamp: player.timestamp(), track: player.current() }
                .draw(&self.ui, f, bottom[0])?;
            self.visualizer.draw(&self.ui, f, bottom[1])?;
            eyre::Ok(())
        };
        terminal.draw(|f| cb(f).expect("failed to render app"))?;
        Ok(())
    }

    fn lookup_binding(&self, ev: Event) -> Option<Message> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else {
            return None;
        };
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
#[derive(Debug)]
pub enum Message {
    Command(Command),
    Player(PlayerMessage),
}

/// A [`Command`] corresponds to a single user input. The translation of keys to commands is done
/// by a match statement on (active panel, keycode).
#[derive(Debug)]
pub enum Command {
    /// Cancel out of whatever it is we're doing.
    Cancel,
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
    /// Adds the currently selected song to the play queue.
    AddSongToQueue,
    /// Seeks to the previous song if near the beginning, or restarts the song if not.
    PreviousOrSeekToStart,
    Play,
    Pause,
    Stop,
    PlayPause,
    NextTrack,
    Quit,
}

impl App {
    fn key_to_command(&self, key: KeyCode) -> Option<Command> {
        let message = match (self.active_panel, key) {
            (Panel::Library, KeyCode::Char('/')) => Command::StartSearch,
            (Panel::Library, KeyCode::Char('q')) => Command::Quit,
            (Panel::Library, KeyCode::Tab) => Command::NextFocus,
            (Panel::Library, KeyCode::Char('u')) => Command::AddSongToQueue,
            (Panel::Search, KeyCode::Char(c)) => Command::SearchInput(c),
            (Panel::Search, KeyCode::Backspace) => Command::SearchBackspace,
            (_, KeyCode::Up) => Command::MoveCursor(Motion::Up),
            (_, KeyCode::Down) => Command::MoveCursor(Motion::Down),
            (_, KeyCode::Enter) => Command::Activate,
            (_, KeyCode::Char(',')) => Command::Seek(-5),
            (_, KeyCode::Char('.')) => Command::Seek(5),
            (_, KeyCode::Char('z')) => Command::PreviousOrSeekToStart,
            (_, KeyCode::Char('x')) => Command::PlayPause,
            (_, KeyCode::Char('c')) => Command::NextTrack,
            (_, KeyCode::Esc) => Command::Cancel,
            _ => return None,
        };
        Some(message)
    }

    async fn dispatch(&mut self, message: Message) -> Result<()> {
        use Message::*;
        match message {
            Command(command) => {
                self.dispatch_command(command).await?;
            }

            Player(PlayerMessage::AudioFragment { buffer, timestamp }) => {
                self.player.write().await.set_timestamp(Some(timestamp));
                self.visualizer.update_spectrum(buffer)?;
            }
            Player(PlayerMessage::Finished) => {
                self.dispatch_command(self::Command::NextTrack).await?;
            }
        }
        Ok(())
    }

    async fn dispatch_command(&mut self, command: Command) -> Result<()> {
        use Command::*;
        match command {
            Cancel => match self.active_panel {
                Panel::Library => (),
                Panel::Search => self.active_panel = Panel::Library,
            },
            StartSearch => {
                self.active_panel = Panel::Search;
                self.search = Search::default();
            }
            SearchInput(c) => {
                self.search.run_query(&self.library, format!("{}{}", self.search.query(), c))?;
            }
            SearchBackspace => {
                let mut chars = self.search.query().chars();
                chars.next_back();
                let query = chars.as_str().to_owned();
                self.search.run_query(&self.library, query)?;
            }
            Activate => {
                self.activate_item().await?;
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
                    Panel::Search => self.search.move_cursor(delta),
                }
            }
            NextFocus => {
                self.library_panel.focus = next_cycle(&self.library_panel.focus).unwrap();
            }
            Seek(seconds) => {
                let mut player = self.player.write().await;
                let Some(now) = player.timestamp() else {
                    return Ok(());
                };
                let target = if seconds > 0 {
                    now + Duration::from_secs(seconds.unsigned_abs())
                } else {
                    now.saturating_sub(Duration::from_secs(seconds.unsigned_abs()))
                };
                if player.seek(target).await.is_err() {
                    // can happen when seeking off the end, etc
                    player.next().await?;
                }
                self.visualizer.reset()?;
            }
            AddSongToQueue => {
                let Some(selected) = self.library_panel.track_list.selected() else {
                    return Ok(());
                };
                self.player.write().await.queue_push(selected);
            }
            Play => self.player.write().await.play().await?,
            PlayPause => {
                let mut player = self.player.write().await;
                if player.playing().await {
                    player.pause().await;
                } else {
                    player.play().await?;
                }
            }
            Pause => self.player.write().await.pause().await,
            Stop => self.player.write().await.stop().await,
            PreviousOrSeekToStart => {
                const MIN_DURATION_TO_SEEK: Duration = Duration::from_secs(5);
                let mut player = self.player.write().await;
                if player.timestamp().map_or(false, |dur| dur >= MIN_DURATION_TO_SEEK) {
                    player.seek(Duration::ZERO).await?;
                } else {
                    player.previous().await?;
                }
                self.visualizer.reset()?;
            }
            NextTrack => {
                self.player.write().await.next().await?;
                self.visualizer.reset()?;
            }
        }
        Ok(())
    }

    async fn activate_item(&mut self) -> Result<()> {
        match self.active_panel {
            Panel::Library => match self.library_panel.focus {
                PanelItem::ArtistAlbumList => {
                    self.library_panel.artist_album_list.toggle();
                }
                PanelItem::TrackList => {
                    let Some(selected) = self.library_panel.track_list.selected() else {
                        return Ok(());
                    };
                    let tracks = self.library_panel.track_list.tracks().collect_vec();
                    let index = tracks.iter().find_position(|t| **t == selected).unwrap().0;
                    let mut player = self.player.write().await;
                    player.set_play_queue(tracks).await;
                    player.set_queue_index(Some(index)).await?;
                    player.play().await?;
                    self.visualizer.reset()?;
                }
            },
            Panel::Search => {
                let Some(selected) = self.search.selected_item() else {
                    return Ok(());
                };
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
