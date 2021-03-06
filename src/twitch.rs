use crate::{Event, EventBusSender};
use std::net::TcpStream;
use twitchchat::{commands, Client, Message, UserConfig, Writer};

pub fn connect(token: String) -> Client<TcpStream> {
    twitchchat::connect(
        UserConfig::builder()
            .membership()
            .commands()
            .tags()
            .nick("StovBot")
            .token(token)
            .build()
            .expect("error creating UserConfig"),
    )
    .expect("failed to connect to twitch")
    .filter::<commands::PrivMsg>()
}

#[derive(Clone)]
pub enum TwitchEvent {
    Ready(Writer),
    PrivMsg(Writer, commands::PrivMsg),
}

pub struct Handler {
    pub sender: EventBusSender,
}

impl Handler {
    pub fn listen(&self, client: Client<TcpStream>) {
        let writer = client.writer();
        for event in client {
            match event {
                twitchchat::Event::TwitchReady(_) => {
                    self.sender
                        .send(Event::TwitchEvent(TwitchEvent::Ready(writer.clone())));
                }
                twitchchat::Event::Message(Message::PrivMsg(msg)) => {
                    self.sender.send(Event::TwitchEvent(TwitchEvent::PrivMsg(
                        writer.clone(),
                        msg,
                    )));
                }
                twitchchat::Event::Message(Message::Irc(_)) => {}
                twitchchat::Event::Error(err) => {
                    eprintln!("Twitch error: {}", err);
                    break;
                }
                _ => unreachable!(),
            }
        }
    }
}
