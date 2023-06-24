# deimos configuration

The path to the configuration file depends on your operating system.

- **macOS** uses `$HOME/Library/Preferences/zone.synthetic.deimos/config.toml`
- **Windows** uses `%appdata%/deimos/config.toml`
- **Linux** uses `$HOME/.config/deimos/config.toml`

Other OSes aren't supported at the moment. Sorry :(

Configuration options and their default values are given in this file.

## `[format]`

Specifies how various parts of the UI should be formatted. Format strings are in
[mimi](../mimi/README.md) syntax.

All song formatters have the `artist`, `album`, and `title` keys available.

```toml
[format]

# Formats the songs when viewing playlists or the song queue.
playlist-song = "$title - $artist - $album"

# Formats the currently-playing song at the bottom of the display. The default
# value is equal to format.playlist-song.
now-playing = "whatever playlist-song is"
```

