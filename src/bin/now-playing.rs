// run like `now-playing path/to/music/directory`
#![feature(nll)]
use id3;
use mpd;
use std::path::Path;

fn get_album_art<P: AsRef<Path>>(path: P) -> Option<Vec<u8>> {
    let tag = id3::Tag::read_from_path(path).ok()?;
    // TODO: look for the cover
    let picture = tag.pictures().next()?;
    Some(picture.data.clone())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut conn = mpd::Client::connect("127.0.0.1:6600").expect("failed to connect to MPD");
    if let Some(song) = conn.currentsong().expect("failed to get song") {
        let file = song.file;
        if let Some(art) = get_album_art(Path::new(&args[1]).join(file)) {
            println!(
                "\x1b]1337;File=inline=1;width=20:{}\x07\n{} - {:#?}",
                base64::encode(&art),
                song.title.unwrap_or("".into()),
                song.tags
            );
        } else {
            println!("no art");
        }
    }
}
