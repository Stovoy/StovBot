use crate::Event;
use crossbeam::channel::Sender;
use env_logger;
use futures::task::Waker;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use twitchchat::{commands, Client, Message, UserConfig, Writer};

pub fn connect(token: String) -> Client<TcpStream> {
    env_logger::init().unwrap();

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
    pub sender: Sender<Event>,
    pub stream_waker: Arc<Mutex<Option<Waker>>>,
}

impl Handler {
    fn send_event(&self, event: TwitchEvent) {
        self.sender.send(Event::TwitchEvent(event)).unwrap();
        let mut stream_waker = self.stream_waker.lock().unwrap();
        if let Some(waker) = stream_waker.take() {
            waker.wake()
        }
    }

    pub fn listen(&self, client: Client<TcpStream>) {
        let writer = client.writer();
        for event in client {
            match event {
                twitchchat::Event::TwitchReady(_) => {
                    self.send_event(TwitchEvent::Ready(writer.clone()));
                }
                twitchchat::Event::Message(Message::PrivMsg(msg)) => {
                    self.send_event(TwitchEvent::PrivMsg(writer.clone(), msg));
                }
                twitchchat::Event::Message(Message::Irc(_)) => {}
                twitchchat::Event::Error(err) => {
                    eprintln!("error: {}", err);
                    break;
                }
                _ => unreachable!(),
            }
        }
    }
}
