use crate::command;
use crate::db::Database;
use crate::discord::DiscordEvent;
use crate::twitch::TwitchEvent;
use crossbeam::channel::{select, Receiver, Sender};
use rusqlite::Error;
use serenity::model::channel::Message as DiscordMessage;
use serenity::prelude::Context as DiscordContext;
use serenity::utils::MessageBuilder as DiscordMessageBuilder;
use twitchchat::Writer;

#[derive(Debug, Clone)]
pub struct BotEvent {}

pub struct Bot {
    pub(crate) username: String,
    pub(crate) commands: Vec<command::Command>,

    #[allow(dead_code)]
    pub(crate) bot_event_sender: Sender<BotEvent>,

    pub(crate) twitch_event_receiver: Receiver<TwitchEvent>,
    pub(crate) discord_event_receiver: Receiver<DiscordEvent>,
    pub(crate) twitch_writer: Writer,

    pub(crate) database: Database,
}

impl Bot {
    pub(crate) fn new(
        bot_event_sender: Sender<BotEvent>,
        twitch_event_receiver: Receiver<TwitchEvent>,
        discord_event_receiver: Receiver<DiscordEvent>,
        twitch_writer: Writer,
    ) -> Result<Bot, Error> {
        let database = Database::new()?;
        let mut stovbot = Bot {
            username: "StovBot".to_string(),
            commands: Vec::new(),
            bot_event_sender,
            twitch_event_receiver,
            discord_event_receiver,
            twitch_writer,
            database,
        };
        stovbot.load_commands()?;
        Ok(stovbot)
    }

    fn load_commands(&mut self) -> Result<(), Error> {
        self.commands = self.database.get_commands()?;
        Ok(())
    }

    pub fn run(&self) {
        loop {
            let message = select! {
                recv(self.twitch_event_receiver) -> msg => match msg {
                    Ok(event) => match event {
                        TwitchEvent::Ready => {
                            self.twitch_writer.join("stovoy").unwrap();
                            None
                        }
                        TwitchEvent::PrivMsg(msg) => Some(Message {
                            sender: User {
                                username: msg.user().to_string(),
                            },
                            text: msg.message().to_string(),
                            source: Source::Twitch("stovoy".to_string()),
                        })
                    }
                    Err(_) => None,
                },
                recv(self.discord_event_receiver) -> msg => match msg {
                    Ok(event) => match event {
                        DiscordEvent::Ready => None,
                        DiscordEvent::Message(ctx, msg) => Some(Message {
                            sender: User {
                                username: msg.author.name.to_string(),
                            },
                            text: msg.content.to_string(),
                            source: Source::Discord(ctx, msg),
                        })
                    }
                    Err(_) => None,
                },
            };
            match message {
                None => {}
                Some(message) => {
                    let responses = self.respond(&message);
                    for response in responses.iter() {
                        self.send_message(&message.source, &response.text);
                    }
                }
            }
        }
    }

    fn respond(&self, message: &Message) -> Vec<BotMessage> {
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
