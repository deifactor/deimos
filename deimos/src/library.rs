use anyhow::{anyhow, Result};
use sqlx::{sqlite::SqliteConnectOptions, Executor, Pool, Sqlite, SqlitePool, Transaction};
use std::{fs::File, os::unix::prelude::OsStrExt, path::Path};
use symphonia::core::{
    io::MediaSourceStream,
    meta::{Metadata, StandardTagKey, Value},
};

use walkdir::WalkDir;

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
        let Ok(mut probed) = probed else { continue; };
        if let Some(metadata) = probed.metadata.get() {
            insert_song(entry.path(), &metadata, conn).await?;
        }
    }
    Ok(())
}

async fn insert_song(
    path: &Path,
    metadata: &Metadata<'_>,
    conn: &mut Transaction<'_, Sqlite>,
) -> Result<()> {
    let mut title: Option<String> = None;
    let mut album: Option<String> = None;
    let mut artist: Option<String> = None;
    let mut number: Option<i64> = None;
    for tag in metadata
        .current()
        .ok_or(anyhow!("no metadata found"))?
        .tags()
    {
        use StandardTagKey::*;
        match (tag.std_key, &tag.value) {
            (Some(TrackTitle), Value::String(s)) => title = Some(s.clone()),
            (Some(Album), Value::String(s)) => album = Some(s.clone()),
            (Some(AlbumArtist), Value::String(s)) => artist = Some(s.clone()),
            (Some(TrackNumber), Value::UnsignedInt(i)) => number = Some(*i as i64),
            (Some(TrackNumber), Value::SignedInt(i)) => number = Some(*i),
            (Some(Artist), Value::String(s)) => {
                if artist.is_none() {
                    artist = Some(s.clone())
                }
            }
            _ => (),
        }
    }

    let path_bytes = path.as_os_str().as_bytes();
    sqlx::query!(
        "INSERT INTO songs (path, number, title, album, artist) VALUES (?, ?, ?, ?, ?)",
        path_bytes,
        number,
        title,
        album,
        artist
    )
    .execute(conn)
    .await?;
    Ok(())
}
