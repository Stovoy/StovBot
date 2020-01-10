use crate::bot::SharedState;
use crate::Event;
use crossbeam::channel::Sender;
use serenity::{
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum DiscordEvent {
    Ready,
    Message(Box<Arc<Mutex<Context>>>, Box<Message>),
}

struct Handler {
    sender: Sender<Event>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl Handler {
    fn send_event(&self, event: DiscordEvent) {
        self.sender.send(Event::DiscordEvent(event)).unwrap();
        let mut shared_state = self.shared_state.lock().unwrap();
        if let Some(waker) = shared_state.waker.take() {
            waker.wake()
        }
    }
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        self.send_event(DiscordEvent::Message(
            Box::new(Arc::new(Mutex::new(ctx))),
            Box::new(msg),
        ));
    }

    fn ready(&self, _: Context, _: Ready) {
        self.send_event(DiscordEvent::Ready);
    }
}

pub fn connect(
    token: String,
    sender: Sender<Event>,
    shared_state: Arc<Mutex<SharedState>>,
) -> Client {
    Client::new(
        &token,
        Handler {
            sender,
            shared_state,
        },
    )
    .expect("Err creating client")
}
