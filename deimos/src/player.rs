use anyhow::Result;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use rodio::{cpal::FromSample, OutputStreamHandle, Sample, Sink, Source};

/// Controls playback and emits events related to playback. These APIs are non-blocking.
#[derive(Clone)]
pub struct Player {
    sink: Arc<Mutex<Sink>>,
    elapsed: Arc<Mutex<Duration>>,
}

impl Player {
    pub fn new(handle: OutputStreamHandle) -> Result<Self> {
        let sink = Arc::new(Mutex::new(Sink::try_new(&handle)?));
        let elapsed = Arc::new(Mutex::new(Duration::ZERO));
        Ok(Self { sink, elapsed })
    }

    pub fn clear(&self) {
        self.sink.lock().unwrap().clear()
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
