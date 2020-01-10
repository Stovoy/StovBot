use crate::Event;
use crossbeam::channel::Sender;
use futures::task::Waker;
use serenity::model::id::ChannelId;
use serenity::{
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum DiscordEvent {
    Ready(Box<Arc<Mutex<Context>>>, ChannelId),
    Message(Box<Arc<Mutex<Context>>>, Box<Message>),
}

struct Handler {
    sender: Sender<Event>,
    stream_waker: Arc<Mutex<Option<Waker>>>,
}

impl Handler {
    fn send_event(&self, event: DiscordEvent) {
        self.sender.send(Event::DiscordEvent(event)).unwrap();
        let mut stream_waker = self.stream_waker.lock().unwrap();
        if let Some(waker) = stream_waker.take() {
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

    fn ready(&self, ctx: Context, msg: Ready) {
        let mut notification_channel_id = None;
        for guild_status in msg.guilds.iter() {
            for channel in guild_status.id().channels(ctx.http.clone()).unwrap() {
                if channel.1.name == "stream-is-on" {
                    notification_channel_id = Some(channel.1.id);
                    break;
                }
            }
        }
        match notification_channel_id {
            None => panic!("Could not find stream-is-on channel"),
            Some(id) => {
                self.send_event(DiscordEvent::Ready(Box::new(Arc::new(Mutex::new(ctx))), id))
            }
        }
    }
}

pub fn connect(
    token: String,
    sender: Sender<Event>,
    stream_waker: Arc<Mutex<Option<Waker>>>,
) -> Client {
    Client::new(
        &token,
        Handler {
            sender,
            stream_waker,
        },
    )
    .expect("Err creating client")
}
