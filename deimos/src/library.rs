use anyhow::{bail, Result};
use sqlx::{
    pool::PoolConnection,
    sqlite::{SqliteConnectOptions},
    Executor, Sqlite, SqlitePool,
};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use symphonia::core::{
    io::MediaSourceStream,
    meta::{MetadataRevision, StandardTagKey, Value},
};
use tokio::fs::remove_file;

use walkdir::WalkDir;

/// Initialize the song database, creating all tables. This deletes any existing database.
pub async fn initialize_db(path: impl AsRef<Path>) -> Result<PoolConnection<Sqlite>> {
    remove_file(&path).await?;

    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true),
    )
    .await?;
    let mut conn = pool.acquire().await?;
    conn.execute(include_str!("./create_db.sql")).await?;
    Ok(conn)
}

pub fn find_music(path: impl AsRef<Path>) -> Result<PathBuf> {
    let probe = symphonia::default::get_probe();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        println!("{:?}", entry.path());
        let file = File::open(entry.path())?;
        let media_source = MediaSourceStream::new(Box::new(file), Default::default());
        let mut probed = probe.format(
            &Default::default(),
            media_source,
            &Default::default(),
            &Default::default(),
        )?;
        if probed.metadata.get().is_some() {
            return Ok(PathBuf::from(entry.path()));
        }
    }
    bail!("couldn't find any music")
}

fn get_title(rev: &MetadataRevision) -> Option<&Value> {
    for tag in rev.tags() {
        if tag.std_key == Some(StandardTagKey::TrackTitle) {
            return Some(&tag.value);
        }
    }
    None
}
