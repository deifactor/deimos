use std::{iter, sync::Arc, time::Duration};

use cpal::{
    traits::{DeviceTrait, HostTrait},
    Sample, SampleRate, Stream,
};
use educe::Educe;
use eyre::{eyre, Result};
use fragile::Fragile;
use itertools::Itertools;
use log::error;
use mpris_server::LoopStatus;
use symphonia::core::audio::{AudioBuffer, SampleBuffer};
use tokio::sync::{mpsc::UnboundedSender, Mutex, RwLock};

use crate::{app::Message, library::Track};

use self::{
    play_queue::PlayQueue,
    reader::{Fragment, SymphoniaReader},
};

mod play_queue;
mod reader;

pub struct Player {
    /// Provides an iterator over indiviudal samples as well as access to the underlying reader.
    source: Arc<Mutex<Option<Source>>>,
    tx_message: UnboundedSender<Message>,

    /// If true, playback is paused. If there are no songs in the queue, the value of this is not
    /// specified.
    paused: Arc<RwLock<bool>>,

    // note: not set by the [`Player`] itself, but by the [`App`] on receiving a [`PlayerMessage`]
    timestamp: Option<Duration>,

    queue: PlayQueue,

    /// Streams audio to the underlying OS audio library. This has a sample rate/channel count
    /// corresponding to the currently playing song. This is wrapped in [`Fragile`] so that other
    /// threads can read the player state; we don't make this publicly readable anywhere.
    stream: Option<Fragile<Stream>>,
}

#[derive(Educe)]
#[educe(Debug)]
pub enum PlayerMessage {
    AudioFragment {
        #[educe(Debug(ignore))]
        buffer: AudioBuffer<f32>,
        timestamp: Duration,
    },
    Finished,
}

impl Player {
    pub fn new(tx_message: UnboundedSender<Message>) -> Result<Self> {
        let source: Arc<Mutex<Option<Source>>> = Arc::new(Mutex::new(None));
        let paused = Arc::new(RwLock::new(true));

        Ok(Self {
            source,
            tx_message,
            paused,
            timestamp: None,
            queue: PlayQueue::default(),
            stream: None,
        })
    }

    /// Build a `Stream` that can handle playback with the given channel count and sample rate.
    fn build_stream(&self, channels: u16, sample_rate: u32) -> Result<Stream> {
        let host = cpal::default_host();
        let device =
            host.default_output_device().ok_or_else(|| eyre!("no default output device"))?;
        let config = device
            .supported_output_configs()?
            .find(|config| config.channels() == channels)
            .ok_or_else(|| eyre!("unable to find config supporting {channels} channels"))?
            .with_sample_rate(SampleRate(sample_rate))
            .config();
        let source_clone = Arc::clone(&self.source);
        let paused_clone = Arc::clone(&self.paused);
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                match source_clone.blocking_lock().as_mut() {
                    Some(iter) if !*paused_clone.blocking_read() => {
                        // copy from src to dst, zeroing the rest
                        for (dst, src) in
                            data.iter_mut().zip(iter.chain(iter::repeat(f32::EQUILIBRIUM)))
                        {
                            *dst = src
                        }
                    }
                    // no data, so just zero the entire thing
                    _ => data.fill(f32::EQUILIBRIUM),
                }
            },
            |e| {
                error!("Error while streaming audio out: {e}");
            },
            None,
        )?;
        Ok(stream)
    }

    pub fn queue(&self) -> &PlayQueue {
        &self.queue
    }

    /// The currently-playing track.
    pub fn current(&self) -> Option<Arc<Track>> {
        self.queue.current_track()
    }

    pub fn timestamp(&self) -> Option<Duration> {
        self.timestamp
    }

    pub fn set_timestamp(&mut self, timestamp: Option<Duration>) {
        self.timestamp = timestamp;
    }

    /// Sets the play queue to the given playlist. Also stops existing playback.
    pub async fn set_play_queue(&mut self, tracks: Vec<Arc<Track>>) {
        self.stop().await;
        let mut queue = PlayQueue::new(tracks);
        queue.set_loop_status(self.queue.loop_status());
        queue.set_shuffle(self.queue.shuffle());
        self.queue = queue;
    }

    pub fn queue_push(&mut self, track: Arc<Track>) {
        self.queue.push(track);
    }

    /// Sets the current track to the one at the given position. Panics if that's out of bounds.
    pub async fn set_queue_index(&mut self, index: Option<usize>) -> Result<()> {
        if index.is_none() {
            self.stop().await;
            return Ok(());
        }

        self.queue.set_current(index);
        let track =
            self.queue.current_track().expect("set current index to non-None, but no track");

        let reader = SymphoniaReader::from_path(&track.path)?;
        self.stream =
            Some(Fragile::new(self.build_stream(reader.channels() as u16, reader.sample_rate())?));

        let reader = Arc::new(Mutex::new(reader));
        let tx_message = self.tx_message.clone();
        let on_decode: DecodeCallback = Box::new(move |fragment| {
            let _ = tx_message.send(Message::Player(PlayerMessage::AudioFragment {
                buffer: fragment.buffer,
                timestamp: fragment.timestamp,
            }));
        });
        let tx_message = self.tx_message.clone();
        let on_finish: FinishCallback = Box::new(move || {
            let _ = tx_message.send(Message::Player(PlayerMessage::Finished));
        });
        let source = Source::new(reader, on_decode, on_finish);
        *self.source.lock().await = Some(source);
        Ok(())
    }
}

