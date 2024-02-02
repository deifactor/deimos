use eyre::{eyre, Result};
use image::DynamicImage;
use itertools::Itertools;
use lofty::{Accessor, ItemKey, TaggedFileExt};
use mpris_server::TrackId;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs::File, path::Path};
use symphonia::core::io::MediaSourceStream;

use walkdir::WalkDir;

/// Stores information about the library as a whole.
#[derive(Debug, Clone, Default)]
pub struct Library {
    pub artists: HashMap<ArtistName, Artist>,
}

// Intentionally *not* `Option<String>` so that we can support "Various Artists" later.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum ArtistName {
    Unknown,
    Artist(String),
}

/// Information about an individual artist. We guarantee that `self.albums[name].artist ==
/// self.name`.
#[derive(Debug, Clone)]
pub struct Artist {
    pub name: ArtistName,
    pub albums: HashMap<AlbumName, Album>,
}

impl Artist {
    pub fn new(name: ArtistName) -> Self {
        Self { name, albums: HashMap::new() }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct AlbumName(pub Option<String>);

/// Information about an album from a single artist. We guarantee that `self.tracks[i].album ==
/// self.name && self.tracks[i].artist == self.artist`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Album {
    pub name: AlbumName,
    pub tracks: Vec<Arc<Track>>,
}

impl Album {
    pub fn new(name: AlbumName) -> Self {
        Self { name, tracks: vec![] }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct Track {
    /// Arbitrary numeric ID used for MPRIS purposes.
    pub id: u64,
    pub number: Option<u32>,
    pub path: PathBuf,
    pub title: Option<String>,
    pub album: AlbumName,
    pub artist: ArtistName,
    pub length: OrderedFloat<f64>,
}

impl Track {
    pub fn mpris_id(&self) -> TrackId {
        format!("/{}", self.id).try_into().expect("failed to convert track id to dbus object")
    }

    /// Looks for album art. This loads the image off disk. Returns `Ok(Some(img))` on success,
    /// Ok(None)` if the image just doesn't have any album art, and `Err(e)` if something went
    /// wrong.
    pub fn album_art(&self) -> Result<Option<DynamicImage>> {
        let tagged = lofty::read_from_path(&self.path)?;
        // TODO: look at PictureType? check my collection to see if this is even used.
        tagged
            .primary_tag()
            .and_then(|tag| tag.pictures().first())
            .map(|img| image::load_from_memory(img.data()))
            .transpose()
            .map_err(Into::into)
    }

    #[cfg(test)]
    /// A test track with all of the fields set. `test_track(i) ==
    /// test_track(j)` iff `i == j`, but don't rely on any specific properties.
    pub fn test_track(id: u64) -> Track {
        Track {
            id,
            number: Some(id as u32),
            path: PathBuf::from(format!("/{id}.mp3")),
            title: Some(format!("Test track {id}")),
            album: AlbumName(Some("Test album".into())),
            artist: ArtistName::Artist("Test artist".into()),
            length: OrderedFloat(200.0),
        }
    }
}

impl Library {
    /// Loads the library from disk. A library is serialized as a list of tracks.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let tracks: Vec<Track> = serde_json::from_slice(fs::read(path)?.as_slice())?;
        let mut library = Self::default();
        for track in tracks {
            library.insert_track(track)?;
        }
        Ok(library)
    }

    /// Serializes the library to disk. A library is serialized as a list of tracks.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let tracks = self.tracks().collect_vec();
        fs::write(path, serde_json::to_vec(&tracks)?.as_slice())?;
        Ok(())
    }

    /// Scan the given path for music, initializing it as we go.
    pub fn scan(path: impl AsRef<Path>) -> Result<Self> {
        let mut library = Self::default();
        let mut id = 0;

        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if let Ok(track) = Track::from_path(entry.path(), id) {
                library.insert_track(track)?;
                id += 1;
            }
        }
        Ok(library)
    }

