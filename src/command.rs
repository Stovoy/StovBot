use crate::bot::BotMessage;
use crate::models::{Command, Message};
use crate::script_runner;
use logos::Logos;
use std::collections::hash_map::Values;
use std::collections::HashMap;

#[cfg(test)]
use crate::database;
#[cfg(test)]
use crate::models::{StringItem, Variable, VariableValue};

#[derive(Logos, Debug, PartialEq)]
enum Token {
    #[end]
    End,

    #[error]
    Error,

    #[token = "$1"]
    ArgOne,

    #[token = "$2"]
    ArgTwo,

    #[token = "$user"]
    User,

    #[token = "$year"]
    Year,

    #[token = "{{"]
    ScriptStart,

    #[token = "}}}"]
    ScriptEndAndExtra,

    #[token = "}}"]
    ScriptEnd,

    #[regex = "."]
    Other,
}

pub trait CommandExt {
    fn matches_trigger(&self, message: &Message) -> bool;
    #[cfg(test)]
    fn respond(&self, message: &Message) -> Option<BotMessage>;
    fn respond_no_check(&self, message: &Message) -> BotMessage;
    fn parse(&self, message: &Message) -> String;
}

impl CommandExt for Command {
    fn matches_trigger(&self, message: &Message) -> bool {
        message.text == self.trigger || message.text.starts_with(&format!("{} ", self.trigger))
    }

    #[cfg(test)]
    fn respond(&self, message: &Message) -> Option<BotMessage> {
        match self.matches_trigger(message) {
            true => Some(self.respond_no_check(message)),
            false => None,
        }
    }

    fn respond_no_check(&self, message: &Message) -> BotMessage {
        let response = self.parse(&message);
        BotMessage { text: response }
    }

    fn parse(&self, message: &Message) -> String {
        let text = message.after_trigger(&self.trigger);
        let mut args = text.split(' ');
        let mut lexer = Token::lexer(self.response.as_str());
        let mut response = "".to_string();
        let mut script = "".to_string();
        let mut in_script = false;
        let mut accumulator = &mut response;
        loop {
            match lexer.token {
                Token::ArgOne => {
                    *accumulator += match args.nth(0) {
                        Some(text) => text,
                        None => "",
                    };
                }
                Token::ArgTwo => {
                    *accumulator += match args.nth(1) {
                        Some(text) => text,
                        None => "",
                    };
                }
                Token::User => {
                    *accumulator += &message.sender.username;
                }
                Token::Year => {
                    *accumulator += "YEAR";
                }
                Token::ScriptStart => {
                    if in_script {
                        *accumulator += lexer.slice();
                    } else {
                        script = "".to_string();
                        accumulator = &mut script;
                        in_script = true;
                    }
                }
                Token::ScriptEnd => {
                    if in_script {
                        let script_result = &script_runner::run(&script, &self.database_path);
                        accumulator = &mut response;
                        *accumulator += script_result;
                        in_script = false;
                    } else {
                        *accumulator += lexer.slice();
                    }
                }
                Token::ScriptEndAndExtra => {
                    if in_script {
                        *accumulator += "}";
                        let script_result = &script_runner::run(&script, &self.database_path);
                        accumulator = &mut response;
                        *accumulator += script_result;
                        in_script = false;
                    } else {
                        *accumulator += lexer.slice();
                    }
                }
                Token::Other => {
                    *accumulator += lexer.slice();
                }
                Token::Error => {
                    println!("Lexer error: {}", lexer.slice());
                }
                Token::End => {
                    break;
                }
            }
            lexer.advance();
        }

        if in_script {
            // Script was never ended.
            accumulator = &mut response;
            *accumulator += &script;
        }

        response
    }
}

pub struct Commands {
    commands: HashMap<String, Command>,
}

impl Commands {
    pub fn new(commands: Vec<Command>) -> Commands {
        let mut commands_map = HashMap::new();
        for command in commands {
            commands_map.insert(command.trigger.clone(), command);
        }
        Commands {
            commands: commands_map,
        }
    }

    pub fn iter(&self) -> Values<'_, String, Command> {
        self.commands.values()
    }

    pub fn contains(&self, command: &Command) -> bool {
        self.commands.contains_key(&command.trigger)
    }

    pub fn update_command(&mut self, command: &Command) {
        self.commands
            .insert(command.trigger.clone(), command.clone());
    }

    pub fn delete_command(&mut self, command: &Command) {
        self.commands.remove(&command.trigger);
    }
}

#[test]
fn test_basic_command() {
    let response = "test successful!".to_string();
    let command = Command::new("!test".to_string(), response.clone());
    assert_eq!(
        response,
        command
            .respond(&Message::new("!test".to_string()))
            .unwrap()
            .text
    );
    assert!(command
        .respond(&Message::new("random text".to_string()))
        .is_none());
}

#[test]
fn test_args_command() {
    let command = Command::new("!hi".to_string(), "Hi $1!".to_string());
    assert_eq!(
        "Hi !".to_string(),
        command
            .respond(&Message::new("!hi".to_string()))
            .unwrap()
            .text
    );
    assert_eq!(
        "Hi foo!".to_string(),
        command
            .respond(&Message::new("!hi foo".to_string()))
            .unwrap()
            .text
    );
    assert_eq!(
        "Hi foo!".to_string(),
        command
            .respond(&Message::new("!hi foo bar".to_string()))
            .unwrap()
            .text
    );
}

