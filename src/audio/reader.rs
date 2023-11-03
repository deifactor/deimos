use std::{fs::File, path::Path, time::Duration};

use anyhow::{bail, Result};
use symphonia::{
    core::{
        audio::AudioBuffer,
        codecs::{Decoder, DecoderOptions},
        formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
        units::Time,
    },
    default::get_probe,
};

/// Reads out samples from a file using Symphonia, providing an iterator over
pub struct SymphoniaReader {
    decoder: Box<dyn Decoder>,
    format: Box<dyn FormatReader>,
}

/// A decoded audio buffer with some extra context information.
pub struct Fragment {
    pub buffer: AudioBuffer<f32>,
    /// Timestamp of this fragment within the song.
    pub timestamp: Duration,
}

/// Give up after this many consecutive decode errors.
const MAX_DECODE_ERRORS: usize = 3;

impl SymphoniaReader {
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
        let probed = get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

        let stream = match probed.format.default_track() {
            Some(stream) => stream,
            None => bail!("couldn't find a default track"),
        };

        let decoder = symphonia::default::get_codecs()
            .make(&stream.codec_params, &DecoderOptions { verify: true })?;

        Ok(Self {
            decoder,
            format: probed.format,
        })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        Self::new(mss, path.as_ref().extension().and_then(|ext| ext.to_str()))
    }

    /// Try to decode a single packet. Semantics are the same as `next`.
    fn try_decode(&mut self) -> Result<Fragment> {
        let packet = self.format.next_packet()?;

        // compute timestamp
        let time_base = self.decoder.codec_params().time_base.unwrap();
        let timestamp = time_base.calc_time(packet.ts + packet.dur);
        let timestamp = Duration::from_secs_f64(timestamp.seconds as f64 + timestamp.frac);

        let decoded = self.decoder.decode(&packet)?;
        let mut buffer = decoded.make_equivalent::<f32>();
        decoded.convert(&mut buffer);

        Ok(Fragment { buffer, timestamp })
    }

    pub(super) fn seek(&mut self, target: Duration) -> Result<()> {
        let target = Time::new(target.as_secs(), target.as_secs_f64().fract());
        self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: target,
                track_id: None,
            },
        )?;
        self.decoder.reset();
        Ok(())
    }
}

impl Iterator for SymphoniaReader {
    type Item = Fragment;

    /// Decode the buffer out of the next packet. Returns None if there was a decode error and on EOF.
    fn next(&mut self) -> Option<Fragment> {
        for _ in 0..MAX_DECODE_ERRORS {
            match self.try_decode() {
                Ok(out) => return Some(out),
                Err(_) => continue,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_timestamp() {
        let mss = MediaSourceStream::new(
            Box::new(Cursor::new(include_bytes!("../../test_data/3_seconds.mp3"))),
            Default::default(),
        );
        let reader = SymphoniaReader::new(mss, Some("mp3")).unwrap();
        let last = reader.last().unwrap();
        assert_eq!(last.timestamp, Duration::from_secs(3));
    }
}
