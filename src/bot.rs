use crate::admin::AdminEvent;
use crate::command::CommandExt;
use crate::command::Commands;
use crate::database::Database;
#[cfg(feature = "discord")]
use crate::discord::DiscordEvent;
use crate::models::{Action, ActionError, Command, Message, Source, User, Variable};
#[cfg(feature = "twitch")]
use crate::twitch::TwitchEvent;
use crate::{special_command, Event};
use bus::BusReader;
use crossbeam::channel::Sender;
use futures::task::Waker;
use rusqlite::Error;
#[cfg(feature = "discord")]
use serenity::utils::MessageBuilder as DiscordMessageBuilder;
use std::sync::{Arc, Mutex};

pub struct SharedState {
    pub waker: Option<Waker>,
}

#[derive(Debug, Clone)]
pub enum BotEvent {
    // On initial load from the database.
    LoadCommand(Command),
    // User command actions.
    AddCommand(Command, User),
    EditCommand(Command, User),
    DeleteCommand(Command, User),

    // On initial load from the database.
    LoadVariable(Variable),
    // User variable actions.
    AddVariable(Variable, User),
    EditVariable(Variable, User),
    DeleteVariable(Variable, User),
}

pub struct Bot {
    pub username: String,
    pub commands: Commands,

    pub event_sender: Sender<Event>,
    pub event_rx: BusReader<Event>,
    pub shared_state: Arc<Mutex<SharedState>>,

    pub database: Database,
}

impl Bot {
    pub fn new(
        event_sender: Sender<Event>,
        event_rx: BusReader<Event>,
        shared_state: Arc<Mutex<SharedState>>,
    ) -> Result<Bot, Error> {
        let database = Database::new()?;
        let mut commands = special_command::commands();
        commands.append(database.get_commands()?.as_mut());
        for command in commands.iter() {
            send_event(
                &event_sender,
                &shared_state,
                BotEvent::LoadCommand(command.clone()),
            );
        }
        for variable in database.get_variables()? {
            send_event(
                &event_sender,
                &shared_state,
                BotEvent::LoadVariable(variable),
            );
        }
        let stovbot = Bot {
            username: "StovBot".to_string(),
            commands: Commands::new(commands),
            event_sender,
            event_rx,
            shared_state,
            database,
        };
        Ok(stovbot)
    }

    fn is_builtin_command(&self, command: &Command) -> bool {
        Command::default_commands()
            .iter()
            .any(|default_command| default_command.trigger == command.trigger)
            || special_command::commands()
                .iter()
                .any(|special_command| special_command.trigger == command.trigger)
    }

