use crate::bot::BotEvent;
use crate::discord::DiscordEvent;
use crate::twitch::TwitchEvent;
use crossbeam::channel;
use crossbeam::channel::Receiver;
use db::Database;
use futures::task::{Context, Poll, Waker};
use futures::Stream;
use serde::export::fmt::Error;
use serde::export::Formatter;
use serde::Deserialize;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::thread;

mod bot;
mod command;
mod db;
mod discord;
mod script;
mod twitch;

#[cfg(feature = "gui")]
mod gui;

#[cfg(not(feature = "gui"))]
use futures::executor::block_on;
#[cfg(not(feature = "gui"))]
use futures::stream::{BoxStream, StreamExt};

#[derive(Deserialize, Debug)]
struct Secrets {
    twitch_token: String,
    discord_token: String,
}

fn main() -> Result<(), ConnectError> {
    #[cfg(feature = "gui")]
    gui::run();
    #[cfg(not(feature = "gui"))]
    block_on(run())?;
    Ok(())
}

#[cfg(not(feature = "gui"))]
async fn run() -> Result<(), ConnectError> {
    let connected_state = connect().await?;
    let stream: BoxStream<'static, Event> = Pin::from(Box::from(connected_state.boxed()));
    stream
        .for_each(|event| {
            println!("{:?}", event);
            futures::future::ready(())
        })
        .await;
    Ok(())
}

#[derive(Debug, Clone)]
enum ConnectError {
    FileError,
}

#[derive(Clone)]
struct ConnectedState {
    bot_event_receiver: Receiver<BotEvent>,
    twitch_event_receiver: Receiver<TwitchEvent>,
    discord_event_receiver: Receiver<DiscordEvent>,
    shared_state: Arc<Mutex<SharedState>>,
}

// Can't derive debug on Arc, so implement our own.
impl Debug for ConnectedState {
    fn fmt(&self, _: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}

pub struct SharedState {
    pub waker: Option<Waker>,
}

async fn connect() -> Result<ConnectedState, ConnectError> {
    match Database::new() {
        Ok(_database) => {}
        Err(e) => println!("Database error: {}", e),
    }
    let secrets_file = async_std::fs::read_to_string("secrets.toml")
        .await
        .map_err(|_| ConnectError::FileError)?;

    let secrets: Secrets = toml::from_str(&secrets_file).expect("failed to parse secrets");

    let shared_state = Arc::new(Mutex::new(SharedState { waker: None }));

    let mut twitch_event_senders = Vec::new();
    let mut twitch_event_receivers = Vec::new();
    for _ in 0..2 {
        let (s, r) = channel::bounded(10);
        twitch_event_senders.push(s);
        twitch_event_receivers.push(r);
    }

    let mut discord_event_senders = Vec::new();
    let mut discord_event_receivers = Vec::new();
    for _ in 0..2 {
        let (s, r) = channel::bounded(10);
        discord_event_senders.push(s);
        discord_event_receivers.push(r);
    }

    let (bot_event_sender, bot_event_receiver) = channel::bounded(0);

    let (twitch_client, twitch_writer) = twitch::connect(secrets.twitch_token);
    let thread_shared_state = shared_state.clone();
    thread::spawn(|| {
        let handler = twitch::Handler {
            senders: twitch_event_senders,
            shared_state: thread_shared_state,
        };
        handler.listen(twitch_client);
    });

    let thread_shared_state = shared_state.clone();
    let discord_token = secrets.discord_token;
    thread::spawn(|| {
        let mut discord_client =
            discord::connect(discord_token, discord_event_senders, thread_shared_state);
        if let Err(why) = discord_client.start_autosharded() {
            println!("Discord client error: {:?}", why);
        }
    });

    let bot_twitch_event_receiver = twitch_event_receivers[0].clone();
    let bot_discord_event_receiver = discord_event_receivers[0].clone();

    thread::spawn(|| {
        let mut stovbot = bot::Bot {
            username: "StovBot".to_string(),
            commands: Vec::new(),
            bot_event_sender,
            twitch_event_receiver: bot_twitch_event_receiver,
            discord_event_receiver: bot_discord_event_receiver,
            twitch_writer,
        };
        stovbot.commands.push(command::Command {
            trigger: "!test".to_string(),
            response: "test successful".to_string(),
        });
        stovbot.commands.push(command::Command {
            trigger: "!8ball".to_string(),
            response: "🎱 {{\
            let responses = [\"All signs point to yes...\", \"Yes!\", \"My sources say nope.\", \
             \"You may rely on it.\", \"Concentrate and ask again...\", \
             \"Outlook not so good...\", \"It is decidedly so!\", \
             \"Better not tell you.\", \"Very doubtful.\", \"Yes - Definitely!\", \
             \"It is certain!\", \"Most likely.\", \"Ask again later.\", \"No!\", \
             \"Outlook good.\", \
             \"Don't count on it.\"]; \
              responses[floor(random() * len(responses))]\
            }}".to_string(),
        });
        stovbot.run();
    });

    Ok(ConnectedState {
        bot_event_receiver,
        twitch_event_receiver: twitch_event_receivers[1].clone(),
        discord_event_receiver: discord_event_receivers[1].clone(),
        shared_state,
    })
}

impl Stream for ConnectedState {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut shared_state = self.shared_state.lock().unwrap();
        match self.bot_event_receiver.try_recv() {
            Ok(e) => Poll::Ready(Some(Event::BotEvent(e))),
            Err(_) => match self.twitch_event_receiver.try_recv() {
                Ok(e) => Poll::Ready(Some(Event::TwitchEvent(e))),
                Err(_) => match self.discord_event_receiver.try_recv() {
                    Ok(e) => Poll::Ready(Some(Event::DiscordEvent(e))),
                    Err(_) => {
                        shared_state.waker = Some(cx.waker().clone());
                        Poll::Pending
                    }
                },
            },
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

#[derive(Clone)]
pub enum Event {
    BotEvent(bot::BotEvent),
    TwitchEvent(twitch::TwitchEvent),
    DiscordEvent(discord::DiscordEvent),
}

// Application needs Debug implemented, but we can't implement it on an DiscordEvent.
impl Debug for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(
            match self {
                Event::BotEvent(_e) => format!("bot event"),
                Event::TwitchEvent(e) => match e {
                    TwitchEvent::Ready => "Twitch - Ready".to_string(),
                    TwitchEvent::PrivMsg(msg) => {
                        format!("{}: {}", msg.user(), msg.message()).to_string()
                    }
                },
                Event::DiscordEvent(e) => match e {
                    DiscordEvent::Ready => "Discord - Ready".to_string(),
                    DiscordEvent::Message(_, msg) => {
                        format!("{}: {}", msg.author.name, msg.content).to_string()
                    }
                },
            }
            .as_str(),
        )
    }
}
