use crate::admin::AdminEvent;
use crate::command::CommandExt;
use crate::command::Commands;
use crate::database::Database;
use crate::discord::DiscordEvent;
use crate::models::{
    Action, ActionError, Command, EditType, Message, Source, User, Variable, VariableValue,
};
use crate::twitch::TwitchEvent;
use crate::{special_command, Event, EventBusSender};
use crossbeam::channel::Receiver;
use regex::Regex;
use rusqlite::Error;
use serde::{Deserialize, Serialize};
use serenity::http::AttachmentType as DiscordAttachmentType;
use std::cmp::min;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub sender: EventBusSender,
    pub event_rx: Receiver<Event>,

    pub database: Database,
}

impl Bot {
    pub fn new(sender: EventBusSender, event_rx: Receiver<Event>) -> Result<Bot, Error> {
        let database = Database::new()?;
        let mut commands = special_command::commands();
        commands.append(database.get_commands()?.as_mut());
        for command in commands.iter() {
            sender.send(Event::BotEvent(BotEvent::LoadCommand(command.clone())));
        }
        for variable in database.get_variables()? {
            sender.send(Event::BotEvent(BotEvent::LoadVariable(variable)));
        }
        let stovbot = Bot {
            username: "StovBot".to_string(),
            commands: Commands::new(commands),
            sender,
            event_rx,
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
            let message = match self.event_rx.recv() {
                Ok(event) => match event {
                    Event::BotEvent(_) => None,
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
                    Event::DiscordEvent(event) => match event {
                        DiscordEvent::Ready(_, _) => None,
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

    fn process_command(
        &self,
        command: &Command,
        message: &Message,
    ) -> Result<(BotMessage, Option<Action>), ActionError> {
        let mut deferred_action = None;
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
                        Action::EditVariable(variable, edit_type) => {
                            // TODO: Catch other DB connection errors.
                            match self.database.get_variable(&variable.name) {
                                Ok(old_variable) => match edit_type {
                                    EditType::RemoveAt(_) => None,
                                    _ => match (&variable.value, old_variable.value) {
                                        (VariableValue::Text(_), VariableValue::Text(_)) => None,
                                        (
                                            VariableValue::StringList(_),
                                            VariableValue::StringList(_),
                                        ) => None,
                                        _ => Some(ActionError::VariableWrongType),
                                    },
                                },
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
            None => {
                let response = command.respond_no_check(message);
                if command.is_alias {
                    match self.get_triggered_command(&response.text) {
                        None => Err(ActionError::BadCommandAlias),
                        Some(command) => {
                            self.process_command(
                                command,
                                &Message {
                                    sender: message.sender.clone(),
                                    text: response.text,
                                    // TODO: Way to move or clone the source?
                                    source: Source::Admin,
                                },
                            )
                        }
                    }
                } else {
                    Ok((response, deferred_action))
                }
            }
            Some(e) => Ok((
                BotMessage {
                    text: format!("{:?}", e),
                },
                None,
            )),
        }
    }

    fn get_triggered_command(&self, text: &String) -> Option<&Command> {
        self.commands
            .iter()
            .filter(|command| command.matches_trigger(text))
            .max_by_key(|command| command.trigger.len())
    }

    fn respond(&mut self, message: &Message) -> Option<BotMessage> {
        if message.sender.username == self.username {
            return None;
        }

        let triggered_command = self.get_triggered_command(&message.text);
        let (response, action) = match triggered_command {
            None => (None, None),
            Some(command) => match self.process_command(command, message) {
                Err(e) => (
                    Some(BotMessage {
                        text: format!("{:?}", e),
                    }),
                    None,
                ),
                Ok((response, action)) => (Some(response), action),
            },
        };

        // Deferred because it modifies self.commands,
        // but it'd be nice to propagate these error messages properly.
        // TODO: We could do the database bits first, then defer only adding to commands.
        let event = match action {
            None => None,
            Some(action) => match action {
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
                Action::EditVariable(mut variable, edit_type) => {
                    if edit_type != EditType::Overwrite() {
                        let old_variable = match self.database.get_variable(&variable.name) {
                            Ok(v) => Some(v),
                            Err(e) => {
                                println!("Error editing variable {}: {}", variable.name, e);
                                None
                            }
                        };
                        if let Some(old_variable) = old_variable {
                            match edit_type {
                                EditType::Append() => match (&variable.value, old_variable.value) {
                                    (
                                        VariableValue::Text(new_text),
                                        VariableValue::Text(old_text),
                                    ) => {
                                        variable.value = VariableValue::Text(old_text + new_text);
                                    }
                                    (
                                        VariableValue::StringList(new_list),
                                        VariableValue::StringList(old_list),
                                    ) => {
                                        let mut list = old_list.clone();
                                        list.extend(new_list.clone());
                                        variable.value = VariableValue::StringList(list);
                                    }
                                    _ => {}
                                },
                                EditType::Remove() => match (&variable.value, old_variable.value) {
                                    (
                                        VariableValue::Text(text_to_remove),
                                        VariableValue::Text(old_text),
                                    ) => {
                                        variable.value = VariableValue::Text(
                                            old_text.replace(text_to_remove, ""),
                                        );
                                    }
                                    (
                                        VariableValue::StringList(new_list),
                                        VariableValue::StringList(mut old_list),
                                    ) => {
                                        for item_to_remove in new_list.iter() {
                                            old_list.retain(|old_item| {
                                                old_item.value != item_to_remove.value
                                            });
                                        }
                                        variable.value = VariableValue::StringList(old_list);
                                    }
                                    _ => {}
                                },
                                EditType::InsertAt(index) => {
                                    match (&variable.value, old_variable.value) {
                                        (
                                            VariableValue::Text(text_to_insert),
                                            VariableValue::Text(mut old_text),
                                        ) => {
                                            let index = min(index, old_text.len());
                                            old_text.insert_str(index, text_to_insert);
                                            variable.value = VariableValue::Text(old_text);
                                        }
                                        (
                                            VariableValue::StringList(new_list),
                                            VariableValue::StringList(mut old_list),
                                        ) => {
                                            let index = min(index, old_list.len());
                                            for item_to_insert in new_list.iter().rev() {
                                                old_list.insert(index, item_to_insert.clone());
                                            }
                                            variable.value = VariableValue::StringList(old_list);
                                        }
                                        _ => {}
                                    }
                                }
                                EditType::RemoveAt(index) => match old_variable.value {
                                    VariableValue::Text(mut old_text) => {
                                        let index = min(index, old_text.len());
                                        old_text.remove(index);
                                        variable.value = VariableValue::Text(old_text);
                                    }
                                    VariableValue::StringList(mut old_list) => {
                                        let index = min(index, old_list.len());
                                        old_list.remove(index);
                                        variable.value = VariableValue::StringList(old_list);
                                    }
                                },
                                _ => {}
                            }
                        }
                    };
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
            Some(event) => self.sender.send(Event::BotEvent(event)),
        };

        response
    }

    fn send_message(&self, source: &Source, text: &str) {
        let image_regex = Regex::new(r"\{\{IMAGE\|(.*?)}}").unwrap();

        let mut png: Option<Vec<u8>> = None;

        let mut without_image = String::with_capacity(text.len());
        let text = match image_regex.captures(text) {
            None => text,
            Some(matches) => {
                let png_base64 = &matches[1];
                png = match base64::decode(png_base64) {
                    Ok(png) => Some(png),
                    Err(e) => {
                        println!("Error decoded image base64: {}", e);
                        None
                    }
                };
                let full_match = matches.get(0).unwrap();
                let (l, _) = text.split_at(full_match.start());
                let (_, r) = text.split_at(full_match.end());
                without_image += l;
                without_image += r;
                &without_image
            }
        };

        match source {
            #[cfg(test)]
            Source::None => {}
            Source::Admin => println!("{}", text),
            Source::Twitch(writer, channel) => match png {
                Some(_) => {
                    let text = format!("{} (only works in discord)", text).to_string();
                    writer.send(channel, text).unwrap();
                }
                None => {
                    writer.send(channel, text).unwrap();
                }
            },
            Source::Discord(ctx, msg) => {
                if let Err(e) = msg.channel_id.send_message(&ctx.lock().unwrap().http, |m| {
                    m.content(text);
                    if let Some(ref png) = png {
                        m.embed(|e| {
                            e.image("attachment://waifu.png");
                            e
                        });
                        m.add_file(DiscordAttachmentType::Bytes {
                            data: std::borrow::Cow::Borrowed(png.as_slice()),
                            filename: "waifu.png".to_string(),
                        });
                    }
                    m
                }) {
                    println!("Error sending message: {:?}", e);
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

    pub fn after_trigger(&self, trigger: &str) -> &str {
        if trigger.len() + 1 > self.text.len() {
            ""
        } else {
            let (_, text) = self.text.split_at(trigger.len() + 1);
            text
        }
    }
}
