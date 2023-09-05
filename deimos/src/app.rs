use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use itertools::Itertools;
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
    library::{AlbumId, ArtistId, Library, Track},
    library_panel::LibraryPanel,
    ui::{
        artist_album_list::ArtistAlbumList,
        now_playing::{NowPlaying, PlayState},
        search::{Search, SearchResult},
        spectrogram::Visualizer,
        track_list::{TrackList, TrackListItem},
        DeimosBackend, Ui,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Play,
    Search,
}

pub struct App {
    library: Library,
    player_sink: Sink,
    library_panel: LibraryPanel,
    now_playing: NowPlaying,
    visualizer: Visualizer,
    search: Search,
    mode: Mode,
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
            mode: Mode::Play,
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

        let (tx_action, rx_action) = unbounded_channel::<Action>();
        pin!(terminal_events);

        let mut event_stream = AppEvent::stream(terminal_events, rx_action);

        while let Some(event) = event_stream.next().await {
            self.handle_event(event, &tx_action)?;
            terminal.draw(|f| self.draw(f).expect("failed to rerender app"))?;
            if self.should_quit {
                return Ok(());
            }
        }

        Ok(())
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    fn handle_event(&mut self, event: AppEvent, tx_action: &UnboundedSender<Action>) -> Result<()> {
        let action = match event {
            AppEvent::Terminal(terminal_event) => {
                if let Some(action) = self.handle_terminal(terminal_event) {
                    self.dispatch(action, tx_action)?
                } else {
                    None
                }
            }
            AppEvent::Action(action) => self.dispatch(action, tx_action)?,
        };
        if let Some(action) = action {
            self.handle_event(AppEvent::Action(action), tx_action)?;
        }
        Ok(())
    }

    pub fn draw(&mut self, f: &mut Frame<'_, DeimosBackend>) -> Result<()> {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Max(6)])
            .split(f.size());
        match self.mode {
            Mode::Play => self.library_panel.draw(&self.ui, f, root[0])?,
            Mode::Search => self.search.draw(&self.ui, f, root[0])?,
        }
        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(root[1]);
        self.now_playing.draw(&self.ui, f, bottom[0])?;
        self.visualizer.draw(&self.ui, f, bottom[1])?;
        Ok(())
    }

    fn handle_terminal(&mut self, ev: Event) -> Option<Action> {
        let Event::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = ev else { return None };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Some(Action::Quit),
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.search = Search::default();
            }
            _ => {
                return match self.mode {
                    Mode::Play => self.library_panel.handle_keycode(code),
                    Mode::Search => self.search.handle_keycode(code),
                }
            }
        }
        None
    }
}

/// An [`Action`] corresponds to a mutation of the application state. All mutation of application
/// state should be done through actions.
pub enum Action {
    LibraryTreeItemSelected {
        artist: ArtistId,
        album: Option<AlbumId>,
    },
    SetNowPlaying(Option<PlayState>),
    UpdateSpectrum(AudioBuffer<f32>),
    RunSearch(String),
    SelectEntity(SearchResult),
    SelectEntityTracksLoaded(String),
    PlayTrack(Track),
    Quit,
}

impl App {
    pub fn dispatch(
        &mut self,
        action: Action,
        tx_action: &UnboundedSender<Action>,
    ) -> Result<Option<Action>> {
        use Action::*;
        match action {
            LibraryTreeItemSelected { artist, album } => match album {
                Some(album) => {
                    let tracks = self.library.artists[&artist].albums[&album].tracks.clone();
                    self.library_panel.track_list =
                        TrackList::new(tracks.into_iter().map(TrackListItem::Track).collect());
                }
                None => {
                    let mut albums = self.library.artists[&artist]
                        .albums
                        .iter()
                        .map(|(id, album)| (format!("{}", id), album.tracks.clone()))
                        .collect_vec();
                    albums.sort_unstable_by_key(|(id, _)| id.clone());
                    self.library_panel.track_list = TrackList::new(
                        albums
                            .into_iter()
                            .flat_map(|(title, tracks)| {
                                std::iter::once(TrackListItem::Section(title))
                                    .chain(tracks.into_iter().map(TrackListItem::Track))
                            })
                            .collect(),
                    );
                }
            },
            SetNowPlaying(play_state) => self.now_playing.play_state = play_state,
            UpdateSpectrum(buf) => {
                self.visualizer.update_spectrum(buf).unwrap();
            }
            RunSearch(query) => {
                let results = Search::run_search_query(&self.library, query)?;
                self.search.set_results(results);
            }
            SelectEntity(result) => {
                self.mode = Mode::Play;
                self.library_panel.select_entity(&result);
                self.dispatch(
                    LibraryTreeItemSelected {
                        artist: result.album_artist().clone(),
                        album: result.album().cloned(),
                    },
                    tx_action,
                )?;
                if let Some(title) = result.track_title() {
                    return Ok(Some(SelectEntityTracksLoaded(title.to_owned())));
                }
            }
            SelectEntityTracksLoaded(title) => self.library_panel.track_list.select(&title),
            PlayTrack(track) => {
                let tx_action = tx_action.clone();
                let decoder = TrackingSymphoniaDecoder::from_path(&track.path)?.with_callback(
                    move |buffer, timestamp| {
                        tx_action
                            .send(Action::SetNowPlaying(Some(PlayState {
                                timestamp,
                                track: track.clone(),
                            })))
                            .unwrap();
                        tx_action.send(Action::UpdateSpectrum(buffer)).unwrap();
                    },
                );
                self.player_sink.stop();
                self.player_sink.append(decoder);
                self.player_sink.play();
            }
            Quit => self.quit(),
        }
        Ok(None)
    }
}

enum AppEvent {
    Terminal(Event),
    Action(Action),
}

impl AppEvent {
    fn stream(
        terminal_events: impl Stream<Item = Event>,
        rx_action: UnboundedReceiver<Action>,
    ) -> impl Stream<Item = Self> {
        UnboundedReceiverStream::new(rx_action)
            .map(AppEvent::Action)
            .merge(terminal_events.map(AppEvent::Terminal))
    }
}
