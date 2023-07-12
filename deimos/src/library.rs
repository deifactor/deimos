use anyhow::{bail, Context, Result};
use lofty::{Accessor, TaggedFileExt};
use ordered_float::OrderedFloat;
use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite, SqlitePool, Transaction};
use std::{fs::File, os::unix::prelude::OsStrExt, path::Path};
use symphonia::core::formats::Track as SymphoniaTrack;
use symphonia::core::io::MediaSourceStream;

use walkdir::WalkDir;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Track {
    pub song_id: i64,
    pub number: Option<i64>,
    pub path: String,
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub length: OrderedFloat<f64>,
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

pub async fn find_music(path: impl AsRef<Path>, conn: &mut Transaction<'_, Sqlite>) -> Result<()> {
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
    let artist = tag.artist();
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
