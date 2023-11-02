use std::{
    iter,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Stream,
};
use itertools::Itertools;
use symphonia::core::audio::{AudioBuffer, SampleBuffer};
use tokio::sync::mpsc::UnboundedSender;

use crate::{app::Message, library::Track};

use self::reader::{Fragment, SymphoniaReader};

mod reader;

pub struct Player {
    /// Provides an iterator over indiviudal samples as well as access to the underlying reader.
    source: Arc<Mutex<Option<Source>>>,
    tx_message: UnboundedSender<Message>,
    /// Streams audio to the underlying OS audio library. We set this up on construction and never
    /// change it; instead, we just modify what `source` points to.
    _stream: Stream,
}

pub enum PlayerMessage {
    AudioFragment {
        buffer: AudioBuffer<f32>,
        timestamp: Duration,
        track: Arc<Track>,
    },
}

impl Player {
    pub fn new(tx_message: UnboundedSender<Message>) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no default output device")?;
        let source: Arc<Mutex<Option<Source>>> = Arc::new(Mutex::new(None));
        let config = device.default_output_config()?.config();
        let source_clone = Arc::clone(&source);
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                source_clone.lock().unwrap().as_mut().map(|iter| {
                    for (dst, src) in data.iter_mut().zip(iter.by_ref()) {
                        *dst = src
                    }
                });
            },
            |e| {
                dbg!(e);
            },
            None,
        )?;
        Ok(Self {
            source,
            tx_message,
            _stream: stream,
        })
    }

    pub fn play_track(&mut self, track: Arc<Track>) -> Result<()> {
        let reader = Arc::new(Mutex::new(SymphoniaReader::from_path(&track.path)?));
        let tx_message = self.tx_message.clone();
        let on_decode: DecodeCallback = Box::new(move |fragment| {
            let _ = tx_message.send(Message::Player(PlayerMessage::AudioFragment {
                buffer: fragment.buffer,
                timestamp: fragment.timestamp,
                track: Arc::clone(&track),
            }));
        });
        let on_finish: FinishCallback = Box::new(|| ());
        let source = Source::new(reader, on_decode, on_finish);
        *self.source.lock().unwrap() = Some(source);
        Ok(())
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

        Self {
            reader,
            iterator: Box::new(iterator),
        }
    }
}

impl Iterator for Source {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.iterator.next()
    }
}
