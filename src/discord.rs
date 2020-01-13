use crate::{Event, EventSender};
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
    sender: EventSender,
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        self.sender.send(Event::DiscordEvent(DiscordEvent::Message(
            Box::new(Arc::new(Mutex::new(ctx))),
            Box::new(msg),
        )));
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
                self.sender.send(Event::DiscordEvent(DiscordEvent::Ready(
                    Box::new(Arc::new(Mutex::new(ctx))),
                    id,
                )));
            }
        }
    }
}

pub fn connect(token: String, sender: EventSender) -> Client {
    Client::new(&token, Handler { sender }).expect("Err creating client")
}
