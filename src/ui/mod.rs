pub(crate) mod album_art;
pub(crate) mod artist_album_list;
pub(crate) mod now_playing;
pub(crate) mod search;
pub(crate) mod spectrogram;
pub(crate) mod track_list;

use std::cmp::Reverse;

use eyre::{Context, Result};
use image::RgbImage;
use itertools::Itertools;
use ordered_float::OrderedFloat;
use palette::{FromColor, Oklab, Oklch, Srgb};
use quantette::{kmeans::Centroids, ColorSpace, QuantizeOutput, UniqueColorCounts};
use ratatui::style::{Color, Modifier, Style};
use tap::Pipe;

use crate::library::Track;

#[derive(Debug, Default)]
pub struct Ui {
    pub theme: Theme,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub focused_border: Style,
    pub unfocused_border: Style,
    pub section_header: Style,
    pub now_playing_track: Style,
}

impl Theme {
    pub fn new(colors: &ColorScheme) -> Self {
        Self {
            focused_border: Style::default().fg(colors.primary_accent),
            unfocused_border: Style::default(),
            section_header: Style::default()
                .bg(colors.secondary_accent)
                .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            now_playing_track: Style::default()
                .fg(colors.primary_accent)
                .add_modifier(Modifier::BOLD),
        }
    }

    pub fn from_track(track: &Track) -> Result<Self> {
        let options = ColorSchemeOptions::default();
        let Some(album_art) = track.album_art()? else {
            return Ok(Self::default());
        };
        let candidates = options
            .candidates(&album_art.into_rgb8())?
            .into_iter()
            .map(|(color, _)| color)
            .collect_vec();
        Ok(Self::new(&ColorScheme::from_candidates(&candidates)))
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(&ColorScheme::default())
    }
}

impl Ui {
    pub fn border(&self, state: ActiveState) -> Style {
        match state {
            ActiveState::Focused => self.theme.focused_border,
            ActiveState::Inactive => self.theme.unfocused_border,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ActiveState {
    Focused,
    Inactive,
}

impl ActiveState {
    pub fn focused_if(cond: bool) -> Self {
        if cond {
            Self::Focused
        } else {
            Self::Inactive
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorSchemeOptions {
    /// The lower this is, the less we take lightness into account during palettization.
    pub lightness_weight: f32,
    /// Number of candidate colors to generate.
    pub candidates: u8,
    pub k_means: bool,
}

impl Default for ColorSchemeOptions {
    fn default() -> Self {
        Self { lightness_weight: 0.40, candidates: 8, k_means: true }
    }
}

#[derive(Debug, Clone)]
pub struct ColorScheme {
    /// Suitable for using as the background of album art.
    pub background: Color,
    // A highlight color.
    pub primary_accent: Color,
    // Another highlight color, ideally one that contrasts with `primary_accent`.
    pub secondary_accent: Color,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            background: Color::Reset,
            primary_accent: Color::LightCyan,
            secondary_accent: Color::Blue,
        }
    }
}

impl ColorScheme {
    pub fn from_candidates(candidates: &[Oklch]) -> Self {
        // All of these are just guesses and stuff. The numbers don't have any deep roots in human
        // color perception, they're just what I thought looked nice.
        let background = candidates
            .iter()
            .find(|color| color.l < 0.1)
            .cloned()
            .unwrap_or(Oklch::new(Oklch::min_l(), 0.0, 0.0));
        let mut primary_accent = candidates
            .iter()
            .filter(|color| color.l > 0.1 && color.chroma > 0.02)
            .max_by_key(|color| OrderedFloat(color.chroma))
            .cloned()
            .unwrap_or(Oklch::new(Oklch::max_l(), 0.0, 0.0));
        let mut secondary_accent = candidates
            .iter()
            .filter(|color| color.l > 0.01 && color.chroma > 0.02)
            .max_by_key(|c| {
                // into_radians().abs() ensures that hue 0 and hue 359 are 'close'
                (c.chroma * 2.0 + (c.hue - primary_accent.hue).into_radians().abs())
                    .pipe(OrderedFloat)
            })
            .cloned()
            .unwrap_or(primary_accent);
        // apply minimum lightness so it looks good
        primary_accent.l = primary_accent.l.max(0.5);
        secondary_accent.l = secondary_accent.l.max(0.5);
        Self {
            background: crossterm_color(background).into(),
            primary_accent: crossterm_color(primary_accent).into(),
            secondary_accent: crossterm_color(secondary_accent).into(),
        }
    }
}

impl ColorSchemeOptions {
    /// Generate a set of candidate colors for using in the color scheme. The result is a list of
    /// (color, ratio) pairs sorted by decreasing frequency.
    pub fn candidates(&self, image: &RgbImage) -> Result<Vec<(Oklch, f32)>> {
        let color_counts = UniqueColorCounts::try_from_rgbimage_par(image, |srgb| {
            let mut oklab = Oklab::from_color(srgb.into_format::<f32>());
            oklab.l *= self.lightness_weight;
            oklab
        })
        .wrap_err("couldn't extract color counts from image")?;
        let quantized = quantette::wu::palette_par(
            &color_counts,
            self.candidates.into(),
            &ColorSpace::default_binner_oklab_f32(),
        );
        if self.k_means {
            let quantized = quantette::kmeans::palette_par(
                &color_counts,
                // needed for images with very few colors
                (color_counts.num_colors() / 2).max(4096),
                4096, // batch_size; arbitrary
                Centroids::from_truncated(quantized.palette.clone()),
                0,
            );
            Ok(self.palette_by_frequency(quantized))
        } else {
            Ok(self.palette_by_frequency(quantized))
        }
    }

    // Sort the colors by output. This also removes the lightness transform.
    fn palette_by_frequency(&self, quantized: QuantizeOutput<Oklab>) -> Vec<(Oklch, f32)> {
        let total_counts: u32 = quantized.counts.iter().sum();
        quantized
            .palette
            .into_iter()
            .map(|mut color| {
                color.l /= self.lightness_weight;
                color
            })
            .zip(quantized.counts)
            .sorted_unstable_by_key(|(_, count)| Reverse(*count))
            .map(|(color, count)| {
                (Oklch::from_color(color), (count as f32) / (total_counts as f32))
            })
            .collect_vec()
    }
}

pub fn crossterm_color<T: Copy>(color: T) -> crossterm::style::Color
where
    Srgb: FromColor<T>,
{
    let srgb = Srgb::from_color(color).into_format::<u8>();
    crossterm::style::Color::Rgb { r: srgb.red, g: srgb.green, b: srgb.blue }
}
