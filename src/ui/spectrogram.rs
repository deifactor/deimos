use anyhow::{anyhow, Result};
use itertools::Itertools;
use ratatui::widgets::Sparkline;
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling::divide_by_N_sqrt, windows::hann_window, FrequencyLimit,
    FrequencySpectrum,
};

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
        Self {
            window_length: 4096,
            decay: 0.2,
            min_freq: 100.0,
            max_freq: 3000.0,
        }
    }
}

#[derive(Debug)]
pub struct Visualizer {
    options: VisualizerOptions,
    /// Buffer of the most recent `options.window_length` samples. Always has length `options.window_length`.
    buffer: Vec<f32>,
    /// The FFT of `buffer`, padded if necessary.
    spectrum: FrequencySpectrum,
    amplitudes: Option<Vec<f32>>,
}

impl Default for Visualizer {
    fn default() -> Self {
        Self::new(VisualizerOptions::default()).unwrap()
    }
}

impl Visualizer {
    pub fn new(options: VisualizerOptions) -> Result<Self> {
        let buffer = vec![0.0; options.window_length];
        let spectrum = samples_fft_to_spectrum(
            &hann_window(&buffer),
            44100,
            FrequencyLimit::All,
            Some(&divide_by_N_sqrt),
        )
        .map_err(|e| anyhow!("{:?}", e))?;
        Ok(Self {
            buffer: vec![0.0; options.window_length],
            options,
            spectrum,
            amplitudes: None,
        })
    }

    /// Recompute `self.spectrum` from the given samples.
    pub fn update_spectrum(&mut self, buffer: AudioBuffer<f32>) -> Result<()> {
        if buffer.spec().channels.count() == 1 {
            self.buffer.extend(buffer.chan(0));
        } else {
            // downmix to mono if it's 2-channel or more
            self.buffer.extend(
                buffer
                    .chan(0)
                    .iter()
                    .zip(buffer.chan(1))
                    .map(|(a, b)| (a + b) / 2.0),
            )
        }

        if self.buffer.len() < self.options.window_length {
            return Ok(());
        }
        // if we have enough samples, take the last window_length and FFT
        self.buffer = self
            .buffer
            .split_off(self.buffer.len() - self.options.window_length);

        self.spectrum = samples_fft_to_spectrum(
            &hann_window(&self.buffer),
            44100,
            FrequencyLimit::All,
            Some(&divide_by_N_sqrt),
        )
        .map_err(|e| anyhow!("{:?}", e))?;
        Ok(())
    }

    /// Updates `self.amplitudes` using `new_amplitudes`. If they're different
    /// sizes, just uses `new_amplitudes`, or else lerps between them using the
    /// decay value. After calling this, `self.amplitudes` is always `Some`.
    fn merge_amplitudes(&mut self, new_amplitudes: Vec<f32>) {
        if self
            .amplitudes
            .as_ref()
            .map_or(true, |vec| vec.len() != new_amplitudes.len())
        {
            self.amplitudes = Some(new_amplitudes);
        } else {
            let amplitudes = self.amplitudes.as_mut().unwrap();
            for i in 0..amplitudes.len() {
                amplitudes[i] = (1.0 - self.options.decay) * amplitudes[i]
                    + self.options.decay * new_amplitudes[i];
            }
        }
    }

    /// Picks the `n` frequencies to display the spectrogram at.
    fn frequencies(&self, n: usize) -> impl Iterator<Item = f32> {
        let step = (self.options.max_freq / self.options.min_freq).powf(1.0 / (n as f32 - 1.0));
        let min_freq = self.options.min_freq;
        (0..n).map(move |i| min_freq * step.powi(i as i32))
    }

    pub fn draw(
        &mut self,
        _ui: &crate::ui::Ui,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
    ) -> Result<()> {
        let width = area.width as i32;
        if width < 2 {
            return Ok(());
        }

        let new_amplitudes = self
            .frequencies(width as usize)
            .map(|freq| {
                self.spectrum.freq_val_exact(freq).val() * (freq / 400.0).powf(2.0).min(1.0)
            })
            .collect_vec();
        self.merge_amplitudes(new_amplitudes);

        let u64_amplitudes = self
            .amplitudes
            .as_ref()
            .unwrap()
            .iter()
            .map(|x| (x * 64.0) as u64)
            .collect_vec();

        let sparkline = Sparkline::default().data(&u64_amplitudes).max(64);
        frame.render_widget(sparkline, area);
        Ok(())
    }
}
