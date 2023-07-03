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

use sqlx::{Connection, Pool, Sqlite};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{app::App, artist_album_list::ArtistAlbumList, library};

/// An [`Action`] corresponds to a mutation of the application state. Actions
/// are semantic. For example, 'the user pressed the n key' is not a good
/// choice for an action, but 'the user wants to advance in the current list'
/// and 'the user input an n into the current text entry' are both good
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    NextFocus,
    NextList,
    ToggleExpansion,
    SetArtists(HashMap<String, Vec<String>>),
    Quit,
}

impl Action {
    pub fn dispatch(self, app: &mut App, _sender: &UnboundedSender<Command>) -> Result<()> {
        use Action::*;
        match self {
            NextList => app.artist_album_list.next(),
            SetArtists(artists) => app.artist_album_list = ArtistAlbumList::new(artists),
            ToggleExpansion => app.artist_album_list.toggle(),
            NextFocus => (),
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
}

impl Command {
    async fn execute(self, pool: &Pool<Sqlite>) -> Result<Option<Action>> {
        use Command::*;
        let action = match self {
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

                let mut artists: HashMap<String, Vec<String>> = HashMap::new();
                sqlx::query!(
                    r#"SELECT DISTINCT artist AS "artist!", album AS "album!"
                       FROM songs WHERE artist IS NOT NULL AND album IS NOT NULL
                       ORDER BY artist, album"#
                )
                .fetch_all(&mut conn)
                .await?
                .into_iter()
                .for_each(|row| artists.entry(row.artist).or_default().push(row.album));
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
