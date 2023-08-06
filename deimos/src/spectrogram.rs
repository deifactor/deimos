use anyhow::{anyhow, Result};
use itertools::Itertools;
use ratatui::widgets::Sparkline;
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling::divide_by_N_sqrt, windows::hann_window, FrequencyLimit,
    FrequencySpectrum,
};

use crate::ui::Component;

#[derive(Debug, Default)]
pub struct Visualizer {
    /// Spectrum from the most recent sample.
    spectrum: Option<FrequencySpectrum>,
    magnitudes: Option<Vec<f32>>,
}

impl Visualizer {
    pub fn update_spectrum(&mut self, samples: &[f32]) -> Result<()> {
        let mut samples: Vec<f32> = samples.to_vec();
        samples.resize(1024, 0.0);
        let spectrum = samples_fft_to_spectrum(
            &hann_window(&samples),
            44100,
            FrequencyLimit::All,
            Some(&divide_by_N_sqrt),
        )
        .map_err(|e| anyhow!("{:?}", e))?;
        self.spectrum = Some(spectrum);
        Ok(())
    }

    fn merge_magnitudes(&mut self, new_magnitudes: Vec<f32>) -> &Vec<f32> {
        if self
            .magnitudes
            .as_ref()
            .map_or(true, |vec| vec.len() != new_magnitudes.len())
        {
            self.magnitudes = Some(new_magnitudes);
            return &self.magnitudes.as_ref().unwrap();
        }
        let magnitudes = self.magnitudes.as_mut().unwrap();
        for i in 0..magnitudes.len() {
            magnitudes[i] = 0.7 * magnitudes[i] + 0.3 * new_magnitudes[i];
        }
        magnitudes
    }
}

impl Component for Visualizer {
    fn draw(
        &mut self,
        _ui: &crate::ui::Ui,
        frame: &mut ratatui::Frame<crate::ui::DeimosBackend>,
        area: ratatui::layout::Rect,
    ) -> Result<()> {
        let Some(spectrum) = &self.spectrum else { return Ok(()) };

        let width = area.width as i32;
        // experimentally, in my music collection most of the interesting dynamics happen here
        let min_freq = 50.0f32;
        let max_freq = 2000.0f32;
        let step = (max_freq / min_freq).powf(1.0 / (width as f32 - 1.0));
        let frequencies = (0..width).map(|i| min_freq * step.powi(i));

        let new_magnitudes = frequencies
            .map(|freq| spectrum.freq_val_exact(freq))
            .map(|val| (val.val()))
            .collect_vec();

        let u64_magnitudes = self
            .merge_magnitudes(new_magnitudes)
            .iter()
            .map(|x| (x * 64.0) as u64)
            .collect_vec();

        let sparkline = Sparkline::default().data(&u64_magnitudes).max(256);
        frame.render_widget(sparkline, area);
        Ok(())
    }
}