    fn insert_track(&mut self, track: Track) -> Result<()> {
        let tracks = &mut self
            .artists
            .entry(track.artist.clone())
            .or_insert_with_key(|id| Artist::new(id.clone()))
            .albums
            .entry(track.album.clone())
            .or_insert_with_key(|id| Album::new(id.clone()))
            .tracks;
        tracks.push(Arc::new(track));
        tracks.sort_by_key(|track| track.number);
        Ok(())
    }
}

/// Handy iterators.
impl Library {
    pub fn artists(&self) -> impl Iterator<Item = &Artist> {
        self.artists.values()
    }

    pub fn albums_with_artist(&self) -> impl Iterator<Item = (&Album, &Artist)> {
        self.artists().flat_map(|artist| artist.albums.values().map(move |album| (album, artist)))
    }

    pub fn albums(&self) -> impl Iterator<Item = &Album> {
        self.albums_with_artist().map(|(album, _)| album)
    }

    pub fn tracks(&self) -> impl Iterator<Item = Arc<Track>> + '_ {
        self.albums().flat_map(|album| album.tracks.iter()).cloned()
    }
}

impl Track {
    pub fn from_path(path: &Path, id: u64) -> Result<Self> {
        let probe = symphonia::default::get_probe();

        let file = File::open(path)?;
        let media_source = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = probe.format(
            &Default::default(),
            media_source,
            &Default::default(),
            &Default::default(),
        )?;
        let stream =
            probed.format.default_track().ok_or_else(|| eyre!("couldn't find a default track"))?;

        let tagged_file = lofty::read_from_path(path)?;
        let tag = tagged_file.primary_tag().ok_or_else(|| eyre!("no tags found"))?;
        let artist =
            tag.get_string(&ItemKey::AlbumArtist).or(tag.get_string(&ItemKey::TrackArtist));
        let time_base = stream.codec_params.time_base.unwrap();
        let duration = time_base.calc_time(stream.codec_params.n_frames.unwrap());
        let duration = duration.seconds as f64 + duration.frac;

        Ok(Self {
            id,
            number: tag.track(),
            path: path.to_owned(),
            title: tag.title().map(normalize),
            album: tag.album().map(normalize).into(),
            artist: artist.map(normalize).into(),
            length: duration.into(),
        })
    }
}

// miscellaneous impls

impl Display for ArtistName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtistName::Unknown => "<unknown>".fmt(f),
            ArtistName::Artist(name) => name.fmt(f),
        }
    }
}

impl From<Option<String>> for ArtistName {
    fn from(value: Option<String>) -> Self {
        value.map_or(ArtistName::Unknown, ArtistName::Artist)
    }
}

impl Display for AlbumName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.as_deref().unwrap_or("<unknown>").fmt(f)
    }
}

impl From<Option<String>> for AlbumName {
    fn from(value: Option<String>) -> Self {
        AlbumName(value)
    }
}

/// String normalization, Removes characters nucleo doesn't handle.
fn normalize(s: impl AsRef<str>) -> String {
    // not the most efficient, but this only runs on library load so it's fine
    let normalize_map = HashMap::from([
        ('\u{2018}', '\''),
        ('\u{2019}', '\''),
        ('\u{201c}', '"'),
        ('\u{201d}', '"'),
    ]);
    s.as_ref().chars().map(|c| normalize_map.get(&c).copied().unwrap_or(c)).collect()
}

#[cfg(test)]
mod tests {
    use crate::test_data;

    use super::*;
    #[test]
    fn equal_test_track_ids_are_equal() {
        assert_eq!(Track::test_track(0), Track::test_track(0));
        assert_eq!(Track::test_track(1), Track::test_track(1));
    }

    #[test]
    fn unequal_test_track_ids_are_unequal() {
        assert_ne!(Track::test_track(0), Track::test_track(1));
    }

    #[test]
    fn no_album_art() -> Result<()> {
        let track = Track::from_path(&test_data!("3_seconds.mp3"), 0)?;
        assert_eq!(track.album_art()?, None);
        Ok(())
    }
}