    pub fn run(&mut self) {
        loop {
            let message = match self.event_rx.try_recv() {
                Ok(event) => match event {
                    Event::BotEvent(_) => None,
                    #[cfg(feature = "twitch")]
                    Event::TwitchEvent(event) => match event {
                        TwitchEvent::Ready(writer) => {
                            writer.join("stovoy").unwrap();
                            None
                        }
                        TwitchEvent::PrivMsg(writer, msg) => Some(Message {
                            sender: User {
                                username: msg.user().to_string(),
                            },
                            text: msg.message().to_string(),
                            source: Source::Twitch(writer, "stovoy".to_string()),
                        }),
                    },
                    #[cfg(feature = "discord")]
                    Event::DiscordEvent(event) => match event {
                        DiscordEvent::Ready => None,
                        DiscordEvent::Message(ctx, msg) => Some(Message {
                            sender: User {
                                username: msg.author.name.to_string(),
                            },
                            text: msg.content.to_string(),
                            source: Source::Discord(ctx, msg),
                        }),
                    },
                    Event::AdminEvent(event) => match event {
                        AdminEvent::Message(msg) => Some(Message {
                            sender: User {
                                username: "Stovoy".to_string(),
                            },
                            text: msg,
                            source: Source::Admin,
                        }),
                    },
                },
                Err(_) => None,
            };
            match message {
                None => {}
                Some(message) => match self.respond(&message) {
                    None => {}
                    Some(response) => self.send_message(&message.source, &response.text),
                },
            };
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
                let action_error = match &command.actor {
                    None => None,
                    Some(actor) => match actor.0(&command, message) {
                        // TODO: Add GetCommand and GetVariable which respond with the raw data.
                        Ok(action) => {
                            let action_error = match &action {
                                Action::AddCommand(command) => {
                                    if self.commands.contains(command) {
                                        Some(ActionError::CommandAlreadyExists)
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
                                Action::DeleteCommand(command) => {
                                    if !self.commands.contains(command) {
                                        Some(ActionError::CommandDoesNotExist)
                                    } else if self.is_builtin_command(command) {
                                        Some(ActionError::CannotDeleteBuiltInCommand)
                                    } else {
                                        None
                                    }
                                }
                                Action::AddVariable(variable) => {
                                    match self.database.get_variable(&variable.name) {
                                        Ok(_) => Some(ActionError::VariableAlreadyExists),
                                        Err(_) => None,
                                    }
                                }
                                Action::EditVariable(variable) => {
                                    // TODO: Catch other DB connection errors.
                                    match self.database.get_variable(&variable.name) {
                                        Ok(_) => None,
                                        Err(_) => Some(ActionError::VariableDoesNotExist),
                                    }
                                }
                                Action::DeleteVariable(variable) => {
                                    // TODO: Catch other DB connection errors.
                                    match self.database.get_variable(&variable.name) {
                                        Ok(_) => None,
                                        Err(_) => Some(ActionError::VariableDoesNotExist),
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

        // Deferred because it modifies self.commands,
        // but it'd be nice to propagate these error messages properly.
        // TODO: We could do the database bits first, then defer only adding to commands.
        let event = match deferred_action {
            None => None,
            Some(deferred_action) => match deferred_action {
                Action::AddCommand(command) => {
                    if let Err(e) = self.database.add_command(&command) {
                        println!("Error adding command {}: {}", command.trigger, e)
                    }
                    self.commands.update_command(&command);
                    Some(BotEvent::AddCommand(command, message.sender.clone()))
                }
                Action::EditCommand(command) => {
                    if let Err(e) = self.database.update_command(&command) {
                        println!("Error updating command {}: {}", command.trigger, e)
                    }
                    self.commands.update_command(&command);
                    Some(BotEvent::EditCommand(command, message.sender.clone()))
                }
                Action::DeleteCommand(command) => {
                    if let Err(e) = self.database.delete_command(&command) {
                        println!("Error deleting command {}: {}", command.trigger, e)
                    }
                    self.commands.delete_command(&command);
                    Some(BotEvent::DeleteCommand(command, message.sender.clone()))
                }
                Action::AddVariable(variable) => {
                    if let Err(e) = self.database.set_variable(&variable) {
                        println!("Error adding variable {}: {}", variable.name, e)
                    }
                    Some(BotEvent::AddVariable(variable, message.sender.clone()))
                }
                Action::EditVariable(variable) => {
                    if let Err(e) = self.database.set_variable(&variable) {
                        println!("Error editing variable {}: {}", variable.name, e)
                    }
                    Some(BotEvent::EditVariable(variable, message.sender.clone()))
                }
                Action::DeleteVariable(variable) => {
                    if let Err(e) = self.database.delete_variable(&variable) {
                        println!("Error deleting variable {}: {}", variable.name, e)
                    }
                    Some(BotEvent::DeleteVariable(variable, message.sender.clone()))
                }
            },
        };

        match event {
            None => {}
            Some(event) => self.send_event(event),
        };

        response
    }

    fn send_message(&self, source: &Source, text: &str) {
        match source {
            #[cfg(test)]
            Source::None => {}
            Source::Admin => println!("{}", text),
            #[cfg(feature = "twitch")]
            Source::Twitch(writer, channel) => {
                writer.send(channel, text).unwrap();
            }
            #[cfg(feature = "discord")]
            Source::Discord(ctx, msg) => {
                let response = DiscordMessageBuilder::new().push(text).build();
                if let Err(why) = msg.channel_id.say(&ctx.lock().unwrap().http, &response) {
                    println!("Error sending message: {:?}", why);
                }
            }
        }
    }

    fn send_event(&self, event: BotEvent) {
        send_event(&self.event_sender, &self.shared_state, event)
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

    pub fn after_trigger(&self, trigger: &str) -> &str {
        if trigger.len() + 1 > self.text.len() {
            ""
        } else {
            let (_, text) = self.text.split_at(trigger.len() + 1);
            text
        }
    }
}

fn send_event(sender: &Sender<Event>, shared_state: &Arc<Mutex<SharedState>>, event: BotEvent) {
    sender.send(Event::BotEvent(event)).unwrap();
    let mut shared_state = shared_state.lock().unwrap();
    if let Some(waker) = shared_state.waker.take() {
        waker.wake()
    }
}