#[test]
fn test_simple_script_command() {
    let command = Command::new(
        "!script".to_string(),
        "Hi $user - 2 + 2 is {{2 + 2}}!".to_string(),
    );
    assert_eq!(
        "Hi foo - 2 + 2 is 4!".to_string(),
        command
            .respond(&Message::new("!script".to_string()))
            .unwrap()
            .text
    );
}

#[test]
fn test_complex_script_command() {
    let command = Command::new(
        "!script".to_string(),
        "Hi $user: {{\"message \" + string(2 + 2) + \" $user\"}}!".to_string(),
    );
    assert_eq!(
        "Hi foo: message 4 foo!".to_string(),
        command
            .respond(&Message::new("!script".to_string()))
            .unwrap()
            .text
    );
}

#[test]
fn test_8ball() {
    let responses = [
        "All signs point to yes...",
        "Yes!",
        "My sources say nope.",
        "You may rely on it.",
        "Concentrate and ask again...",
        "Outlook not so good...",
        "It is decidedly so!",
        "Better not tell you.",
        "Very doubtful.",
        "Yes - Definitely!",
        "It is certain!",
        "Most likely.",
        "Ask again later.",
        "No!",
        "Outlook good.",
        "Don't count on it.",
    ];
    let command = Command::new(
        "!8ball".to_string(),
        "🎱 {{\
         let responses = [\"All signs point to yes...\", \"Yes!\", \"My sources say nope.\", \
         \"You may rely on it.\", \"Concentrate and ask again...\", \
         \"Outlook not so good...\", \"It is decidedly so!\", \
         \"Better not tell you.\", \"Very doubtful.\", \"Yes - Definitely!\", \
         \"It is certain!\", \"Most likely.\", \"Ask again later.\", \"No!\", \
         \"Outlook good.\", \
         \"Don't count on it.\"]; \
         responses[random_index(responses)]\
         }}"
        .to_string(),
    );
    for _ in 0..10 {
        let response = command
            .respond(&Message::new("!8ball".to_string()))
            .unwrap()
            .text;
        println!("{}", response);
        let mut found = false;
        for accepted_response in responses.iter() {
            if response.ends_with(accepted_response) {
                found = true;
                break;
            }
        }
        assert!(found);
    }
}

#[test]
fn test_infinite_loop() {
    let command = Command::new("!loop".to_string(), "{{loop{}}}".to_string());
    let response = command
        .respond(&Message::new("!loop".to_string()))
        .unwrap()
        .text;
    assert!(response.contains("Timeout"));
}

#[test]
fn test_d6() {
    let command = Command::new(
        "!d6".to_string(),
        "{{let n = int(\"$1\"); if n == 0 { n = 1 } let i = 0; let d = 0; while i < n { i += 1; d += floor(random() * 6) + 1 } d}}".to_string(),
    );
    let response = command
        .respond(&Message::new("!d6".to_string()))
        .unwrap()
        .text;
    let n: i64 = response.parse().unwrap();
    assert!(n >= 1 && n <= 6);
}

#[test]
fn test_coinflip() {
    let command = Command::new(
        "!coinflip".to_string(),
        "{{if random() > 0.5 { \"Heads!\" } else { \"Tails!\" }}}".to_string(),
    );
    let response = command
        .respond(&Message::new("!coinflip".to_string()))
        .unwrap()
        .text;
    assert!(response == "Heads!" || response == "Tails!");
}

#[test]
fn test_counter() -> Result<(), rusqlite::Error> {
    database::with_test_db(|connection| {
        connection.set_variable(&Variable::new(
            "count".to_string(),
            VariableValue::Text("0".to_string()),
        ))?;
        let command = Command::new(
            "!count".to_string(),
            "{{let count = get(\"count\"); count += 1; set(\"count\", count); count}}".to_string(),
        )
        .with_database_path(connection.path)
        .build();
        let response = command
            .respond(&Message::new("!count".to_string()))
            .unwrap()
            .text;
        assert_eq!(response, "1");
        let response = command
            .respond(&Message::new("!count".to_string()))
            .unwrap()
            .text;
        assert_eq!(response, "2");
        Ok(())
    })
}

#[test]
fn test_quotes() -> Result<(), rusqlite::Error> {
    database::with_test_db(|connection| {
        let responses = ["hello", "hi", "howdy"];
        connection.set_variable(&Variable::new(
            "quotes".to_string(),
            VariableValue::StringList(
                responses
                    .iter()
                    .map(|response| StringItem::new(response))
                    .collect(),
            ),
        ))?;
        // TODO: Should array modification commands be a special set of command?
        // We want things like !quote add, !quote remove, !quote N
        // Can we do it in such a way that it's not only for quotes, and doesn't require duplication?
        let command = Command::new(
            "!quote".to_string(),
            "{{let quotes = get_list(\"quotes\"); let i = int(\"$1\"); if i == 0 { i = random_index(quotes) } else { i -= 1 } quotes[i]}}".to_string(),
        ).with_database_path(connection.path).build();
        for _ in 0..10 {
            let response = command
                .respond(&Message::new("!quote".to_string()))
                .unwrap()
                .text;
            println!("{}", response);
            let mut found = false;
            for accepted_response in &responses {
                if response == **accepted_response {
                    found = true;
                    break;
                }
            }
            assert!(found);
        }
        for _ in 0..10 {
            let response = command
                .respond(&Message::new("!quote 2".to_string()))
                .unwrap()
                .text;
            assert_eq!(response, responses[1]);
        }
        Ok(())
    })
}
