use std::{sync::Arc, time::Duration};

use anyhow::Result;
use rodio::{Sample, Sink, Source};
use symphonia::core::audio::AudioBuffer;
use tokio::sync::mpsc::UnboundedSender;

use crate::{app::Message, decoder::TrackingSymphoniaDecoder, library::Track};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayPosition {
    track_index: usize,
    timestamp: Duration,
}

pub struct Player {
    sink: Sink,
    tx_message: UnboundedSender<Message>,
}

/// The [`Player`] sends these messages up to the [`App`] as the track advances.
pub enum PlayerMessage {
    TrackFinished,
    Decoded {
        track: Arc<Track>,
        timestamp: Duration,
        buffer: AudioBuffer<f32>,
    },
}

impl Player {
    pub fn new(sink: Sink, tx_message: UnboundedSender<Message>) -> Self {
        Self { sink, tx_message }
    }

    pub fn play_track(&mut self, track: Arc<Track>) -> Result<()> {
        let tx_message = self.tx_message.clone();
        let decoder = TrackingSymphoniaDecoder::from_path(&track.path)?.with_callback(
            move |buffer, timestamp| {
                tx_message
                    .send(Message::Player(PlayerMessage::Decoded {
                        track: Arc::clone(&track),
                        buffer,
                        timestamp,
                    }))
                    .unwrap();
            },
        );
        let tx_message = self.tx_message.clone();
        let source = SourceWithFinishCallback::new(decoder, move || {
            tx_message
                .send(Message::Player(PlayerMessage::TrackFinished))
                .unwrap();
        });
        self.sink.stop();
        self.sink.append(source);
        self.sink.play();
        Ok(())
    }
}

/// Wraps a [`Source`], invoking a callback when it finishes. The callback is called at most once.
struct SourceWithFinishCallback<T> {
    callback: Option<Box<dyn FnOnce() + Send>>,
    inner: T,
}

impl<T> SourceWithFinishCallback<T>
where
    T: Source,
    T::Item: Sample,
{
    fn new(inner: T, callback: impl FnMut() + Send + 'static) -> Self {
        Self {
            inner,
            callback: Some(Box::new(callback)),
        }
    }
}

impl<T> Iterator for SourceWithFinishCallback<T>
where
    T: Source,
    T::Item: Sample,
{
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.inner.next();
        if next.is_none() {
            if let Some(callback) = self.callback.take() {
                callback();
            }
        }
        next
    }
}

impl<T> Source for SourceWithFinishCallback<T>
where
    T: Source,
    T::Item: Sample,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}
