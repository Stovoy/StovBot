use crate::Event;
use crossbeam::channel::Sender;
use futures::task::Waker;
use std::io;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum AdminEvent {
    Message(String),
}

pub fn cli_run(sender: Sender<Event>, stream_waker: Arc<Mutex<Option<Waker>>>) {
    loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        let buffer = buffer.trim();
        let event = AdminEvent::Message(buffer.to_string());
        sender.send(Event::AdminEvent(event)).unwrap();
        let mut stream_waker = stream_waker.lock().unwrap();
        if let Some(waker) = stream_waker.take() {
            waker.wake()
        }
    }
}
