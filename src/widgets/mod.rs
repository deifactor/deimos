mod app;
mod now_playing;
mod queue;

use maplit::hashmap;

pub use self::app::App;
pub use self::now_playing::NowPlaying;
pub use self::queue::Queue;

// Some utility functions that are useful for writing widgets.
fn song_values(song: &mpd::song::Song) -> std::collections::HashMap<&str, String> {
    hashmap![
        "title" => song.title.clone().unwrap_or("Unknown".to_owned()),
        "artist" => song.tags.get("Artist").cloned().unwrap_or("Unknown".to_owned()),
        "album" => song.tags.get("Album").cloned().unwrap_or("Unknown".to_owned()),
    ]
}
