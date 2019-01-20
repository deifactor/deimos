use std::sync::mpsc;
use std::time::Duration;
use std::{io, thread};
use termion::input::TermRead;

pub enum Event {
    Input(termion::event::Event),
    // Occurs once every `tick_duration`.
    Tick,
}

/// Configures how Event
pub struct Config {
    pub tick_duration: Duration,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            tick_duration: Duration::from_millis(100),
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
                        tx.send(Event::Input(evt));
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

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event);
}
