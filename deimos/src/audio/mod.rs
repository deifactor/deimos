use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Sample, Stream,
};
use symphonia::core::audio::{AudioBuffer, SampleBuffer};
use tokio::sync::mpsc::UnboundedSender;

use crate::{app::Message, library::Track};

use self::reader::{Fragment, SymphoniaReader};

mod reader;

pub struct Player {
    source: Arc<Mutex<Option<Source>>>,
    tx_message: UnboundedSender<Message>,
    #[allow(dead_code)]
    stream: Stream,
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
        let config = device.default_output_config()?.config();
        let source = Arc::new(Mutex::new(None as Option<Source>));
        let stream_source = Arc::clone(&source);
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                if let Some(source) = stream_source.lock().unwrap().as_mut() {
                    for (dst, src) in data.iter_mut().zip(source) {
                        *dst = src;
                    }
                } else {
                    data.fill(0.);
                }
            },
            move |e| {
                dbg!(e);
            },
            None,
        )?;

        Ok(Self {
            source,
            tx_message,
            stream,
        })
    }

    pub fn play_track(&mut self, track: Arc<Track>) -> Result<()> {
        let mut source = self.source.lock().unwrap();
        let tx_message = self.tx_message.clone();
        let path = track.path.clone();
        let on_decode: DecodeCallback = Box::new(move |fragment| {
            let _ = tx_message.send(Message::Player(PlayerMessage::AudioFragment {
                buffer: fragment.buffer,
                timestamp: fragment.timestamp,
                track: Arc::clone(&track),
            }));
        });
        let on_finish: FinishCallback = Box::new(|| ());
        *source = Some(Source::new(
            SymphoniaReader::from_path(path)?,
            on_decode,
            on_finish,
        ));
        Ok(())
    }
}

type DecodeCallback = Box<dyn FnMut(Fragment) + Send + 'static>;
type FinishCallback = Box<dyn FnOnce() + Send + 'static>;

/// Yields the individual samples from the [`SymphoniaReader`]. The iterator implementation never
/// finishes; instead, after it's done, it continually yields silence.
struct Source {
    pub reader: SymphoniaReader,
    /// Outside of the constructor, `None` means that there are no samples left.
    buffer: Option<SampleBuffer<f32>>,
    on_decode: DecodeCallback,
    on_finish: Option<FinishCallback>,
    offset: usize,
}

impl Source {
    fn new(reader: SymphoniaReader, on_decode: DecodeCallback, on_finish: FinishCallback) -> Self {
        let mut source = Self {
            reader,
            buffer: None,
            on_decode,
            on_finish: Some(on_finish),
            offset: 0,
        };
        source.decode();
        source
    }

    fn decode(&mut self) {
        self.offset = 0;

        match self.reader.next() {
            Some(fragment) => {
                let mut sample_buffer =
                    SampleBuffer::new(fragment.buffer.capacity() as u64, *fragment.buffer.spec());
                sample_buffer.copy_interleaved_typed(&fragment.buffer);
                self.buffer = Some(sample_buffer);
                (self.on_decode)(fragment);
            }
            None => {
                self.buffer = None;
                if let Some(f) = self.on_finish.take() {
                    f()
                }
            }
        }
    }

    /// True if we need to call decode() before we can try to get a sample.
    fn needs_decode(&self) -> bool {
        match self.buffer.as_ref() {
            Some(buffer) => self.offset > buffer.len() - 1,
            None => true,
        }
    }
}

impl Iterator for Source {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.needs_decode() {
            self.decode();
        }
        let Some(buffer) = self.buffer.as_ref() else {
            return Some(f32::EQUILIBRIUM);
        };
        let val = buffer.samples()[self.offset];
        self.offset += 1;
        Some(val)
    }
}
