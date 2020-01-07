use serde::export::fmt::Error;
use serde::export::Formatter;
use serde::{Deserialize, Serialize};
use serenity::model::channel::Message as DiscordMessage;
use serenity::prelude::Context as DiscordContext;
use std::fmt::Debug;
use std::fmt::Display;
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
    DeleteCommand(Command),
    EditCommand(Command),
}

#[derive(Debug)]
pub enum ActionError {
    CommandAlreadyExists,
    CommandDoesNotExist,
    CannotDeleteBuiltInCommand,
    CannotModifyBuiltInCommand,
    BadCommand(String),
    BadCommandTriggerPrefix,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub id: i32,
    pub time_created: Timespec,
    pub trigger: String,
    pub response: String,
    pub actor: Option<Actor>,
}

pub struct Message {
    pub sender: User,
    pub text: String,
    pub source: Source,
}

#[derive(Debug, Clone)]
pub struct User {
    pub username: String,
}

pub enum Source {
    #[cfg(test)]
    None,
    Twitch(TwitchWriter, String),
    Discord(DiscordContext, DiscordMessage),
}

impl Command {
    pub fn new(trigger: String, response: String) -> Command {
        Command::new_with_actor(trigger, response, None)
    }

    pub fn new_with_actor(trigger: String, response: String, actor: Option<Actor>) -> Command {
        Command {
            id: 0,
            time_created: time::empty_tm().to_timespec(),
            trigger,
            response,
            actor,
        }
    }

    pub fn default_commands() -> Vec<Command> {
        vec![Command::new(
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
             }}"
            .to_string(),
        )]
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Timespec")]
struct TimespecDef {
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
    StringList(Vec<ArrayString>),
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
pub struct ArrayString {
    #[serde(with = "TimespecDef")]
    pub time_created: Timespec,
    pub value: String,
}
