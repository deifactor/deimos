use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};
use rodio::Sink;
use tokio::{
    pin,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};

use crate::{
    action::Action,
    library::Library,
    library_panel::LibraryPanel,
    ui::{
        artist_album_list::ArtistAlbumList, now_playing::NowPlaying, search::Search,
        spectrogram::Visualizer, DeimosBackend, Ui,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Play,
    Search,
}

pub struct App {
    pub library: Library,
    pub player_sink: Sink,
    pub library_panel: LibraryPanel,
    pub now_playing: NowPlaying,
    pub visualizer: Visualizer,
    pub search: Search,
    pub mode: Mode,
    pub ui: Ui,
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
                    action.dispatch(self, tx_action)?
                } else {
                    None
                }
            }
            AppEvent::Action(action) => action.dispatch(self, tx_action)?,
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
