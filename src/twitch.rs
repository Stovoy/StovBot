use crate::gui::SharedState;
use crossbeam::channel;
use env_logger;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use twitchchat::*;

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

pub struct Handler {
    pub senders: Vec<channel::Sender<TwitchEvent>>,
    pub shared_state: Arc<Mutex<SharedState>>,
}

impl Handler {
    fn send_event(&self, event: TwitchEvent) {
        self.senders
            .iter()
            .for_each(|s| s.send(event.clone()).unwrap());
        let mut shared_state = self.shared_state.lock().unwrap();
        if let Some(waker) = shared_state.waker.take() {
            waker.wake()
        }
    }

    pub fn listen(&self, client: Client<TcpStream>) {
        for event in client {
            match event {
                Event::TwitchReady(_) => {
                    self.send_event(TwitchEvent::Ready);
                }
                Event::Message(Message::PrivMsg(msg)) => {
                    println!("Private message - {}: {}", msg.user(), msg.message());
                    self.send_event(TwitchEvent::PrivMsg(msg));
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
}
