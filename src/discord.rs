use crate::{Event, EventBusSender};
use serenity::async_trait;
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
    sender: EventBusSender,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        self.sender.send(Event::DiscordEvent(DiscordEvent::Message(
            Box::new(Arc::new(Mutex::new(ctx))),
            Box::new(msg),
        )));
    }

    async fn ready(&self, ctx: Context, msg: Ready) {
        let mut notification_channel_id = None;
        for guild_status in msg.guilds.iter() {
            let guild_id = guild_status.id;
            if let Ok(channels) = guild_id.channels(&ctx.http).await {
                for channel in channels.values() {
                    if channel.name == "stream-is-on" {
                        notification_channel_id = Some(channel.id);
                        break;
                    }
                }
            }
            if notification_channel_id.is_some() {
                break;
            }
        }
        match notification_channel_id {
            None => println!("Could not find stream-is-on channel"),
            Some(id) => {
                self.sender.send(Event::DiscordEvent(DiscordEvent::Ready(
                    Box::new(Arc::new(Mutex::new(ctx))),
                    id,
                )));
            }
        }
    }
}

pub async fn connect(token: String, sender: EventBusSender) -> Client {
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    Client::builder(token, intents)
        .event_handler(Handler { sender })
        .await
        .expect("Err creating client")
}
