use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use mpris_server::{
    async_trait,
    zbus::{self, fdo},
    LoopStatus, PlaybackStatus, PlayerInterface, RootInterface, TrackId,
};
use tokio::sync::{mpsc::UnboundedSender, RwLock};

use crate::{
    app::{Command, Message},
    audio::Player,
    library::ArtistName,
};

/// Mediates between the `App` struct and the [`RootInterface`] and [`PlayerInterface`] that we
/// need to implement.
pub(crate) struct MprisAdapter {
    tx: UnboundedSender<Message>,
    player: Arc<RwLock<Player>>,
}

impl MprisAdapter {
    pub fn new(tx: UnboundedSender<Message>, player: Arc<RwLock<Player>>) -> Self {
        Self { tx, player }
    }

    /// Sends a command to the main task. This should never fail, but in case it does we return an
    /// err rather than dying.
    fn send_command(&self, command: Command) -> fdo::Result<()> {
        self.tx
            .send(Message::Command(command))
            .map_err(|e| fdo::Error::Failed(format!("failed to send command to main task: {e}")))
    }
}

// These two have to use the "manually-expanded" variant of async_trait's functions because
// async_trait's macro processing happens *before* inner macros expand.

/// Declares a method that sends `Command::$command`.
macro_rules! sends_command {
    ($method:ident, $command:ident) => {
        fn $method<'life, 'async_>(
            &'life self,
        ) -> Pin<Box<dyn Future<Output = fdo::Result<()>> + Send + 'async_>>
        where
            'life: 'async_,
            Self: Sync + 'async_,
        {
            Box::pin(async move { self.send_command(Command::$command) })
        }
    };
}

/// Declares a method that returns `expr`.
macro_rules! returns {
    ($method:ident, $ty:ty, $val:expr) => {
        fn $method<'life, 'async_>(
            &'life self,
        ) -> Pin<Box<dyn Future<Output = fdo::Result<$ty>> + Send + 'async_>>
        where
            'life: 'async_,
            Self: Sync + 'async_,
        {
            Box::pin(async move { Ok($val) })
        }
    };
}

#[async_trait]
impl RootInterface for MprisAdapter {
    returns!(identity, String, "deimos".into());
    returns!(desktop_entry, String, "".into());
    returns!(supported_mime_types, Vec<String>, vec![]);
    returns!(supported_uri_schemes, Vec<String>, vec![]);
    returns!(raise, (), ());
    returns!(can_raise, bool, false);
    returns!(can_quit, bool, true);
    returns!(fullscreen, bool, false);
    returns!(can_set_fullscreen, bool, false);
    returns!(has_track_list, bool, false);
    sends_command!(quit, Quit);

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl PlayerInterface for MprisAdapter {
    // 'traditional' player controls
    sends_command!(next, NextTrack);
    sends_command!(previous, PreviousOrSeekToStart);
    sends_command!(stop, Stop);
    sends_command!(play, Play);
    sends_command!(play_pause, PlayPause);
    sends_command!(pause, Pause);

    returns!(can_play, bool, true);
    returns!(can_pause, bool, true);
    returns!(can_control, bool, true);
    returns!(can_go_next, bool, true);
    returns!(can_go_previous, bool, true);

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let player = self.player.read().await;
        Ok(if player.stopped() {
            PlaybackStatus::Stopped
        } else if player.playing().await {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Paused
        })
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(self.player.read().await.queue().loop_status())
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> zbus::Result<()> {
        self.send_command(Command::SetLoopStatus(loop_status))?;
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.player.read().await.queue().shuffle())
    }

    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        self.send_command(Command::SetShuffle(shuffle))?;
        Ok(())
    }

    // position inside a track

    async fn seek(&self, time: mpris_server::Time) -> fdo::Result<()> {
        self.send_command(Command::Seek(time.as_secs()))
    }
    returns!(can_seek, bool, true);

    async fn position(&self) -> fdo::Result<mpris_server::Time> {
        let timestamp = self
            .player
            .read()
            .await
            .timestamp()
            .ok_or(fdo::Error::Failed("no current song".into()))?;
        Ok(mpris_server::Time::from_micros(timestamp.as_micros() as i64))
    }

    async fn set_position(&self, track_id: TrackId, time: mpris_server::Time) -> fdo::Result<()> {
        let position = Duration::from_micros(time.as_micros() as u64);
        self.send_command(Command::SetPositionIfTrack { position, mpris_id: track_id })
    }

    // rate

    returns!(rate, f64, 1.0);
    returns!(minimum_rate, f64, 1.0);
    returns!(maximum_rate, f64, 1.0);
    async fn set_rate(&self, _rate: f64) -> zbus::Result<()> {
        Err(fdo::Error::NotSupported("can't set rate".into()).into())
    }

    // misc

    async fn volume(&self) -> fdo::Result<mpris_server::Volume> {
        Ok(1.0)
    }

    async fn set_volume(&self, _volume: f64) -> zbus::Result<()> {
        todo!()
    }

    async fn metadata(&self) -> fdo::Result<mpris_server::Metadata> {
        let track = self
            .player
            .read()
            .await
            .current()
            .ok_or(fdo::Error::Failed("no current song".into()))?;
        let mut builder = mpris_server::Metadata::builder().trackid(track.mpris_id());
        if let Some(title) = track.title.as_ref() {
            builder = builder.title(title)
        }
        if let ArtistName::Artist(artist) = &track.artist {
            builder = builder.artist(vec![artist.clone()]);
        }
        if let Some(album) = track.album.0.as_ref() {
            builder = builder.album(album);
        }
        if let Some(track_number) = track.number {
            builder = builder.track_number(track_number as i32);
        }
        let builder =
            builder.length(mpris_server::Time::from_micros((track.length.0 * 1_000_000.0) as i64));
        Ok(builder.build())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("can't open URIs".into()))
    }
}
