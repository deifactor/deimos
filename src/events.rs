use std::sync::mpsc;
use std::time::Duration;
use std::{io, thread};
use termion::input::TermRead;

#[derive(Debug)]
pub enum Event {
    Input(termion::event::Event),
    // Occurs once every `tick_duration`.
    Tick,
}

impl Event {
    /// If this event is a key, returns the inner [termion::event::Event::Key].
    /// Otherwise, returns `None`.
    pub fn key(&self) -> Option<&termion::event::Key> {
        if let Event::Input(termion::event::Event::Key(ref k)) = self {
            Some(k)
        } else {
            None
        }
    }
}

/// Configures how Event
pub struct Config {
    pub tick_duration: Duration,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            tick_duration: Duration::from_millis(500),
        }
    }
}

/// The base event handler. Sends all key/mouse events and a
pub struct EventReceiver {
    rx: mpsc::Receiver<Event>,
}

impl EventReceiver {
    pub fn new(config: Config) -> EventReceiver {
        let (tx, rx) = mpsc::channel();
        // Start listening for input events.
        {
            let tx = tx.clone();
            thread::spawn(move || {
                let stdin = io::stdin();
                for event in stdin.events() {
                    if let Ok(evt) = event {
                        tx.send(Event::Input(clean_termion_event(evt)));
                    }
                }
            });
        }
        // Start sending tick events.
        {
            let tx = tx.clone();
            thread::spawn(move || loop {
                tx.send(Event::Tick);
                thread::sleep(config.tick_duration);
            });
        }
        EventReceiver { rx }
    }

    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.rx.recv()
    }
}

/// Adds proper handling for arrow keys, which don't seem to work on my setup
/// (iTerm2); it just passes them through as unrecognized key sequences. XXX:
/// fix this in termion.
fn clean_termion_event(event: termion::event::Event) -> termion::event::Event {
    use termion::event::{Event, Key};
    if let termion::event::Event::Unsupported(ref bytes) = event {
        let mut it = bytes.iter();
        if it.next() == Some(&b'\x1B') && it.next() == Some(&b'O') {
            match it.next() {
                Some(b'A') => return Event::Key(Key::Up),
                Some(b'B') => return Event::Key(Key::Down),
                Some(b'C') => return Event::Key(Key::Right),
                Some(b'D') => return Event::Key(Key::Left),
                _ => (),
            }
        }
    }
    event
}

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event);
}
