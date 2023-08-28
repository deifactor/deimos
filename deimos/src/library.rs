use anyhow::{bail, Context, Result};
use lofty::{Accessor, ItemKey, TaggedFileExt};
use ordered_float::OrderedFloat;
use sqlx::Connection;
use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::{fs::File, os::unix::prelude::OsStrExt, path::Path};
use symphonia::core::formats::Track as SymphoniaTrack;
use symphonia::core::io::MediaSourceStream;

use walkdir::WalkDir;

/// Stores information about the library as a whole.
#[allow(dead_code)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct AlbumId(pub Option<String>);

/// Information about an album from a single artist. We guarantee that `self.tracks[i].album ==
/// self.name && self.tracks[i].artist == self.artist`.
#[derive(Debug, Clone)]
pub struct Album {
    pub id: AlbumId,
    pub artist: ArtistId,
    pub tracks: Vec<Track>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Track {
    pub song_id: i64,
    pub number: Option<i64>,
    pub path: String,
    pub title: Option<String>,
    pub album: AlbumId,
    pub artist: ArtistId,
    pub length: OrderedFloat<f64>,
}

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

impl From<Option<String>> for AlbumId {
    fn from(value: Option<String>) -> Self {
        AlbumId(value)
    }
}

impl Display for AlbumId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.as_deref().unwrap_or("<unknown>").fmt(f)
    }
}

/// Initialize the song database, creating all tables. This deletes any existing database.
pub async fn initialize_db(path: impl AsRef<Path>) -> Result<Pool<Sqlite>> {
    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true),
    )
    .await?;
    let mut conn = pool.acquire().await?;
    sqlx::migrate!().run(&mut conn).await?;
    Ok(pool)
}

async fn find_music(path: impl AsRef<Path>, conn: &mut Transaction<'_, Sqlite>) -> Result<()> {
    let probe = symphonia::default::get_probe();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file = File::open(entry.path())?;
        let media_source = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = probe.format(
            &Default::default(),
            media_source,
            &Default::default(),
            &Default::default(),
        );
        let Ok(probed) = probed else { continue; };
        let stream = match probed.format.default_track() {
            Some(stream) => stream,
            None => bail!("couldn't find a default track"),
        };
        let _ = insert_song(entry.path(), stream, conn).await;
    }
    Ok(())
}

async fn insert_song(
    path: &Path,
    stream: &SymphoniaTrack,
    conn: &mut Transaction<'_, Sqlite>,
) -> Result<()> {
    let tagged_file = lofty::read_from_path(path)?;
    let tag = tagged_file.primary_tag().context("no tags found")?.clone();
    let path_bytes = path.as_os_str().as_bytes();
    let track = tag.track();
    let title = tag.title();
    let album = tag.album();
    let artist = tag
        .get_string(&ItemKey::AlbumArtist)
        .or(tag.get_string(&ItemKey::TrackArtist));
    let time_base = stream.codec_params.time_base.unwrap();
    let duration = time_base.calc_time(stream.codec_params.n_frames.unwrap());
    let duration = duration.seconds as f64 + duration.frac;
    sqlx::query!(
        "INSERT INTO songs (path, number, title, album, artist, length) VALUES (?, ?, ?, ?, ?, ?)",
        path_bytes,
        track,
        title,
        album,
        artist,
        duration
    )
    .execute(&mut **conn)
    .await?;
    Ok(())
}

/// Performs the initial library load. Returns a map from artist names to their albums.
pub async fn load_library(
    pool: &Pool<Sqlite>,
) -> Result<HashMap<String, Vec<String>>, anyhow::Error> {
    let mut conn = pool.acquire().await?;
    let count = sqlx::query!("SELECT COUNT(*) AS count FROM songs")
        .fetch_one(&mut *conn)
        .await?
        .count;
    if count == 0 {
        conn.transaction(|conn| {
            Box::pin(async move { find_music("/home/vector/music", conn).await })
        })
        .await?;
    }
    let mut artists: HashMap<String, Vec<String>> = HashMap::new();
    sqlx::query!(
        r#"SELECT DISTINCT artist AS "artist!", album AS "album!"
                       FROM songs WHERE artist IS NOT NULL AND album IS NOT NULL
                       ORDER BY artist, album"#
    )
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .for_each(|row| artists.entry(row.artist).or_default().push(row.album));
    Ok(artists)
}
