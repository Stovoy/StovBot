use crate::twitch::TwitchEvent;
use crossbeam::channel;

#[derive(Debug, Clone)]
pub struct BotEvent {}

pub struct Bot {
    pub username: String,
    pub commands: Vec<Box<dyn Command>>,
    pub bot_event_sender: channel::Sender<BotEvent>,
    pub twitch_writer: twitchchat::Writer,
    pub twitch_event_receiver: channel::Receiver<TwitchEvent>,
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
                }),
            },
            Err(_) => {}
        }
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
        self.twitch_writer.send("stovoy", text).unwrap();
        println!("{}", text);
    }
}

pub struct BotMessage {
    text: String,
}

pub struct Message {
    sender: User,
    text: String,
}

impl Message {
    #[cfg(test)]
    fn new(text: String) -> Message {
        Message {
            sender: User {
                username: "".to_string(),
            },
            text,
        }
    }
}

struct User {
    username: String,
}

pub trait Command {
    fn respond(&self, message: &Message) -> Option<BotMessage>;
}

pub struct BasicCommand {
    pub trigger: String,
    pub response: String,
}

impl Command for BasicCommand {
    fn respond(&self, message: &Message) -> Option<BotMessage> {
        if message.text == self.trigger {
            return Some(BotMessage {
                text: self.response.clone(),
            });
        }

        None
    }
}

#[test]
fn test_basic_command() {
    let response = "test successful!".to_string();
    let command = BasicCommand {
        trigger: "!test".to_string(),
        response: response.clone(),
    };
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
