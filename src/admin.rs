use crate::{Event, EventSender};
use std::io;

#[derive(Debug, Clone)]
pub enum AdminEvent {
    Message(String),
}

pub fn cli_run(sender: EventSender) {
    loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        let buffer = buffer.trim();
        let event = AdminEvent::Message(buffer.to_string());
        sender.send(Event::AdminEvent(event));
    }
}
