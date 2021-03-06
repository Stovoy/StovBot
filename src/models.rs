use crate::database::Database;
use serde::export::fmt::Error;
use serde::export::Formatter;
use serde::{Deserialize, Serialize};
use serenity::model::channel::Message as DiscordMessage;
use serenity::prelude::Context as DiscordContext;
use std::fmt::Debug;
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use time::Timespec;
use twitchchat::Writer as TwitchWriter;

// Note: Wrapped in struct so that we can implement Debug on it.
#[derive(Clone)]
pub struct Actor(pub fn(&Command, &Message) -> Result<Action, ActionError>);

impl Debug for Actor {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str("<Actor>")?;
        Ok(())
    }
}

pub enum Action {
    AddCommand(Command),
    EditCommand(Command),
    DeleteCommand(Command),
    AddVariable(Variable),
    EditVariable(Variable, EditType),
    DeleteVariable(Variable),
    SendLiveNotification,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditType {
    Overwrite(),
    Append(),
    Remove(),
    InsertAt(usize),
    RemoveAt(usize),
}

#[derive(Debug)]
pub enum ActionError {
    None,
    CommandAlreadyExists,
    CommandDoesNotExist,
    CannotDeleteBuiltInCommand,
    CannotModifyBuiltInCommand,
    BadCommand(String),
    BadCommandTriggerPrefix,
    BadVariable(String),
    BadCommandAlias,
    VariableAlreadyExists,
    VariableDoesNotExist,
    VariableEditTypeNotSupported,
    VariableWrongType,
    VariableBadEditIndex,
    VariableBadEditIndexValue,
    PermissionDenied,
    NotificationChannelNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: i32,
    #[serde(with = "TimespecDef")]
    pub time_created: Timespec,
    pub trigger: String,
    pub response: String,
    #[serde(skip)]
    pub actor: Option<Actor>,
    #[serde(skip)]
    pub database_path: String,
    pub is_alias: bool,
}

pub struct Message {
    pub sender: User,
    pub text: String,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
}

pub enum Source {
    #[cfg(test)]
    None,
    Admin,
    Twitch(TwitchWriter, String),
    Discord(Box<Arc<Mutex<DiscordContext>>>, Box<DiscordMessage>),
}

impl Command {
    pub fn new(trigger: String, response: String) -> Command {
        Command {
            id: 0,
            time_created: time::empty_tm().to_timespec(),
            trigger,
            response,
            actor: None,
            database_path: Database::default_path(),
            is_alias: false,
        }
    }

    pub fn new_alias(trigger: String, alias: String) -> Command {
        Command {
            id: 0,
            time_created: time::empty_tm().to_timespec(),
            trigger,
            response: alias,
            actor: None,
            database_path: Database::default_path(),
            is_alias: true,
        }
    }

    #[cfg(test)]
    pub fn with_database_path(&mut self, database_path: String) -> &mut Command {
        self.database_path = database_path;
        self
    }

    pub fn with_actor(&mut self, actor: Actor) -> &mut Command {
        self.actor = Some(actor);
        self
    }

    pub fn build(&self) -> Command {
        self.clone()
    }

    pub fn default_commands() -> Vec<Command> {
        vec![
            Command::new(
                "!8ball".to_string(),
                "🎱 {{\
                 let responses = [\"All signs point to yes...\", \"Yes!\", \"My sources say nope.\", \
                 \"You may rely on it.\", \"Concentrate and ask again...\", \
                 \"Outlook not so good...\", \"It is decidedly so!\", \
                 \"Better not tell you.\", \"Very doubtful.\", \"Yes - Definitely!\", \
                 \"It is certain!\", \"Most likely.\", \"Ask again later.\", \"No!\", \
                 \"Outlook good.\", \
                 \"Don't count on it.\"]; \
                 responses[floor(random() * len(responses))]\
                 }}".to_string(),
            ),
            Command::new(
                "!quote".to_string(),
                "{{\
                let quotes = get_list(\"quotes\"); \
                let i = int(\"$1\"); if i == 0 { i = random_index(quotes) } else { i -= 1 } \
                \"#\" + string(i + 1) + \": \" + quotes[i]\
                }}".to_string(),
            ),
            Command::new_alias(
                "!quote add".to_string(),
                "!variable edit quotes+ [$text]".to_string(),
            ),
            Command::new_alias(
                "!quote remove".to_string(),
                "!variable edit quotes-# {{int(\"$text\") - 1}}".to_string(),
            ),
            Command::new(
                "!waifu".to_string(),
                "{{\"@$user \" + upload_image(waifu())}}".to_string(),
            ),
        ]
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Timespec")]
pub struct TimespecDef {
    sec: i64,
    nsec: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    pub id: i32,
    #[serde(with = "TimespecDef")]
    pub time_created: Timespec,
    #[serde(with = "TimespecDef")]
    pub time_modified: Timespec,
    pub name: String,
    pub value: VariableValue,
}

impl Variable {
    pub fn new(name: String, value: VariableValue) -> Variable {
        Variable {
            id: 0,
            time_created: time::empty_tm().to_timespec(),
            time_modified: time::empty_tm().to_timespec(),
            name,
            value,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableValue {
    Text(String),
    StringList(Vec<StringItem>),
}

impl Display for VariableValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            VariableValue::Text(value) => {
                f.write_str(value)?;
            }
            VariableValue::StringList(value) => {
                f.write_str(format!("{:?}", value).as_ref())?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringItem {
    #[serde(with = "TimespecDef")]
    pub time_created: Timespec,
    pub value: String,
}

impl StringItem {
    pub fn new(item: &str) -> StringItem {
        StringItem {
            time_created: time::get_time(),
            value: item.to_string(),
        }
    }
}
