use std::{
    iter,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait},
    Sample, Stream,
};
use educe::Educe;
use eyre::{eyre, Result};
use fragile::Fragile;
use itertools::Itertools;
use log::error;
use symphonia::core::audio::{AudioBuffer, SampleBuffer};
use tokio::sync::mpsc::UnboundedSender;

use crate::{app::Message, library::Track};

use self::reader::{Fragment, SymphoniaReader};

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
    /// Streams audio to the underlying OS audio library. We set this up on construction and never
    /// change it; instead, we just modify what `source` points to.
    ///
    /// This is wrapped in [`Fragile`] so that other threads can read the player state; we don't
    /// make this publicly readable anywhere.
    _stream: Fragile<Stream>,
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

#[derive(Debug, Default)]
struct PlayQueue {
    index: Option<usize>,
    tracks: Vec<Arc<Track>>,
}

impl Player {
    pub fn new(tx_message: UnboundedSender<Message>) -> Result<Self> {
        let host = cpal::default_host();
        let device =
            host.default_output_device().ok_or_else(|| eyre!("no default output device"))?;
        let source: Arc<Mutex<Option<Source>>> = Arc::new(Mutex::new(None));
        let source_clone = Arc::clone(&source);
        let paused = Arc::new(RwLock::new(true));
        let paused_clone = Arc::clone(&paused);

        let config = device.default_output_config()?.config();
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                match source_clone.lock().unwrap().as_mut() {
                    Some(iter) if !*paused_clone.read().unwrap() => {
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
        Ok(Self {
            source,
            tx_message,
            paused,
            timestamp: None,
            queue: PlayQueue::default(),
            _stream: Fragile::new(stream),
        })
    }

    /// The currently-playing track.
    pub fn current(&self) -> Option<Arc<Track>> {
        self.queue.current()
    }

    pub fn timestamp(&self) -> Option<Duration> {
        self.timestamp
    }

    pub fn set_timestamp(&mut self, timestamp: Option<Duration>) {
        self.timestamp = timestamp;
    }

    /// Sets the play queue to the given playlist. Also stops existing playback.
    pub fn set_play_queue(&mut self, tracks: Vec<Arc<Track>>) {
        self.stop();
        self.queue = PlayQueue::new(tracks);
    }

    pub fn queue_push(&mut self, track: Arc<Track>) {
        self.queue.tracks.push(track);
    }

    /// Sets the current track to the one at the given position. Panics if that's out of bounds.
    pub fn set_queue_index(&mut self, index: Option<usize>) -> Result<()> {
        self.queue.index = index;
        let Some(track) = self.queue.current() else {
            self.stop();
            return Ok(());
        };

        let reader = Arc::new(Mutex::new(SymphoniaReader::from_path(&track.path)?));
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
        *self.source.lock().unwrap() = Some(source);
        Ok(())
    }
}

/// Functions related to playback control.
impl Player {
    pub fn previous(&mut self) -> Result<()> {
        self.set_queue_index(self.queue.index.and_then(|i| i.checked_sub(1)))
    }

    /// Unpauses. If there is no selected track, selects the first track.
    pub fn play(&mut self) -> Result<()> {
        if self.queue.current().is_none() && !self.queue.tracks.is_empty() {
            self.set_queue_index(Some(0))?;
        }
        self.set_paused(false);
        Ok(())
    }

    /// True if audio is being produced (i.e., we're not paused *and* there's a current song).
    pub fn playing(&self) -> bool {
        !self.paused() && self.current().is_some()
    }

    pub fn stopped(&self) -> bool {
        self.queue.index.is_none()
    }

    pub fn paused(&self) -> bool {
        *self.paused.write().unwrap()
    }

    pub fn pause(&mut self) {
        if self.playing() {
            self.set_paused(true);
        }
    }

    fn set_paused(&mut self, paused: bool) {
        *self.paused.write().unwrap() = paused;
    }

    /// Moves to the next track. If this was the last track, equivalent to stop().
    pub fn next(&mut self) -> Result<()> {
        self.set_queue_index(
            self.queue.index.map(|i| i + 1).filter(|i| *i < self.queue.tracks.len()),
        )
    }
    /// Stops playback. This also unsets our position in the play queue.
    pub fn stop(&mut self) {
        self.queue.index = None;
        *self.source.lock().unwrap() = None;
    }

    /// Seek to the given timestamp. Does nothing if there's no currently-playing track.
    pub fn seek(&mut self, target: Duration) -> Result<()> {
        let mut source = self.source.lock().unwrap();
        if let Some(source) = source.as_mut() {
            source.reader.lock().unwrap().seek(target)
        } else {
            Ok(())
        }
    }
}

impl PlayQueue {
    pub fn new(tracks: Vec<Arc<Track>>) -> Self {
        Self { index: None, tracks }
    }

    pub fn current(&self) -> Option<Arc<Track>> {
        self.index.map(|i| Arc::clone(&self.tracks[i]))
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
            let samples = reader_clone.lock().unwrap().next().map(|fragment| {
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
