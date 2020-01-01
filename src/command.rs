use crate::bot::{BotMessage, Message};
use crate::script::ScriptEngine;
use logos::Logos;
use time::Timespec;

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

    #[token = "}}"]
    ScriptEnd,

    #[regex = "."]
    Other,
}

#[derive(Debug)]
pub(crate) struct Command {
    pub(crate) id: i32,
    pub(crate) time_created: Timespec,
    pub(crate) trigger: String,
    pub(crate) response: String,
}

impl Command {
    pub(crate) fn new(trigger: String, response: String) -> Command {
        Command {
            id: 0,
            time_created: time::empty_tm().to_timespec(),
            trigger,
            response,
        }
    }

    pub(crate) fn respond(&self, message: &Message) -> Option<BotMessage> {
        if message.text.starts_with(&self.trigger) {
            let response = self.parse(&message);
            return Some(BotMessage { text: response });
        }

        None
    }

    fn parse(&self, message: &Message) -> String {
        let (_, text) = message.text.split_at(self.trigger.len());
        let mut args = text.split(" ");
        let mut lexer = Token::lexer(self.response.as_str());
        let mut response = "".to_string();
        let mut script = "".to_string();
        let mut in_script = false;
        let mut accumulator = &mut response;
        loop {
            match lexer.token {
                Token::ArgOne => {
                    *accumulator += match args.nth(1) {
                        Some(text) => text,
                        None => "",
                    };
                }
                Token::ArgTwo => {
                    *accumulator += match args.nth(2) {
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
                        let script_result = &ScriptEngine::run(&script);
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

        return response;
    }

    pub(crate) fn default_commands() -> Vec<Command> {
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
        "Hi $user: {{\"message \" + to_string(2 + 2) + \" $user\"}}!".to_string(),
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
         responses[floor(random() * len(responses))]\
         }}"
        .to_string(),
    );
    for _ in 0..10 {
        let response = command
            .respond(&Message::new("!8ball".to_string()))
            .unwrap()
            .text;
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
