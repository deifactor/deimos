use anyhow::Result;
use std::sync::{Arc, Mutex};

use rodio::{cpal::FromSample, OutputStreamHandle, Sample, Sink, Source};

/// Controls playback and emits events related to playback. These APIs are non-blocking.
#[derive(Clone)]
pub struct Player {
    sink: Arc<Mutex<Sink>>,
}

impl Player {
    pub fn new(handle: OutputStreamHandle) -> Result<Self> {
        let sink = Arc::new(Mutex::new(Sink::try_new(&handle)?));
        Ok(Self { sink })
    }

    pub fn pause(&self) {
        self.sink.lock().unwrap().pause()
    }

    pub fn play(&self) {
        self.sink.lock().unwrap().play()
    }

    pub fn append<S>(&self, source: S)
    where
        S: Source + Send + 'static,
        f32: FromSample<S::Item>,
        S::Item: Sample + Send,
    {
        self.sink.lock().unwrap().append(source)
    }
}
