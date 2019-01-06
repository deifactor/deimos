//! Code for storing and parsing catgirl configuration data. See config.md for
//! end-user documentation.

use std::path;
use std::str::FromStr;

/// The parsed version of a `config.toml` file.
pub struct Config {
    /// Formatting information.
    pub format: Format,
}

impl Config {
    fn from_raw(raw: raw::Config) -> Result<Config, failure::Error> {
        Ok(Config {
            format: Format::from_raw(raw.format)?,
        })
    }
}

impl FromStr for Config {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw: raw::Config = toml::from_str(s)?;
        Config::from_raw(raw)
    }
}

/// Configuration data related to formatting
pub struct Format {
    /// Formats the display of songs in playlists/the current queue.
    pub playlist_song: mimi::Formatter,
    /// Formats the 'now playing' display on the bottom of the screen.
    pub now_playing: mimi::Formatter,
}

impl Format {
    fn from_raw(raw: raw::Format) -> Result<Format, failure::Error> {
        Ok(Format {
            playlist_song: raw.playlist_song.parse()?,
            now_playing: raw.now_playing.unwrap_or(raw.playlist_song).parse()?,
        })
    }
}

/// The directory `catgirl` stores its configuration in. Returns `None` if we
/// couldn't figure something out, which happens if you're not running macOS,
/// Windows, or Linux.
pub fn config_dir() -> Option<path::PathBuf> {
    let subdir = if cfg!(target_os = "macos") {
        "zone.synthetic.catgirl"
    } else {
        "catgirl"
    };
    dirs::config_dir().map(|path| [path, subdir.into()].iter().collect())
}

/// The file `catgirl` will load its configuration from. Returns `None` if we
/// couldn't figure something out, which happens if you're not running macOS,
/// Windows, or Linux.
pub fn config_path() -> Option<path::PathBuf> {
    config_dir().map(|dir| [dir, "config.toml".into()].iter().collect())
}

/// A low-level description of the configuration file. `raw::Config` is transformed into `Config`.
mod raw {
    use serde_derive::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub(super) struct Config {
        #[serde(default)]
        pub format: Format,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub(super) struct Format {
        #[serde(default = "default_playlist_song")]
        pub playlist_song: String,
        pub now_playing: Option<String>,
    }

    impl Default for Format {
        // XXX: is there a better way to do this?
        fn default() -> Format {
            toml::from_str("").unwrap()
        }
    }

    fn default_playlist_song() -> String {
        "$title - $artist - $album".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;
    use std::collections::HashMap;

    // Keys to use when testing formatter equality.
    fn test_keys() -> HashMap<String, String> {
        hashmap![
            "artist".into() => "Some Artist".into(),
            "album".into() => "Their Worst Album".into(),
            "title".into() => "The One Good Song".into()
        ]
    }

    #[test]
    fn empty_config_is_valid() {
        "".parse::<Config>().unwrap();
    }

    #[test]
    fn empty_tables_are_valid() {
        r#"
[format]
"#
        .parse::<Config>()
        .unwrap();
    }

    #[test]
    fn now_playing_defaults_to_playlist_song() {
        let config: Config = r#"
[format]
playlist-song = "np: $title by $artist from $album"
"#
        .parse()
        .unwrap();
        assert_eq!(
            config.format.now_playing.ansi(&test_keys()),
            config.format.playlist_song.ansi(&test_keys())
        );
    }

    #[test]
    fn now_playing_can_override_playlist_song() {
        let config: Config = r#"
[format]
playlist-song = "np: $title by $artist from $album"
now-playing = "$album - $artist - $title"
"#
        .parse()
        .unwrap();
        assert_ne!(
            config.format.now_playing.ansi(&test_keys()),
            config.format.playlist_song.ansi(&test_keys()),
        );
    }
}
