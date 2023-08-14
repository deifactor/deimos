/// Defines [`Action`], which updates the model in some way, and [`Command`],
/// which performs some kind of async blocking operation.
///
/// These are enums instead of a trait because:
///
/// - We don't have to box them all the time (performance doesn't matter, but it's verbose)
/// - We can make their methods take them by move (can't call a by-move method on a boxed trait object)
/// - Less verbose to declare a new action
use std::{collections::HashMap, fmt::Debug};

use anyhow::Result;

use rodio::Sink;
use sqlx::{Connection, Pool, Sqlite};

use symphonia::core::audio::AudioBuffer;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    app::App,
    decoder::TrackingSymphoniaDecoder,
    library::{self, Track},
    ui::{
        artist_album_list::ArtistAlbumList,
        now_playing::PlayState,
        search::{Search, SearchResult},
        track_list::TrackList,
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
    SetArtists(HashMap<String, Vec<String>>),
    SetTracks(Vec<Track>),
    SetNowPlaying(Option<PlayState>),
    UpdateSpectrum(AudioBuffer<f32>),
    SetSearchResults(Vec<SearchResult>),
    Quit,
}

impl Action {
    pub fn dispatch(self, app: &mut App, _sender: &UnboundedSender<Command>) -> Result<()> {
        use Action::*;
        match self {
            SetArtists(artists) => app.artist_album_list = ArtistAlbumList::new(artists),
            SetTracks(tracks) => app.track_list = TrackList::new(tracks),
            SetNowPlaying(play_state) => app.now_playing.play_state = play_state,
            UpdateSpectrum(buf) => {
                app.visualizer.update_spectrum(buf).unwrap();
            }
            SetSearchResults(results) => app.search.set_results(results),
            Quit => panic!("bye"),
        }
        Ok(())
    }
}

/// A [`Command`] is the way for components to either talk to the external
/// world a nonblocking way or apply a global mutation to the application
/// state. For example, downloading data from the internet and talking to the
/// database should both be done through a [`Command`], and the artist/album
/// tree browser needs to send a command to update the track list.
pub enum Command {
    LoadLibrary,
    LoadTracks { artist: String, album: String },
    PlayTrack(i64),
    Search { query: String },
    RunAction(Action),
}

impl Command {
    async fn execute(
        self,
        pool: &Pool<Sqlite>,
        sink: &Sink,
        tx_action: &UnboundedSender<Action>,
    ) -> Result<Option<Action>> {
        let action = match self {
            Command::LoadLibrary => {
                let mut conn = pool.acquire().await?;
                let count = sqlx::query!("SELECT COUNT(*) AS count FROM songs")
                    .fetch_one(&mut *conn)
                    .await?
                    .count;
                // only reinitialize db if there are no songs
                if count == 0 {
                    conn.transaction(|conn| {
                        Box::pin(
                            async move { library::find_music("/home/vector/music", conn).await },
                        )
                    })
                    .await?;
                }

                let mut artists: HashMap<String, Vec<String>> = HashMap::new();
                sqlx::query!(
                    r#"SELECT DISTINCT artist AS "artist!", album AS "album!"
                       FROM songs WHERE artist IS NOT NULL AND album IS NOT NULL
                       ORDER BY artist, album"#
                )
                .fetch_all(&mut *conn)
                .await?
                .into_iter()
                .for_each(|row| artists.entry(row.artist).or_default().push(row.album));
                Some(Action::SetArtists(artists))
            }

            Command::LoadTracks { artist, album } => {
                let mut conn = pool.acquire().await?;
                let tracks = sqlx::query_as!(
                    Track,
                    r#"SELECT *
                       FROM songs WHERE artist = ? AND album = ?
                       ORDER BY number ASC NULLS FIRST"#,
                    artist,
                    album
                )
                .fetch_all(&mut *conn)
                .await?;
                Some(Action::SetTracks(tracks))
            }

            Command::PlayTrack(song_id) => {
                let mut conn = pool.acquire().await?;
                let track =
                    sqlx::query_as!(Track, "SELECT * FROM songs WHERE song_id = ?", song_id)
                        .fetch_one(&mut *conn)
                        .await?;
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
                sink.stop();
                sink.append(decoder);
                sink.play();
                None
            }

            Command::Search { query } => {
                let mut conn = pool.acquire().await?;
                let results = conn
                    .transaction(|tx| {
                        Box::pin(async move { Search::run_search_query(&query, tx).await })
                    })
                    .await?;
                Some(Action::SetSearchResults(results))
            }

            Command::RunAction(action) => Some(action),
        };
        Ok(action)
    }

    /// Spawns an executor task that will forever execute any commands sent via the returned command sender.
    pub fn spawn_executor(
        pool: Pool<Sqlite>,
        sink: Sink,
        send_action: UnboundedSender<Action>,
    ) -> UnboundedSender<Command> {
        let (tx_cmd, mut rx_cmd) = unbounded_channel::<Command>();
        tokio::spawn(async move {
            while let Some(command) = rx_cmd.recv().await {
                if let Some(action) = command.execute(&pool, &sink, &send_action).await.unwrap() {
                    send_action.send(action).unwrap();
                }
            }
            anyhow::Ok(())
        });
        tx_cmd
    }
}
