/// Defines [`Action`], which updates the model in some way.
use anyhow::Result;

use itertools::Itertools;

use symphonia::core::audio::AudioBuffer;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{App, Mode},
    decoder::TrackingSymphoniaDecoder,
    library::{AlbumId, ArtistId, Track},
    ui::{
        now_playing::PlayState,
        search::{Search, SearchResult},
        track_list::{TrackList, TrackListItem},
    },
};

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

impl Action {
    pub fn dispatch(
        self,
        app: &mut App,
        tx_action: &UnboundedSender<Action>,
    ) -> Result<Option<Action>> {
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
                    return Ok(Some(SelectEntityTracksLoaded(title.to_owned())));
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
