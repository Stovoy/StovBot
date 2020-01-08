use crate::bot::SharedState;
use crossbeam::channel;
use serenity::{
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum DiscordEvent {
    Ready,
    Message(Box<Context>, Box<Message>),
}

struct Handler {
    senders: Vec<channel::Sender<DiscordEvent>>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl Handler {
    fn send_event(&self, event: DiscordEvent) {
        self.senders
            .iter()
            .for_each(|s| s.send(event.clone()).unwrap());
        let mut shared_state = self.shared_state.lock().unwrap();
        if let Some(waker) = shared_state.waker.take() {
            waker.wake()
        }
    }
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        self.send_event(DiscordEvent::Message(Box::new(ctx), Box::new(msg)));
    }

    fn ready(&self, _: Context, _: Ready) {
        self.send_event(DiscordEvent::Ready);
    }
}

pub fn connect(
    token: String,
    senders: Vec<channel::Sender<DiscordEvent>>,
    shared_state: Arc<Mutex<SharedState>>,
) -> Client {
    Client::new(
        &token,
        Handler {
            senders,
            shared_state,
        },
    )
    .expect("Err creating client")
}
