use crate::command::CommandExt;
use crate::command::Commands;
use crate::database::Database;
use crate::discord::DiscordEvent;
use crate::models::{Action, ActionError, Command, Message, Source, User};
use crate::special_command;
use crate::twitch::TwitchEvent;
use crossbeam::channel::{select, Receiver, Sender};
use futures::task::Waker;
use rusqlite::Error;
use serenity::utils::MessageBuilder as DiscordMessageBuilder;
use twitchchat::Writer;

pub struct SharedState {
    pub waker: Option<Waker>,
}

#[derive(Debug, Clone)]
pub struct BotEvent {}

pub struct Bot {
    pub username: String,
    pub commands: Commands,

    #[allow(dead_code)]
    pub bot_event_sender: Sender<BotEvent>,

    pub twitch_event_receiver: Receiver<TwitchEvent>,
    pub discord_event_receiver: Receiver<DiscordEvent>,
    pub twitch_writer: Writer,

    #[allow(dead_code)]
    pub database: Database,
}

impl Bot {
    pub fn new(
        bot_event_sender: Sender<BotEvent>,
        twitch_event_receiver: Receiver<TwitchEvent>,
        discord_event_receiver: Receiver<DiscordEvent>,
        twitch_writer: Writer,
    ) -> Result<Bot, Error> {
        let database = Database::new()?;
        let mut commands = special_command::commands();
        commands.append(database.get_commands()?.as_mut());
        let stovbot = Bot {
            username: "StovBot".to_string(),
            commands: Commands::new(commands),
            bot_event_sender,
            twitch_event_receiver,
            discord_event_receiver,
            twitch_writer,
            database,
        };
        Ok(stovbot)
    }

    fn is_builtin_command(&self, command: &Command) -> bool {
        Command::default_commands()
            .iter()
            .find(|default_command| default_command.trigger == command.trigger)
            .is_some()
            || special_command::commands()
                .iter()
                .find(|special_command| special_command.trigger == command.trigger)
                .is_some()
    }

    pub fn run(&mut self) {
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
                Some(message) => match self.respond(&message) {
                    None => {}
                    Some(response) => self.send_message(&message.source, &response.text),
                },
            }
        }
    }

    fn respond(&mut self, message: &Message) -> Option<BotMessage> {
        if message.sender.username == self.username {
            return None;
        }

        let mut deferred_action = None;

        let triggered_command = self
            .commands
            .iter()
            .find(|command| command.matches_trigger(message));
        let response = match triggered_command {
            None => None,
            Some(command) => {
                let action_error = match command.actor {
                    None => None,
                    Some(actor) => match actor(&command, message) {
                        Ok(action) => {
                            let action_error = match &action {
                                Action::AddCommand(command) => {
                                    match self.commands.contains(command) {
                                        true => Some(ActionError::CommandAlreadyExists),
                                        false => None,
                                    }
                                }
                                Action::DeleteCommand(command) => {
                                    if !self.commands.contains(command) {
                                        Some(ActionError::CommandDoesNotExist)
                                    } else if self.is_builtin_command(command) {
                                        Some(ActionError::CannotDeleteBuiltInCommand)
                                    } else {
                                        None
                                    }
                                }
                                Action::EditCommand(command) => {
                                    if !self.commands.contains(command) {
                                        Some(ActionError::CommandDoesNotExist)
                                    } else if self.is_builtin_command(command) {
                                        Some(ActionError::CannotModifyBuiltInCommand)
                                    } else {
                                        None
                                    }
                                }
                            };
                            match action_error {
                                None => deferred_action = Some(action),
                                Some(_) => {}
                            };
                            action_error
                        }
                        Err(e) => Some(e),
                    },
                };
                match action_error {
                    None => Some(command.respond_no_check(message)),
                    Some(e) => Some(BotMessage {
                        text: format!("{:?}", e),
                    }),
                }
            }
        };

        match deferred_action {
            None => {}
            Some(deferred_action) => match deferred_action {
                Action::AddCommand(command) => {
                    if let Err(e) = self.database.add_command(&command) {
                        println!("Error adding command {}: {}", command.trigger, e)
                    }
                    self.commands.update_command(&command);
                }
                Action::DeleteCommand(command) => {
                    if let Err(e) = self.database.delete_command(&command) {
                        println!("Error deleting command {}: {}", command.trigger, e)
                    }
                    self.commands.delete_command(&command);
                }
                Action::EditCommand(command) => {
                    if let Err(e) = self.database.update_command(&command) {
                        println!("Error updating command {}: {}", command.trigger, e)
                    }
                    self.commands.update_command(&command);
                }
            },
        }

        response
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
    pub text: String,
}

impl Message {
    #[cfg(test)]
    pub fn new(text: String) -> Message {
        Message {
            sender: User {
                username: "foo".to_string(),
            },
            text,
            source: Source::None,
        }
    }

    pub fn after_trigger(&self, trigger: &String) -> &str {
        if trigger.len() + 1 > self.text.len() {
            ""
        } else {
            let (_, text) = self.text.split_at(trigger.len() + 1);
            text
        }
    }
}
