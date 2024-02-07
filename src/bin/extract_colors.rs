use std::path::PathBuf;

use crossterm::style::{PrintStyledContent, Stylize};
use crossterm::ExecutableCommand;
use deimos::library::Track;
use deimos::ui::{crossterm_color, ColorScheme, ColorSchemeOptions};
use eyre::Result;
use itertools::Itertools;
use palette::Oklch;

fn main() -> Result<()> {
    for path in std::env::args().skip(1).map(PathBuf::from) {
        let track = Track::from_path(&path, 0)?;
        let Some(album_art) = track.album_art()? else {
            continue;
        };
        let album_art = album_art.into_rgb8();
        let wu = ColorSchemeOptions { lightness_weight: 0.50, candidates: 6, k_means: false }
            .candidates(&album_art)?;
        println!("Wu candidates");
        display_candidates(&wu)?;

        let candidates =
            ColorSchemeOptions { lightness_weight: 0.50, candidates: 6, k_means: true }
                .candidates(&album_art)?;
        println!("k-means");
        display_candidates(&candidates)?;

        println!("generated scheme");
        let scheme = ColorScheme::from_candidates(
            &candidates.into_iter().map(|(color, _)| color).collect_vec(),
        );
        display_scheme(scheme)?;
    }
    Ok(())
}

fn display_candidates(candidates: &[(Oklch, f32)]) -> Result<()> {
    let mut stdout = std::io::stdout();
    for (color, ratio) in candidates {
        stdout.execute(PrintStyledContent(
            format!("  {:0.3}  ", ratio).with(crossterm_color(*color)),
        ))?;
    }
    println!();
    for (color, ratio) in candidates {
        stdout.execute(PrintStyledContent(
            format!("  {:0.3}  ", ratio).on(crossterm_color(*color)),
        ))?;
    }
    println!();
    println!();
    Ok(())
}

fn display_scheme(scheme: ColorScheme) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout
        .execute(PrintStyledContent("background".white().on(crossterm_color(scheme.background))))?;
    stdout.execute(PrintStyledContent(" primary".with(crossterm_color(scheme.primary_accent))))?;
    stdout
        .execute(PrintStyledContent(" secondary".with(crossterm_color(scheme.secondary_accent))))?;
    Ok(())
}
