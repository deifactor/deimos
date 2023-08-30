use anyhow::{Context, Result};
use lofty::{Accessor, ItemKey, TaggedFileExt};
use ordered_float::OrderedFloat;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::{fs::File, path::Path};
use symphonia::core::io::MediaSourceStream;

use walkdir::WalkDir;

/// Stores information about the library as a whole.
#[derive(Debug, Clone, Default)]
pub struct Library {
    pub artists: HashMap<ArtistId, Artist>,
}

// Intentionally *not* `Option<String>` so that we can support "Various Artists" later.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtistId {
    Unknown,
    Artist(String),
}

/// Information about an individual artist. We guarantee that `self.albums[name].artist ==
/// self.name`.
#[derive(Debug, Clone)]
pub struct Artist {
    pub id: ArtistId,
    pub albums: HashMap<AlbumId, Album>,
}

impl Artist {
    pub fn new(id: ArtistId) -> Self {
        Self {
            id,
            albums: HashMap::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord, Hash)]
pub struct AlbumId(pub Option<String>);

/// Information about an album from a single artist. We guarantee that `self.tracks[i].album ==
/// self.name && self.tracks[i].artist == self.artist`.
#[derive(Debug, Clone)]
pub struct Album {
    pub id: AlbumId,
    pub tracks: Vec<Track>,
}

impl Album {
    pub fn new(id: AlbumId) -> Self {
        Self { id, tracks: vec![] }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Track {
    pub number: Option<u32>,
    pub path: PathBuf,
    pub title: Option<String>,
    pub album: AlbumId,
    pub artist: ArtistId,
    pub length: OrderedFloat<f64>,
}

impl Library {
    /// Scan the given path for music, initializing it as we go.
    pub fn scan(path: impl AsRef<Path>) -> Result<Self> {
        let mut library = Self::default();

        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if let Ok(track) = Track::from_path(entry.path()) {
                library.insert_track(track)?;
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
        tracks.push(track);
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
        self.artists()
            .flat_map(|artist| artist.albums.values().map(move |album| (album, artist)))
    }

    pub fn albums(&self) -> impl Iterator<Item = &Album> {
        self.albums_with_artist().map(|(album, _)| album)
    }

    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.albums().flat_map(|album| album.tracks.iter())
    }
}

impl Track {
    pub fn from_path(path: &Path) -> Result<Self> {
        let probe = symphonia::default::get_probe();

        let file = File::open(path)?;
        let media_source = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = probe.format(
            &Default::default(),
            media_source,
            &Default::default(),
            &Default::default(),
        )?;
        let stream = probed
            .format
            .default_track()
            .context("couldn't find a default track")?;

        let tagged_file = lofty::read_from_path(path)?;
        let tag = tagged_file.primary_tag().context("no tags found")?;
        let artist = tag
            .get_string(&ItemKey::AlbumArtist)
            .or(tag.get_string(&ItemKey::TrackArtist));
        let time_base = stream.codec_params.time_base.unwrap();
        let duration = time_base.calc_time(stream.codec_params.n_frames.unwrap());
        let duration = duration.seconds as f64 + duration.frac;

        Ok(Self {
            number: tag.track(),
            path: path.to_owned(),
            title: tag.title().map(|s| s.into_owned()),
            album: tag.album().map(|s| s.into_owned()).into(),
            artist: artist.map(|s| s.to_owned()).into(),
            length: duration.into(),
        })
    }
}

// miscellaneous impls

impl Display for ArtistId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtistId::Unknown => "<unknown>".fmt(f),
            ArtistId::Artist(name) => name.fmt(f),
        }
    }
}

impl From<Option<String>> for ArtistId {
    fn from(value: Option<String>) -> Self {
        value.map_or(ArtistId::Unknown, ArtistId::Artist)
    }
}

impl Display for AlbumId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.as_deref().unwrap_or("<unknown>").fmt(f)
    }
}

impl From<Option<String>> for AlbumId {
    fn from(value: Option<String>) -> Self {
        AlbumId(value)
    }
}
