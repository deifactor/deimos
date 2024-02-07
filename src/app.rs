use std::{io::Stdout, ops::Deref, sync::Arc, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use itertools::Itertools;
use log::{debug, error};
use mpris_server::{LoopStatus, Server, TrackId};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::{Backend, Rect},
    Terminal,
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
    library::{Library, Track},
    library_panel::{LibraryPanel, PanelItem},
    mpris::MprisAdapter,
    ui::{
        album_art::AlbumArt, artist_album_list::ArtistAlbumList, now_playing::NowPlaying,
        search::Search, spectrogram::Visualizer, Theme, Ui,
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
    album_art: AlbumArt,
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
            album_art: AlbumArt::new().expect("failed to initialize image display"),

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

        terminal.hide_cursor()?;
        self.draw(terminal).await?;
        while let Some(event) = event_stream.next().await {
            let message = match event {
                AppEvent::Terminal(terminal_event) => self.lookup_binding(terminal_event),
                AppEvent::Message(message) => Some(message),
                AppEvent::Tick => {
                    if self.tick().await? {
                        self.draw(terminal).await?;
                    }
                    continue;
                }
            };
            let audio_only = message.as_ref().map_or(false, |m| m.audio_only());
            if let Some(message) = message {
                debug!("Received message {message:?}");
                self.dispatch(message).await?;
            }
            if audio_only {
                self.draw_audio_only(terminal).await?;
            } else {
                self.draw(terminal).await?;
            }
            if self.should_quit {
                return Ok(());
            }
        }

        Ok(())
    }

    pub async fn draw<T: Backend>(&mut self, terminal: &mut Terminal<T>) -> Result<()> {
        // This is kind of an inlined version of Terminal::draw(), with some changes.

        // We swap at the *start* of the function so that `draw_audio_only` won't be working from a
        // blank buffer.
        terminal.swap_buffers();

        let player = self.player.read().await;
        // Autoresize - otherwise we get glitches if shrinking or potential desync between widgets
        // and the terminal (if growing), which may OOB.
        terminal.autoresize()?;

        let frame = &mut terminal.get_frame();
        let bounds = Bounds::new(frame.size());
        match self.active_panel {
            Panel::Library => {
                self.library_panel.draw(&self.ui, frame, bounds.panel, player.current())?
            }
            Panel::Search => self.search.draw(&self.ui, frame, bounds.panel)?,
        }
        NowPlaying { timestamp: player.timestamp(), track: player.current() }.draw(
            &self.ui,
            frame,
            bounds.now_playing,
        )?;
        self.visualizer.draw(&self.ui, frame, bounds.visualizer)?;
        self.album_art.draw(&self.ui, frame, bounds.album_art)?;

        // Draw to stdout
        terminal.flush()?;

        // Flush
        terminal.backend_mut().flush()?;
        Ok(())
    }

    pub async fn draw_audio_only<T: Backend>(&mut self, terminal: &mut Terminal<T>) -> Result<()> {
        let player = self.player.read().await;
        terminal.autoresize()?;

        let frame = &mut terminal.get_frame();
        let bounds = Bounds::new(frame.size());
        NowPlaying { timestamp: player.timestamp(), track: player.current() }.draw(
            &self.ui,
            frame,
            bounds.now_playing,
        )?;
        self.visualizer.draw(&self.ui, frame, bounds.visualizer)?;
        let buffer = frame.buffer_mut();
        let mut updates = vec![];
        for rect in [bounds.now_playing, bounds.visualizer] {
            for y in rect.top()..rect.bottom() {
                for x in rect.left()..rect.right() {
                    updates.push((x, y, buffer.get(x, y).clone()));
                }
            }
        }

        terminal.backend_mut().draw(updates.iter().map(|(x, y, cell)| (*x, *y, cell)))?;
        terminal.backend_mut().flush()?;

        Ok(())
    }

    fn lookup_binding(&self, ev: Event) -> Option<Message> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else {
            return None;
        };
        self.key_to_command(code).map(Message::Command)
    }

    /// Handles a time tick. The return value is true if this needs a refresh; not all ticks
    /// actually require a redraw.
    async fn tick(&self) -> Result<bool> {
        Ok(false)
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

impl Message {
    /// True if the message corresponds to progress from audio playback. We use this for more
    /// efficient draw calls.
    fn audio_only(&self) -> bool {
        match self {
            Message::Command(_) => false,
            Message::Player(PlayerMessage::AudioFragment { .. }) => true,
            Message::Player(_) => false,
        }
    }
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
    /// If the current track has the given track ID, sets the position accordingly. This is used
    /// by the mpris server, where the track ID is present to avoid race conditions.
    SetPositionIfTrack {
        position: Duration,
        mpris_id: TrackId,
    },
    SetLoopStatus(LoopStatus),
    SetShuffle(bool),
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
        let old_track = self.player.read().await.current();
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
        let new_track = self.player.read().await.current();
        // Check if the track changed; if so, update the theme.
        if old_track != new_track {
            self.on_track_change(old_track.as_deref()).await?;
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
            NextFocus => self.library_panel.focus = self.library_panel.focus.next(),
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
            SetPositionIfTrack { position, mpris_id } => {
                let mut player = self.player.write().await;
                if player.current().map(|t| t.mpris_id()) != Some(mpris_id) {
                    return Ok(());
                }
                if player.seek(position).await.is_err() {
                    // can happen when seeking off the end, etc
                    player.next().await?;
                }
                self.visualizer.reset()?;
            }
            SetLoopStatus(loop_status) => {
                self.player.write().await.set_loop_status(loop_status);
            }
            SetShuffle(shuffle) => {
                self.player.write().await.set_shuffle(shuffle);
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

    async fn on_track_change(&mut self, track: Option<&Track>) -> Result<()> {
        self.album_art.set_track(track)?;
        self.ui.theme = match track.map(Theme::from_track) {
            Some(Ok(t)) => t,
            Some(Err(e)) => {
                error!("Failed to get theme for track {track:?}: {e}");
                Theme::default()
            }
            None => Theme::default(),
        };
        Ok(())
    }
}

struct Bounds {
    panel: Rect,
    now_playing: Rect,
    album_art: Rect,
    visualizer: Rect,
}

impl Bounds {
    fn new(area: Rect) -> Self {
        let [main, visualizer] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(4)])
            .splits(area);
        let [side, panel] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(32), Constraint::Min(1)])
            .splits(main);
        let [_padding, album_art, now_playing] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(16), Constraint::Min(1)])
            .splits(side);
        Self { panel, now_playing, visualizer, album_art }
    }
}

enum AppEvent {
    Terminal(Event),
    Message(Message),
    /// Sent every so often. This normally doesn't trigger anything in and of itself.
    Tick,
}

impl AppEvent {
    fn stream(
        terminal_events: impl Stream<Item = Event>,
        rx_message: UnboundedReceiver<Message>,
    ) -> impl Stream<Item = Self> {
        let ticks = tokio_stream::iter(std::iter::from_fn(|| Some(AppEvent::Tick)))
            .throttle(Duration::from_millis(50));

        UnboundedReceiverStream::new(rx_message)
            .map(AppEvent::Message)
            .merge(terminal_events.map(AppEvent::Terminal))
            .merge(Box::pin(ticks))
    }
}

trait LayoutExt {
    /// Convenient method to allow destructuring a split.
    fn splits<const N: usize>(&self, area: Rect) -> [Rect; N];
}

impl LayoutExt for Layout {
    fn splits<const N: usize>(&self, area: Rect) -> [Rect; N] {
        self.split(area).deref().try_into().unwrap()
    }
}
