use std::{future::Future, pin::Pin, sync::Arc};

use mpris_server::{
    async_trait,
    zbus::{self, fdo, zvariant::ObjectPath},
    LoopStatus, PlaybackStatus, PlayerInterface, RootInterface, TrackId,
};
use tokio::sync::{mpsc::UnboundedSender, RwLock};

use crate::{
    app::{Command, Message},
    audio::Player,
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
}

// These two have to use the "manually-expanded" variant of async_trait's functions because
// async_trait's macro processing happens *before* inner macros expand.

/// Declares a method that sends `Command::$command`.
macro_rules! send_command {
    ($method:ident, $command:ident) => {
        fn $method<'life, 'async_>(
            &'life self,
        ) -> Pin<Box<dyn Future<Output = fdo::Result<()>> + Send + 'async_>>
        where
            'life: 'async_,
            Self: Sync + 'async_,
        {
            Box::pin(async move {
                self.tx.send(Message::Command(Command::$command)).unwrap();
                Ok(())
            })
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

/// TODO: replace all unwraps here with error mapping

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
    send_command!(quit, Quit);

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl PlayerInterface for MprisAdapter {
    // 'traditional' player controls
    send_command!(next, NextTrack);
    send_command!(previous, PreviousOrSeekToStart);
    send_command!(stop, Stop);
    send_command!(play, Play);
    send_command!(play_pause, PlayPause);
    send_command!(pause, Pause);

    returns!(can_play, bool, true);
    returns!(can_pause, bool, true);
    returns!(can_control, bool, true);
    returns!(can_go_next, bool, true);
    returns!(can_go_previous, bool, true);

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let player = self.player.read().await;
        Ok(if player.stopped() {
            PlaybackStatus::Stopped
        } else if player.playing() {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Paused
        })
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> zbus::Result<()> {
        todo!()
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        todo!()
    }

    // position inside a track

    async fn seek(&self, time: mpris_server::Time) -> fdo::Result<()> {
        self.tx.send(Message::Command(Command::Seek(time.as_secs()))).unwrap();
        Ok(())
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

    async fn set_position(&self, _track_id: TrackId, _time: mpris_server::Time) -> fdo::Result<()> {
        todo!()
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
        let track_id: ObjectPath = "/1234".try_into().unwrap();
        let metadata = mpris_server::Metadata::builder().trackid(track_id).build();
        Ok(metadata)
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("can't open URIs".into()))
    }
}
