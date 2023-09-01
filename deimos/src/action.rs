/// Defines [`Action`], which updates the model in some way, and [`Command`],
/// which performs some kind of async blocking operation.
///
/// These are enums instead of a trait because:
///
/// - We don't have to box them all the time (performance doesn't matter, but it's verbose)
/// - We can make their methods take them by move (can't call a by-move method on a boxed trait object)
/// - Less verbose to declare a new action
use anyhow::Result;

use itertools::Itertools;
use rodio::Sink;

use symphonia::core::audio::AudioBuffer;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{App, Mode},
    decoder::TrackingSymphoniaDecoder,
    library::{AlbumId, ArtistId, Library, Track},
    ui::{
        now_playing::PlayState,
        search::{Search, SearchResult},
        track_list::{TrackList, TrackListItem},
    },
};

/// An [`Action`] corresponds to a mutation of the application state. Actions
/// should only be used in cases where the mutation can't be done in the
/// function creating it, either because
///
/// - it's a component handler, which only has a mutable reference to that
/// component
/// - it's in a [`Command::execute`] implementation, which doesn't
/// have a reference to the app state at all
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

impl Action {
    pub fn dispatch(
        self,
        app: &mut App,
        tx_action: &UnboundedSender<Action>,
    ) -> Result<Option<Command>> {
        use Action::*;
        match self {
            LibraryTreeItemSelected { artist, album } => match album {
                Some(album) => {
                    let tracks = app.library.artists[&artist].albums[&album].tracks.clone();
                    app.library_panel.track_list =
                        TrackList::new(tracks.into_iter().map(TrackListItem::Track).collect());
                }
                None => {
                    let mut albums = app.library.artists[&artist]
                        .albums
                        .iter()
                        .map(|(id, album)| (format!("{}", id), album.tracks.clone()))
                        .collect_vec();
                    albums.sort_unstable_by_key(|(id, _)| id.clone());
                    app.library_panel.track_list = TrackList::new(
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
            SetNowPlaying(play_state) => app.now_playing.play_state = play_state,
            UpdateSpectrum(buf) => {
                app.visualizer.update_spectrum(buf).unwrap();
            }
            RunSearch(query) => {
                let results = Search::run_search_query(&app.library, query)?;
                app.search.set_results(results);
            }
            SelectEntity(result) => {
                app.mode = Mode::Play;
                app.library_panel.select_entity(&result);
                LibraryTreeItemSelected {
                    artist: result.album_artist().clone(),
                    album: result.album().cloned(),
                }
                .dispatch(app, tx_action)?;
                if let Some(title) = result.track_title() {
                    return Ok(Some(Command::RunAction(SelectEntityTracksLoaded(
                        title.to_owned(),
                    ))));
                }
            }
            SelectEntityTracksLoaded(title) => app.library_panel.track_list.select(&title),
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
                app.player_sink.stop();
                app.player_sink.append(decoder);
                app.player_sink.play();
            }
            Quit => app.quit(),
        }
        Ok(None)
    }
}

/// A [`Command`] is the way for components to either talk to the external
/// world a nonblocking way or apply a global mutation to the application
/// state. For example, downloading data from the internet and talking to the
/// database should both be done through a [`Command`], and the artist/album
/// tree browser needs to send a command to update the track list.
pub enum Command {
    RunAction(Action),
}

impl Command {
    pub fn execute(
        self,
        _library: &Library,
        _sink: &Sink,
        _tx_action: &UnboundedSender<Action>,
    ) -> Result<Option<Action>> {
        let action = match self {
            Command::RunAction(action) => Some(action),
        };
        Ok(action)
    }
}
