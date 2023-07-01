/// Defines [`Action`], which updates the model in some way, and [`Command`],
/// which performs some kind of async blocking operation.
///
/// These both could be enums instead of traits, but using traits allows us to
/// not have to centralize every single action/command definition in a single
/// place.
use std::fmt::Debug;

use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Connection, Pool, Sqlite};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    app::{App, BrowseList},
    library,
};

pub type OptionAction = Option<Box<dyn Action>>;

/// An [`Action`] corresponds to a mutation of the application state. Actions
/// are semantic. For example, 'the user pressed the n key' is not a good
/// choice for an action, but 'the user wants to advance in the current list'
/// and 'the user input an n into the current text entry' are both good
/// choices.
pub trait Action: Debug + Send + Sync + 'static {
    /// Modifies the app in accordance with the action. The `send` parameter
    /// allows an action to schedule one or more [`Command`]s to be performed.
    /// If you want to follow up with another action, return it instead of
    /// executing it directly; this lets the application log all actions that
    /// occur, allowing for better debugging.
    fn dispatch(&mut self, app: &mut App, sender: &CommandSender) -> Result<OptionAction>;

    /// For ease of returning from `Command::execute` and `Action::dispatch`.
    fn wrap(self) -> Result<Option<Box<dyn Action>>>
    where
        Self: Sized,
    {
        Ok(Some(Box::new(self)))
    }
}

/// A [`Command`] talks to the external world in some way that we don't want to
/// block on. For example, downloading data from the internet and talking to
/// the database should both be done through a [`Command`].
#[async_trait]
pub trait Command: Debug + Send + Sync + 'static {
    async fn execute(&mut self, pool: &Pool<Sqlite>) -> Result<OptionAction>;
}

/// Spawns an executor task that will forever execute any commands sent via the returned command sender.
pub fn spawn_executor(
    pool: Pool<Sqlite>,
    send_action: UnboundedSender<Box<dyn Action>>,
) -> CommandSender {
    let (tx_cmd, mut rx_cmd) = unbounded_channel::<Box<dyn Command>>();
    tokio::spawn(async move {
        while let Some(mut command) = rx_cmd.recv().await {
            if let Some(action) = command.execute(&pool).await? {
                send_action.send(action)?;
            }
        }
        anyhow::Ok(())
    });
    CommandSender(tx_cmd)
}

/// Wrapper around a channel sender that will box the command up for us.
#[derive(Debug, Clone)]
pub struct CommandSender(UnboundedSender<Box<dyn Command>>);

impl CommandSender {
    pub fn send<C: Command>(&self, command: C) -> Result<()> {
        self.0.send(Box::new(command))?;
        Ok(())
    }
}

// Some commands; move those to a separate file later.

#[derive(Debug)]
pub struct Quit;

impl Action for Quit {
    fn dispatch(&mut self, _app: &mut App, _sender: &CommandSender) -> Result<OptionAction> {
        panic!("bye")
    }
}

#[derive(Debug)]
pub struct NextFocus;

impl Action for NextFocus {
    fn dispatch(&mut self, app: &mut App, _sender: &CommandSender) -> Result<OptionAction> {
        app.focus = app.focus.next();
        Ok(None)
    }
}

#[derive(Debug)]
pub struct NextList;

impl Action for NextList {
    fn dispatch(&mut self, app: &mut App, sender: &CommandSender) -> Result<OptionAction> {
        app.artists.next();
        sender.send(LoadAlbums {
            artist: app.artists.items[app.artists.state.selected().unwrap()].clone(),
        })?;

        Ok(None)
    }
}

#[derive(Debug)]
pub struct SetArtists(Vec<String>);

impl Action for SetArtists {
    fn dispatch(&mut self, app: &mut App, _sender: &CommandSender) -> Result<OptionAction> {
        app.artists = BrowseList::new(self.0.clone());
        Ok(None)
    }
}

#[derive(Debug)]
pub struct SetAlbums(Vec<String>);

impl Action for SetAlbums {
    fn dispatch(&mut self, app: &mut App, _sender: &CommandSender) -> Result<OptionAction> {
        app.albums = BrowseList::new(self.0.clone());
        Ok(None)
    }
}

/// Performs the initial load of the music library.
#[derive(Debug)]
pub struct LoadAlbums {
    artist: String,
}

#[async_trait]
impl Command for LoadAlbums {
    async fn execute(&mut self, pool: &Pool<Sqlite>) -> Result<OptionAction> {
        let mut conn = pool.acquire().await?;
        let albums = sqlx::query_scalar!(r#"SELECT DISTINCT album AS "album!" FROM songs WHERE artist = ? AND album IS NOT NULL ORDER BY album DESC"#,
        self.artist)
            .fetch_all(&mut conn)
            .await?;
        SetAlbums(albums).wrap()
    }
}

/// Loads the albums 1
#[derive(Debug)]
pub struct LoadLibrary;

#[async_trait]
impl Command for LoadLibrary {
    async fn execute(&mut self, pool: &Pool<Sqlite>) -> Result<OptionAction> {
        let mut conn = pool.acquire().await?;
        let count = sqlx::query!("SELECT COUNT(*) AS count FROM songs")
            .fetch_one(&mut conn)
            .await?
            .count;
        // only reinitialize db if there are no songs
        if count == 0 {
            conn.transaction(|conn| {
                Box::pin(async move { library::find_music("/home/vector/music", conn).await })
            })
            .await?;
        }
        let artists = sqlx::query_scalar!(
            r#"SELECT DISTINCT artist AS "artist!" FROM songs WHERE artist IS NOT NULL"#
        )
        .fetch_all(&mut conn)
        .await?;
        SetArtists(artists).wrap()
    }
}
