use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, Stream,
};
use itertools::Itertools;
use symphonia::core::audio::{AudioBuffer, SampleBuffer};
use tokio::sync::mpsc::UnboundedSender;

use crate::{app::Message, library::Track};

use self::reader::{Fragment, SymphoniaReader};

mod reader;

pub struct Player {
    reader: Option<Arc<Mutex<SymphoniaReader>>>,
    tx_message: UnboundedSender<Message>,
    stream: Option<Stream>,

    device: Device,
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
        Ok(Self {
            reader: None,
            tx_message,
            stream: None,
            device,
        })
    }

    pub fn play_track(&mut self, track: Arc<Track>) -> Result<()> {
        let reader = Arc::new(Mutex::new(SymphoniaReader::from_path(&track.path)?));
        self.reader = Some(Arc::clone(&reader));

        let tx_message = self.tx_message.clone();
        let on_decode: DecodeCallback = Box::new(move |fragment| {
            let _ = tx_message.send(Message::Player(PlayerMessage::AudioFragment {
                buffer: fragment.buffer,
                timestamp: fragment.timestamp,
                track: Arc::clone(&track),
            }));
        });
        let on_finish: FinishCallback = Box::new(|| ());

        // get an iterator over the raw samples
        let source = Source::new(reader, on_decode, on_finish);
        let mut sample_iter = source
            .flat_map(|samples| samples.samples().iter().copied().collect_vec())
            .fuse();

        let config = self.device.default_output_config()?.config();
        let stream = self.device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                for (dst, src) in data.iter_mut().zip(sample_iter.by_ref()) {
                    *dst = src
                }
            },
            |e| {
                dbg!(e);
            },
            None,
        )?;
        self.stream = Some(stream);
        Ok(())
    }

    /// Seek to the given timestamp. Does nothing if there's no currently-playing track.
    pub fn seek(&mut self, target: Duration) -> Result<()> {
        let Some(reader) = self.reader.as_mut() else {
            return Ok(());
        };
        let mut reader = reader.lock().unwrap();
        reader.seek(target)
    }
}

type DecodeCallback = Box<dyn FnMut(Fragment) + Send + 'static>;
type FinishCallback = Box<dyn FnOnce() + Send + 'static>;

/// Wraps a [`SymphoniaReader`] in order to invoke callbacks. This takes out a lock on the
/// reader when it needs to decode a new buffer.
struct Source {
    reader: Arc<Mutex<SymphoniaReader>>,
    /// Invoked every time the inner reader decodes another sample buffer.
    on_decode: DecodeCallback,
    /// Invoked at most once when the inner reader has a (non-retryable) decode failure.
    on_finish: Option<FinishCallback>,
}

impl Source {
    fn new(
        reader: Arc<Mutex<SymphoniaReader>>,
        on_decode: DecodeCallback,
        on_finish: FinishCallback,
    ) -> Self {
        Self {
            reader,
            on_decode,
            on_finish: Some(on_finish),
        }
    }
}

impl Iterator for Source {
    type Item = SampleBuffer<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.lock().unwrap().next() {
            Some(fragment) => {
                let buffer = &fragment.buffer;
                let mut samples = SampleBuffer::new(buffer.capacity() as u64, *buffer.spec());
                samples.copy_interleaved_typed(buffer);
                (self.on_decode)(fragment);
                Some(samples)
            }
            None => {
                if let Some(f) = self.on_finish.take() {
                    f()
                }
                None
            }
        }
    }
}
