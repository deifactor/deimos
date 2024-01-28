use std::f32::consts::PI;

use eyre::{anyhow, eyre, Result};
use itertools::Itertools;
use ratatui::widgets::Sparkline;
use spectrum_analyzer::{samples_fft_to_spectrum, Frequency, FrequencyLimit, FrequencyValue};

use symphonia::core::audio::{AudioBuffer, Signal};

#[derive(Debug, Clone)]
pub struct VisualizerOptions {
    /// Number of samples to perform the FFT on. Must be a power of two. Keep
    /// in mind that audio is 44100Hz, so 2048, 4096, or 8192 are recommended.
    pub window_length: usize,
    /// Controls blending between spectrum samples. 1.0 means always use the
    /// new one, 0.5 means average the new and the old. Must be in `(0.0, 1.0]`.
    pub decay: f32,
    /// Minimum frequency to display, in Hz.
    pub min_freq: f32,
    /// Maximum frequency to display, in Hz.
    pub max_freq: f32,
}

impl Default for VisualizerOptions {
    fn default() -> Self {
        Self { window_length: 4096, decay: 0.2, min_freq: 100.0, max_freq: 3000.0 }
    }
}

#[derive(Debug)]
pub struct Visualizer {
    options: VisualizerOptions,
    /// Buffer of the most recent `options.window_length` samples. Always has length
    /// `options.window_length`. We continually append to the end of this and then trim off the
    /// front.
    buffer: Vec<f32>,
    /// The FFT of `buffer`, padded if necessary.
    spectrum: Vec<(Frequency, FrequencyValue)>,
    amplitudes: Option<Vec<f32>>,
    /// Precomputed coefficients for Hann windowing. Same length as `self.buffer`.
    hann_coefficients: Vec<f32>,
}

impl Default for Visualizer {
    fn default() -> Self {
        Self::new(VisualizerOptions::default()).unwrap()
    }
}

impl Visualizer {
    pub fn new(options: VisualizerOptions) -> Result<Self> {
        let buffer = vec![0.0; options.window_length];
        // no scaling necessary for zeroes
        let spectrum = samples_fft_to_spectrum(&buffer, 44100, FrequencyLimit::All, None)
            .map_err(|e| anyhow!("{:?}", e))?;
        let len = options.window_length as f32;
        let hann_coefficients = (0..options.window_length)
            .map(|i| {
                let x = (2.0 * PI * (i as f32) / len).cos();
                0.5 * (1.0 - x)
            })
            .collect_vec();
        Ok(Self {
            buffer: vec![0.0; options.window_length],
            options,
            spectrum: spectrum.data().to_vec(),
            amplitudes: None,
            hann_coefficients,
        })
    }

    /// Resets the visualizer's state as if freshly-created.
    pub fn reset(&mut self) -> Result<()> {
        self.buffer.fill(0.0);
        // no scaling necessary for zeroes
        self.spectrum = samples_fft_to_spectrum(&self.buffer, 44100, FrequencyLimit::All, None)
            .map_err(|e| eyre!("couldn't FFT: {:?}", e))?
            .data()
            .to_vec();
        self.amplitudes = None;
        Ok(())
    }

    /// Appends the buffer to the internal buffer. Then recomputes the spectrum accordingly.
    pub fn update_spectrum(&mut self, buffer: AudioBuffer<f32>) -> Result<()> {
        if buffer.spec().channels.count() == 1 {
            self.buffer.extend(buffer.chan(0));
        } else {
            // downmix to mono if it's 2-channel or more
            self.buffer
                .extend(buffer.chan(0).iter().zip(buffer.chan(1)).map(|(a, b)| (a + b) / 2.0))
        }

        if self.buffer.len() < self.options.window_length {
            return Ok(());
        }
        // if we have enough samples, take the last window_length and FFT
        self.buffer = self.buffer.split_off(self.buffer.len() - self.options.window_length);

        // using the scaling function argument computes statistics twice (since the scaling function
        // can use the statistics).
        let samples = self.window_and_scale(&self.buffer);
        let new_spectrum = samples_fft_to_spectrum(&samples, 44100, FrequencyLimit::All, None)
            .map_err(|e| anyhow!("{:?}", e))?
            .data()
            .to_vec();
        // Merge the old spectrum and the new spectrum.
        for (old, new) in self.spectrum.iter_mut().zip_eq(new_spectrum.iter()) {
            old.1 = FrequencyValue::from(1.0 - self.options.decay) * old.1
                + FrequencyValue::from(self.options.decay) * new.1;
        }
        Ok(())
    }

    /// Picks the `n` frequencies to display the spectrogram at.
    fn frequencies(&self, n: usize) -> impl Iterator<Item = f32> {
        let step = (self.options.max_freq / self.options.min_freq).powf(1.0 / (n as f32 - 1.0));
        let min_freq = self.options.min_freq;
        (0..n).map(move |i| min_freq * step.powi(i as i32))
    }

    /// Get the amplitude of the spectrum at the given point.
    ///
    /// If it's exactly in the spectrum list, we return that. Otherwise we lerp between the two
    /// adjacent values.
    fn amplitude(&self, frequency: f32) -> f32 {
        let frequency: Frequency = frequency.into();
        let index = self.spectrum.binary_search_by_key(&frequency, |(freq, _)| *freq);
        let amplitude = match index {
            Ok(index) => self.spectrum[index].1,
            Err(index) => {
                let prev = self.spectrum[index.checked_sub(1).unwrap()];
                let next = self.spectrum[index];
                prev.1 + (next.1 - prev.1) * (frequency - prev.0) / (next.0 - prev.0)
            }
        };
        amplitude.val()
    }

    pub fn draw(
        &mut self,
        _ui: &crate::ui::Ui,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
    ) -> Result<()> {
        let width = area.width as usize;
        if width < 2 {
            return Ok(());
        }

        let u64_amplitudes = self
            .frequencies(width)
            .map(|freq| self.amplitude(freq) * (freq / 400.0).powf(2.0).min(1.0))
            // rescale
            .map(|x| (x * 64.0) as u64)
            .collect_vec();

        let sparkline = Sparkline::default().data(&u64_amplitudes).max(64);
        frame.render_widget(sparkline, area);
        Ok(())
    }

    /// Applies (Hann) windowing to samples and scales by sqrt(N).
    fn window_and_scale(&self, samples: &[f32]) -> Vec<f32> {
        let sqrt_n = (samples.len() as f32).sqrt();
        samples
            .iter()
            .zip(self.hann_coefficients.iter())
            .map(|(sample, coeff)| sample * coeff / sqrt_n)
            .collect_vec()
    }
}
