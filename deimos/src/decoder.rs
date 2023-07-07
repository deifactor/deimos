// This file heavily based on `rodio/src/decoder/symphonia.rs`.
use anyhow::{bail, Result};
use rodio::Source;
use std::{
    fs::File,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use symphonia::{
    core::{
        audio::{AudioBufferRef, SampleBuffer, SignalSpec},
        codecs::{Decoder, DecoderOptions},
        errors::Error,
        formats::{FormatOptions, FormatReader},
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    },
    default::get_probe,
};

// Decoder errors are not considered fatal.
// The correct action is to just get a new packet and try again.
// But a decode error in more than 3 consecutive packets is fatal.
const MAX_DECODE_ERRORS: usize = 3;

/// Like rodio's built-in `SymphoniaDecoder`, but also updates a field with the elapsed time after yielding each packet.
pub struct TrackingSymphoniaDecoder {
    decoder: Box<dyn Decoder>,
    current_frame_offset: usize,
    format: Box<dyn FormatReader>,
    buffer: SampleBuffer<i16>,
    spec: SignalSpec,
    timestamp: Arc<Mutex<Duration>>,
}

/// Decodes a media source using symphonia and exposes the current
/// timestamp. The timestamp is not guaranteed to be accurate, but will be
/// fairly close.
impl TrackingSymphoniaDecoder {
    fn new(mss: MediaSourceStream, extension: Option<&str>) -> Result<Self> {
        let mut hint = Hint::new();
        if let Some(ext) = extension {
            hint.with_extension(ext);
        }
        let format_opts: FormatOptions = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts: MetadataOptions = Default::default();
        let mut probed = get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

        let stream = match probed.format.default_track() {
            Some(stream) => stream,
            None => bail!("couldn't find a default track"),
        };

        let mut decoder = symphonia::default::get_codecs()
            .make(&stream.codec_params, &DecoderOptions { verify: true })?;

        let mut decode_errors: usize = 0;
        let decoded = loop {
            let current_frame = probed.format.next_packet()?;
            match decoder.decode(&current_frame) {
                Ok(decoded) => break decoded,
                Err(e) => match e {
                    Error::DecodeError(_) => {
                        decode_errors += 1;
                        if decode_errors > MAX_DECODE_ERRORS {
                            bail!(e);
                        } else {
                            continue;
                        }
                    }
                    _ => bail!(e),
                },
            }
        };
        let spec = decoded.spec().to_owned();
        let buffer = TrackingSymphoniaDecoder::get_buffer(decoded, &spec);

        Ok(TrackingSymphoniaDecoder {
            decoder,
            current_frame_offset: 0,
            format: probed.format,
            buffer,
            spec,
            timestamp: Arc::new(Mutex::new(Duration::ZERO)),
        })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        Self::new(mss, path.as_ref().extension().and_then(|ext| ext.to_str()))
    }

    pub fn timestamp(&self) -> Arc<Mutex<Duration>> {
        Arc::clone(&self.timestamp)
    }

    #[inline]
    fn get_buffer(decoded: AudioBufferRef, spec: &SignalSpec) -> SampleBuffer<i16> {
        let duration = decoded.capacity() as u64;
        let mut buffer = SampleBuffer::<i16>::new(duration, *spec);
        buffer.copy_interleaved_ref(decoded);
        buffer
    }
}

impl Source for TrackingSymphoniaDecoder {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.buffer.samples().len())
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.spec.channels.count() as u16
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.spec.rate
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Iterator for TrackingSymphoniaDecoder {
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<i16> {
        if self.current_frame_offset == self.buffer.len() {
            let mut decode_errors: usize = 0;
            let decoded = loop {
                let packet = self.format.next_packet().ok()?;
                let time_base = self.decoder.codec_params().time_base.unwrap();
                // We only update the elapsed time when we get a new packet to
                // avoid *constantly* updating it.
                let timestamp = time_base.calc_time(packet.ts + packet.dur);
                *self.timestamp.lock().unwrap() =
                    Duration::from_secs_f64(timestamp.seconds as f64 + timestamp.frac);
                match self.decoder.decode(&packet) {
                    Ok(decoded) => break decoded,
                    Err(e) => match e {
                        Error::DecodeError(_) => {
                            decode_errors += 1;
                            if decode_errors > MAX_DECODE_ERRORS {
                                return None;
                            } else {
                                continue;
                            }
                        }
                        _ => return None,
                    },
                }
            };
            self.spec = decoded.spec().to_owned();
            self.buffer = TrackingSymphoniaDecoder::get_buffer(decoded, &self.spec);
            self.current_frame_offset = 0;
        }

        let sample = *self.buffer.samples().get(self.current_frame_offset)?;
        self.current_frame_offset += 1;

        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_timestamp() -> Result<()> {
        let mss = MediaSourceStream::new(
            Box::new(Cursor::new(include_bytes!("../test_data/3_seconds.mp3"))),
            Default::default(),
        );
        let decoder = TrackingSymphoniaDecoder::new(mss, Some("mp3"))?;
        let timestamp = decoder.timestamp();
        // drain the iterator to go all the way to the end
        for _ in decoder.into_iter() {}
        assert_eq!(*timestamp.lock().unwrap(), Duration::from_secs(3));
        Ok(())
    }
}
