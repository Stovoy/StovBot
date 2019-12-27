use std::thread::sleep;
use std::{time, io};
use std::fs;
use serde::{Deserialize};

mod twitch;

#[derive(Deserialize, Debug)]
struct Secrets {
    token: String,
}


fn main() {
    let mut stov_bot = StovBot {
        username: "StovBot".to_string(),
        commands: Vec::new(),
    };
    stov_bot.commands.push(Box::from(BasicCommand {
        trigger: "!test".to_string(),
        response: "test successful".to_string(),
    }));

    let secrets_file = fs::read_to_string("secrets.toml").expect("failed to read secrets");

    let secrets: Secrets = toml::from_str(&secrets_file).expect("failed to parse secrets");
    twitch::connect(secrets.token);

    loop {
        sleep(time::Duration::from_millis(10));

        let mut buffer = String::new();
        // TODO: How to read line without blocking?
        let input = io::stdin().read_line(&mut buffer).unwrap();
        let mut messages = Vec::new();
        let buffer = buffer.trim();
        messages.push(Message {
            sender: User { username: "Stovoy".to_string() },
            text: buffer.to_string(),
        });

        stov_bot.process_messages(messages);
    }
}

struct StovBot {
    username: String,
    commands: Vec<Box<dyn Command>>,
}

impl StovBot {
    fn process_messages(&mut self, messages: Vec<Message>) {
        for message in messages.iter() {
            self.debug_message(&format!("{}: {}", message.sender.username, message.text));
            let responses = self.respond(message);
            for response in responses.iter() {
                self.send_message(&response.text);
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

    fn send_message(&self, text: &String) {
        println!("{}", text);
    }
}

struct BotMessage {
    text: String,
}

struct Message {
    sender: User,
    text: String,
}

impl Message {
    fn new(text: String) -> Message {
        Message { sender: User { username: "".to_string() }, text }
    }
}

struct User {
    username: String,
}

trait Command {
    fn respond(&self, message: &Message) -> Option<BotMessage>;
}

struct BasicCommand {
    trigger: String,
    response: String,
}

impl Command for BasicCommand {
    fn respond(&self, message: &Message) -> Option<BotMessage> {
        if message.text == self.trigger {
            return Some(BotMessage { text: self.response.clone() });
        }

        None
    }
}

#[test]
fn test_basic_command() {
    let response = "test successful!".to_string();
    let command = BasicCommand { trigger: "!test".to_string(), response: response.clone() };
    assert_eq!(response, command.respond(&Message::new("!test".to_string())).unwrap().text);
    assert!(command.respond(&Message::new("random text".to_string())).is_none());
}
