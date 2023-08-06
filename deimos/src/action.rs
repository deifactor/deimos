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

use symphonia::core::audio::{AudioBuffer, Signal};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    app::App,
    artist_album_list::ArtistAlbumList,
    decoder::TrackingSymphoniaDecoder,
    library::{self, Track},
    now_playing::PlayState,
    spectrogram::Visualizer,
    track_list::TrackList,
    ui::FocusTarget,
};

/// An [`Action`] corresponds to a mutation of the application state. Actions
/// are semantic. For example, 'the user pressed the n key' is not a good
/// choice for an action, but 'the user wants to advance in the current list'
/// and 'the user input an n into the current text entry' are both good
pub enum Action {
    NextFocus,
    MoveSelection(isize),
    ToggleExpansion,
    SetArtists(HashMap<String, Vec<String>>),
    SetTracks(Vec<Track>),
    PlaySelectedTrack,
    SetNowPlaying(Option<PlayState>),
    UpdateSpectrum(AudioBuffer<f32>),
    Quit,
}

impl Action {
    pub fn dispatch(self, app: &mut App, sender: &UnboundedSender<Command>) -> Result<()> {
        use Action::*;
        match self {
            MoveSelection(amount) => match app.ui.focus {
                FocusTarget::ArtistAlbumList => {
                    app.artist_album_list.move_selection(amount);
                    sync_track_list(app, sender)?;
                }
                FocusTarget::TrackList => {
                    app.track_list.move_selection(amount);
                }
            },
            SetArtists(artists) => app.artist_album_list = ArtistAlbumList::new(artists),
            SetTracks(tracks) => app.track_list = TrackList::new(tracks),
            PlaySelectedTrack => {
                if let Some(selected) = app.track_list.selected() {
                    sender.send(Command::PlayTrack(selected.song_id))?;
                }
            }
            SetNowPlaying(play_state) => app.now_playing.play_state = play_state,
            ToggleExpansion => app.artist_album_list.toggle(),
            NextFocus => app.ui.focus = app.ui.focus.next(),
            UpdateSpectrum(buf) => {
                app.visualizer.update_spectrum(buf.chan(0)).unwrap();
            }
            Quit => panic!("bye"),
        }
        Ok(())
    }
}

/// A [`Command`] talks to the external world in some way that we don't want to
/// block on. For example, downloading data from the internet and talking to
/// the database should both be done through a [`Command`].
#[derive(Debug)]
pub enum Command {
    LoadLibrary,
    LoadTracks { artist: String, album: String },
    PlayTrack(i64),
}

impl Command {
    async fn execute(
        self,
        pool: &Pool<Sqlite>,
        sink: &Sink,
        tx_action: &UnboundedSender<Action>,
    ) -> Result<Option<Action>> {
        use Command::*;
        let action = match self {
            LoadLibrary => {
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

            LoadTracks { artist, album } => {
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

            PlayTrack(song_id) => {
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

fn sync_track_list(app: &mut App, sender: &UnboundedSender<Command>) -> Result<(), anyhow::Error> {
    let artist = app.artist_album_list.artist();
    let album = app.artist_album_list.album();
    if let (Some(artist), Some(album)) = (artist, album) {
        sender.send(Command::LoadTracks { artist, album })?;
    } else {
        Action::SetTracks(vec![]).dispatch(app, sender)?;
    }
    Ok(())
}
