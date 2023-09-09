use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use enum_iterator::next_cycle;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};
use rodio::Sink;
use symphonia::core::audio::AudioBuffer;
use tokio::{
    pin,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};

use crate::{
    decoder::TrackingSymphoniaDecoder,
    library::{Library, Track},
    library_panel::{LibraryPanel, PanelItem},
    ui::{
        artist_album_list::ArtistAlbumList,
        now_playing::{NowPlaying, PlayState},
        search::{Search, SearchResult},
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
    player_sink: Sink,
    library_panel: LibraryPanel,
    now_playing: NowPlaying,
    visualizer: Visualizer,
    search: Search,
    active_panel: Panel,
    ui: Ui,
    should_quit: bool,
}

impl App {
    pub fn new(library: Library, player_sink: Sink) -> Self {
        Self {
            library,
            player_sink,
            library_panel: LibraryPanel::default(),
            now_playing: NowPlaying::default(),
            visualizer: Visualizer::default(),
            search: Search::default(),
            active_panel: Panel::Library,
            ui: Ui::default(),
            should_quit: false,
        }
    }

    pub async fn run(
        mut self,
        terminal_events: impl Stream<Item = Event> + Send + Sync + 'static,
        mut terminal: Terminal<DeimosBackend>,
    ) -> Result<()> {
        self.library_panel.artist_album_list = ArtistAlbumList::new(&self.library);

        let (tx_message, rx_message) = unbounded_channel::<Message>();
        pin!(terminal_events);

        let mut event_stream = AppEvent::stream(terminal_events, rx_message);

        while let Some(event) = event_stream.next().await {
            self.handle_event(event, &tx_message)?;
            terminal.draw(|f| self.draw(f).expect("failed to rerender app"))?;
            if self.should_quit {
                return Ok(());
            }
        }

        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        tx_message: &UnboundedSender<Message>,
    ) -> Result<()> {
        let message = match event {
            AppEvent::Terminal(terminal_event) => {
                if let Some(message) = self.handle_terminal(terminal_event) {
                    self.dispatch(message, tx_message)?
                } else {
                    None
                }
            }
            AppEvent::Message(message) => self.dispatch(message, tx_message)?,
        };
        if let Some(message) = message {
            self.handle_event(AppEvent::Message(message), tx_message)?;
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

    fn handle_terminal(&self, ev: Event) -> Option<Message> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else { return None };
        self.key_to_command(code)
            .and_then(|cmd| self.dispatch_command(cmd))
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
    StartSearch,
    MoveCursor(Motion),
    ToggleArtistAlbumList,
    SetNowPlaying(Option<PlayState>),
    UpdateSpectrum(AudioBuffer<f32>),
    SetSearchQuery(String),
    SelectEntity(SearchResult),
    PlayTrack(Track),
    SetFocus(PanelItem),
    Quit,
}

/// A [`Command`] corresponds to a single user input. The translation of keys to commands is done
/// by a match statement on (active panel, keycode).
pub enum Command {
    /// Move focus to the next item in the panel.
    NextFocus,
    /// Start a new search.
    StartSearch,
    /// Perform an message on the currently-selected item.
    Activate,
    /// Move the current selection.
    MoveCursor(Motion),
    /// User typed someething into the search input.
    SearchInput(char),
    /// Deletes the most recent character in the search input.
    SearchBackspace,
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
            _ => return None,
        };
        Some(message)
    }
    fn dispatch(
        &mut self,
        message: Message,
        tx_message: &UnboundedSender<Message>,
    ) -> Result<Option<Message>> {
        use Message::*;
        match message {
            StartSearch => {
                self.active_panel = Panel::Search;
                self.search = Search::default();
            }
            SetSearchQuery(query) => {
                self.search.set_query(query);
                let results = Search::run_search_query(&self.library, self.search.query())?;
                self.search.set_results(results);
            }
            SetNowPlaying(play_state) => self.now_playing.play_state = play_state,
            UpdateSpectrum(buf) => {
                self.visualizer.update_spectrum(buf).unwrap();
            }
            SelectEntity(result) => {
                self.active_panel = Panel::Library;
                self.library_panel.select_entity(&self.library, &result)?;
            }
            PlayTrack(track) => {
                let tx_message = tx_message.clone();
                let decoder = TrackingSymphoniaDecoder::from_path(&track.path)?.with_callback(
                    move |buffer, timestamp| {
                        tx_message
                            .send(Message::SetNowPlaying(Some(PlayState {
                                timestamp,
                                track: track.clone(),
                            })))
                            .unwrap();
                        tx_message.send(Message::UpdateSpectrum(buffer)).unwrap();
                    },
                );
                self.player_sink.stop();
                self.player_sink.append(decoder);
                self.player_sink.play();
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
            SetFocus(focus) => {
                self.library_panel.focus = focus;
            }
            ToggleArtistAlbumList => self.library_panel.artist_album_list.toggle(),
        }
        Ok(None)
    }

    fn dispatch_command(&self, command: Command) -> Option<Message> {
        use Command::*;
        Some(match command {
            NextFocus => Message::SetFocus(next_cycle(&self.library_panel.focus).unwrap()),
            StartSearch => Message::StartSearch,
            Activate => match self.active_panel {
                Panel::Library => match self.library_panel.focus {
                    PanelItem::ArtistAlbumList => Message::ToggleArtistAlbumList,
                    PanelItem::TrackList => self
                        .library_panel
                        .track_list
                        .selected()
                        .cloned()
                        .map(Message::PlayTrack)?,
                },
                Panel::Search => self.search.selected_result().map(Message::SelectEntity)?,
            },
            MoveCursor(motion) => Message::MoveCursor(motion),
            SearchInput(c) => Message::SetSearchQuery(format!("{}{}", self.search.query(), c)),
            SearchBackspace => {
                let mut chars = self.search.query().chars();
                chars.next_back();
                Message::SetSearchQuery(chars.as_str().to_owned())
            }
            Quit => Message::Quit,
        })
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
