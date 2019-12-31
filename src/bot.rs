use crate::command;
use crate::discord::DiscordEvent;
use crate::twitch::TwitchEvent;
use crossbeam::channel;
use serenity::model::channel::Message as DiscordMessage;
use serenity::prelude::Context as DiscordContext;
use serenity::utils::MessageBuilder as DiscordMessageBuilder;

#[derive(Debug, Clone)]
pub struct BotEvent {}

pub struct Bot {
    pub(crate) username: String,
    pub(crate) commands: Vec<command::Command>,

    #[allow(dead_code)]
    pub(crate) bot_event_sender: channel::Sender<BotEvent>,

    pub(crate) twitch_event_receiver: channel::Receiver<TwitchEvent>,
    pub(crate) discord_event_receiver: channel::Receiver<DiscordEvent>,
    pub(crate) twitch_writer: twitchchat::Writer,
}

impl Bot {
    pub fn process_messages(&mut self) {
        let mut messages = Vec::new();
        match self.twitch_event_receiver.try_recv() {
            Ok(event) => match event {
                TwitchEvent::Ready => {
                    self.twitch_writer.join("stovoy").unwrap();
                }
                TwitchEvent::PrivMsg(msg) => messages.push(Message {
                    sender: User {
                        username: msg.user().to_string(),
                    },
                    text: msg.message().to_string(),
                    source: Source::Twitch("stovoy".to_string()),
                }),
            },
            Err(_) => {}
        }
        match self.discord_event_receiver.try_recv() {
            Ok(event) => match event {
                DiscordEvent::Ready => {}
                DiscordEvent::Message(ctx, msg) => {
                    messages.push(Message {
                        sender: User {
                            username: msg.author.name.to_string(),
                        },
                        text: msg.content.to_string(),
                        source: Source::Discord(ctx, msg),
                    });
                }
            },
            Err(_) => {}
        }
        for message in messages.iter() {
            self.debug_message(&format!("{}: {}", message.sender.username, message.text));
            let responses = self.respond(message);
            for response in responses.iter() {
                self.send_message(&message.source, &response.text);
            }
        }
    }

    fn respond(&mut self, message: &Message) -> Vec<BotMessage> {
        let mut responses = Vec::new();
        if message.sender.username == self.username {
            return responses;
        }

        for command in self.commands.iter() {
            match command.respond(message) {
                Some(response) => {
                    responses.push(response);
                }
                _ => {}
            }
        }

        responses
    }

    fn debug_message(&self, text: &String) {
        println!("{}", text);
    }

    fn send_message(&self, source: &Source, text: &String) {
        match source {
            #[cfg(test)]
            Source::None => {}
            Source::Twitch(channel) => {
                self.twitch_writer.send(channel, text).unwrap();
            }
            Source::Discord(ctx, msg) => {
                let response = DiscordMessageBuilder::new().push(text).build();
                if let Err(why) = msg.channel_id.say(&ctx.http, &response) {
                    println!("Error sending message: {:?}", why);
                }
            }
        }
        println!("{}", text);
    }
}

pub struct BotMessage {
    pub(crate) text: String,
}

pub struct Message {
    pub(crate) sender: User,
    pub(crate) text: String,
    pub(crate) source: Source,
}

pub(crate) enum Source {
    #[cfg(test)]
    None,
    Twitch(String),
    Discord(DiscordContext, DiscordMessage),
}

impl Message {
    #[cfg(test)]
    pub(crate) fn new(text: String) -> Message {
        Message {
            sender: User {
                username: "foo".to_string(),
            },
            text,
            source: Source::None,
        }
    }
}

pub(crate) struct User {
    pub(crate) username: String,
}
