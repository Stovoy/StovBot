use crate::bot::SharedState;
use crate::Event;
use crossbeam::channel::Sender;
use std::io;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum AdminEvent {
    Message(String),
}

pub fn cli_run(sender: Sender<Event>, shared_state: Arc<Mutex<SharedState>>) {
    loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        let buffer = buffer.trim();
        let event = AdminEvent::Message(buffer.to_string());
        sender.send(Event::AdminEvent(event)).unwrap();
        let mut shared_state = shared_state.lock().unwrap();
        if let Some(waker) = shared_state.waker.take() {
            waker.wake()
        }
    }
}
