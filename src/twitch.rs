use crossbeam::channel;
use env_logger;
use std::net::TcpStream;
use twitchchat::*;

use crate::gui::SharedState;
use std::sync::{Arc, Mutex};

pub fn connect(token: String) -> (Client<TcpStream>, Writer) {
    env_logger::init().unwrap();

    let client = twitchchat::connect(
        twitchchat::UserConfig::builder()
            .membership()
            .commands()
            .tags()
            .nick("StovBot")
            .token(token)
            .build()
            .expect("error creating UserConfig"),
    )
    .expect("failed to connect to twitch")
    .filter::<commands::PrivMsg>();

    let writer = client.writer();
    (client, writer)
}

#[derive(Debug, Clone)]
pub enum TwitchEvent {
    Ready,
    PrivMsg(commands::PrivMsg),
}

pub fn listen(
    client: Client<TcpStream>,
    senders: Vec<channel::Sender<TwitchEvent>>,
    shared_state: Arc<Mutex<SharedState>>,
) {
    for event in client {
        match event {
            Event::TwitchReady(_) => {
                senders
                    .iter()
                    .for_each(|s| s.send(TwitchEvent::Ready).unwrap());
                let mut shared_state = shared_state.lock().unwrap();
                if let Some(waker) = shared_state.waker.take() {
                    waker.wake()
                }
            }
            Event::Message(Message::PrivMsg(msg)) => {
                println!("Private message - {}: {}", msg.user(), msg.message());
                senders
                    .iter()
                    .for_each(|s| s.send(TwitchEvent::PrivMsg(msg.clone())).unwrap());

                let mut shared_state = shared_state.lock().unwrap();
                if let Some(waker) = shared_state.waker.take() {
                    waker.wake()
                }
            }
            Event::Message(Message::Irc(_)) => {}
            Event::Error(err) => {
                eprintln!("error: {}", err);
                break;
            }
            _ => unreachable!(),
        }
    }
}
