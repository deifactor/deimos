/// Defines [`Action`], which updates the model in some way, and [`Command`],
/// which performs some kind of async blocking operation.
///
/// These are enums instead of a trait because:
///
/// - We don't have to box them all the time (performance doesn't matter, but it's verbose)
/// - We can make their methods take them by move (can't call a by-move method on a boxed trait object)
/// - Less verbose to declare a new action
use std::fmt::Debug;

use anyhow::Result;
use ratatui::widgets::ListState;
use sqlx::{Connection, Pool, Sqlite};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{app::App, library};

/// An [`Action`] corresponds to a mutation of the application state. Actions
/// are semantic. For example, 'the user pressed the n key' is not a good
/// choice for an action, but 'the user wants to advance in the current list'
/// and 'the user input an n into the current text entry' are both good
#[derive(Debug)]
pub enum Action {
    NextFocus,
    NextList,
    SetArtists(Vec<String>),
    SetAlbums(Vec<String>),
    Quit,
}

impl Action {
    pub fn dispatch(self, app: &mut App, sender: &UnboundedSender<Command>) -> Result<()> {
        use Action::*;
        match self {
            NextFocus => app.focus = app.focus.next(),
            NextList => {
                app.artists.next();
                sender.send(Command::LoadAlbums {
                    artist: app.artists.items[app.artists.state.selected().unwrap()].clone(),
                })?;
            }
            SetArtists(artists) => {
                app.artists.items = artists;
                app.artists.state = ListState::default();
            }
            SetAlbums(albums) => {
                app.albums.items = albums;
                app.albums.state = ListState::default();
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
    LoadAlbums { artist: String },
    LoadLibrary,
}

impl Command {
    async fn execute(self, pool: &Pool<Sqlite>) -> Result<Option<Action>> {
        use Command::*;
        let action = match self {
            LoadAlbums { artist } => {
                let mut conn = pool.acquire().await?;
                let albums = sqlx::query_scalar!(r#"SELECT DISTINCT album AS "album!" FROM songs WHERE artist = ? AND album IS NOT NULL ORDER BY album DESC"#,
        artist)
            .fetch_all(&mut conn)
            .await?;
                Some(Action::SetAlbums(albums))
            }

            LoadLibrary => {
                let mut conn = pool.acquire().await?;
                let count = sqlx::query!("SELECT COUNT(*) AS count FROM songs")
                    .fetch_one(&mut conn)
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
                let artists = sqlx::query_scalar!(
                    r#"SELECT DISTINCT artist AS "artist!" FROM songs WHERE artist IS NOT NULL"#
                )
                .fetch_all(&mut conn)
                .await?;
                Some(Action::SetArtists(artists))
            }
        };
        Ok(action)
    }

    /// Spawns an executor task that will forever execute any commands sent via the returned command sender.
    pub fn spawn_executor(
        pool: Pool<Sqlite>,
        send_action: UnboundedSender<Action>,
    ) -> UnboundedSender<Command> {
        let (tx_cmd, mut rx_cmd) = unbounded_channel::<Command>();
        tokio::spawn(async move {
            while let Some(command) = rx_cmd.recv().await {
                if let Some(action) = command.execute(&pool).await? {
                    send_action.send(action)?;
                }
            }
            anyhow::Ok(())
        });
        tx_cmd
    }
}