/// Functions related to playback control.
impl Player {
    pub async fn previous(&mut self) -> Result<()> {
        self.set_queue_index(self.queue.previous()).await
    }

    /// Unpauses. If there is no selected track, selects the first track.
    pub async fn play(&mut self) -> Result<()> {
        if self.queue.current_track().is_none() && !self.queue.is_empty() {
            self.set_queue_index(Some(0)).await?;
        }
        self.set_paused(false).await;
        Ok(())
    }

    /// True if audio is being produced (i.e., we're not paused *and* there's a current song).
    pub async fn playing(&self) -> bool {
        !self.paused().await && self.current().is_some()
    }

    pub fn stopped(&self) -> bool {
        self.queue.current().is_none()
    }

    pub async fn paused(&self) -> bool {
        *self.paused.write().await
    }

    pub async fn pause(&mut self) {
        if self.playing().await {
            self.set_paused(true).await;
        }
    }

    async fn set_paused(&mut self, paused: bool) {
        *self.paused.write().await = paused;
    }

    /// Moves to the next track. If this was the last track, equivalent to stop().
    pub async fn next(&mut self) -> Result<()> {
        self.set_queue_index(self.queue.next()).await
    }

    /// Stops playback. This also unsets our position in the play queue.
    pub async fn stop(&mut self) {
        self.queue.set_current(None);
        *self.source.lock().await = None;
    }

    /// Seek to the given timestamp. Does nothing if there's no currently-playing track.
    pub async fn seek(&mut self, target: Duration) -> Result<()> {
        let mut source = self.source.lock().await;
        if let Some(source) = source.as_mut() {
            source.reader.lock().await.seek(target)
        } else {
            Ok(())
        }
    }

    pub fn set_loop_status(&mut self, loop_status: LoopStatus) {
        self.queue.set_loop_status(loop_status)
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.queue.set_shuffle(shuffle)
    }
}

type DecodeCallback = Box<dyn FnMut(Fragment) + Send + 'static>;
type FinishCallback = Box<dyn FnOnce() + Send + 'static>;

/// Iterates over the samples of a reader, invoking callbacks on decode and on finish. Also
/// provides access to the underlying reader so you can seek on it.
struct Source {
    reader: Arc<Mutex<SymphoniaReader>>,
    iterator: Box<dyn Send + Iterator<Item = f32>>,
}

impl Source {
    fn new(
        reader: Arc<Mutex<SymphoniaReader>>,
        mut on_decode: DecodeCallback,
        on_finish: FinishCallback,
    ) -> Self {
        let reader_clone = Arc::clone(&reader);
        let mut on_finish = Some(on_finish);
        let iterator = iter::from_fn(move || {
            let samples = reader_clone.blocking_lock().next().map(|fragment| {
                let buffer = &fragment.buffer;
                let mut samples = SampleBuffer::new(buffer.capacity() as u64, *buffer.spec());
                samples.copy_interleaved_typed(buffer);
                (on_decode)(fragment);
                samples
            });
            if samples.is_none() {
                if let Some(f) = on_finish.take() {
                    f()
                }
            }
            samples
        })
        .flat_map(|samples| samples.samples().iter().copied().collect_vec())
        .fuse();

        Self { reader, iterator: Box::new(iterator) }
    }
}

impl Iterator for Source {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.iterator.next()
    }
}
