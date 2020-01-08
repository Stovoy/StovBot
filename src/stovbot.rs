use admin::AdminEvent;
use bot::{Bot, BotEvent, SharedState};
use crossbeam::channel;
use crossbeam::channel::Receiver;
#[cfg(feature = "discord")]
use discord::DiscordEvent;
use futures::task::{Context, Poll};
use futures::Stream;
use serde::export::fmt::Error;
use serde::export::Formatter;
use serde::Deserialize;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::thread;
#[cfg(feature = "twitch")]
use twitch::TwitchEvent;

mod admin;
mod bot;
mod command;
pub mod database;
#[cfg(feature = "discord")]
mod discord;
pub mod models;
mod script_runner;
mod special_command;
#[cfg(feature = "twitch")]
mod twitch;

#[cfg(feature = "gui")]
mod gui;

use crossbeam::channel::TryRecvError;
#[cfg(not(feature = "gui"))]
use futures::executor::block_on;
#[cfg(not(feature = "gui"))]
use futures::stream::{BoxStream, StreamExt};

#[derive(Deserialize, Debug, Clone)]
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
    println!("Connecting...");
    let connected_state = connect().await?;
    println!("Connected");
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
    #[cfg(any(feature = "twitch", feature = "discord"))]
    FileError,
    #[cfg(any(feature = "twitch", feature = "discord"))]
    TomlError,
}

#[derive(Clone)]
struct ConnectedState {
    bot_event_receiver: Receiver<BotEvent>,
    #[cfg(feature = "twitch")]
    twitch_event_receiver: Receiver<TwitchEvent>,
    #[cfg(feature = "discord")]
    discord_event_receiver: Receiver<DiscordEvent>,
    admin_event_receiver: Receiver<AdminEvent>,
    shared_state: Arc<Mutex<SharedState>>,
}

// Can't derive debug on Arc, so implement our own.
impl Debug for ConnectedState {
    fn fmt(&self, _: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(any(feature = "twitch", feature = "discord"))]
async fn load_secrets() -> Result<Secrets, ConnectError> {
    let secrets_file = async_std::fs::read_to_string("./.stovbot/secrets.toml")
        .await
        .map_err(|_| ConnectError::FileError)?;

    let secrets = toml::from_str(&secrets_file).map_err(|_| ConnectError::TomlError)?;
    Ok(secrets)
}

const CHANNEL_SIZE: usize = 1024;

async fn connect() -> Result<ConnectedState, ConnectError> {
    let shared_state = Arc::new(Mutex::new(SharedState { waker: None }));

    #[cfg(any(feature = "twitch", feature = "discord"))]
    let secrets = load_secrets().await?;
    #[cfg(feature = "twitch")]
    let twitch_event_receivers = connect_twitch_thread(shared_state.clone(), secrets.clone());
    #[cfg(feature = "twitch")]
    let bot_twitch_event_receiver = twitch_event_receivers[0].clone();

    #[cfg(feature = "discord")]
    let discord_event_receivers = connect_discord_thread(shared_state.clone(), secrets.clone());
    #[cfg(feature = "discord")]
    let bot_discord_event_receiver = discord_event_receivers[0].clone();

    let admin_event_receivers = connect_admin_cli_thread(shared_state.clone());
    let bot_admin_event_receiver = admin_event_receivers[0].clone();

    let (bot_event_sender, bot_event_receiver) = channel::bounded(CHANNEL_SIZE);

    let thread_shared_state = shared_state.clone();

    thread::spawn(|| {
        match Bot::new(
            bot_event_sender,
            #[cfg(feature = "twitch")]
            bot_twitch_event_receiver,
            #[cfg(feature = "discord")]
            bot_discord_event_receiver,
            bot_admin_event_receiver,
            thread_shared_state,
        ) {
            Ok(mut stovbot) => {
                stovbot.run();
            }
            Err(e) => {
                println!("Error running bot: {}", e);
            }
        }
    });

    Ok(ConnectedState {
        bot_event_receiver,
        #[cfg(feature = "twitch")]
        twitch_event_receiver: twitch_event_receivers[1].clone(),
        #[cfg(feature = "discord")]
        discord_event_receiver: discord_event_receivers[1].clone(),
        admin_event_receiver: admin_event_receivers[1].clone(),

        shared_state,
    })
}

#[cfg(feature = "twitch")]
fn connect_twitch_thread(
    shared_state: Arc<Mutex<SharedState>>,
    secrets: Secrets,
) -> Vec<Receiver<TwitchEvent>> {
    let mut twitch_event_senders = Vec::new();
    let mut twitch_event_receivers = Vec::new();
    for _ in 0..2 {
        let (s, r) = channel::bounded(CHANNEL_SIZE);
        twitch_event_senders.push(s);
        twitch_event_receivers.push(r);
    }

    let thread_shared_state = shared_state.clone();
    let twitch_token = secrets.twitch_token;
    thread::spawn(|| {
        let client = twitch::connect(twitch_token);
        let handler = twitch::Handler {
            senders: twitch_event_senders,
            shared_state: thread_shared_state,
        };
        handler.listen(client);
    });

    twitch_event_receivers
}

#[cfg(feature = "discord")]
fn connect_discord_thread(
    shared_state: Arc<Mutex<SharedState>>,
    secrets: Secrets,
) -> Vec<Receiver<DiscordEvent>> {
    let mut discord_event_senders = Vec::new();
    let mut discord_event_receivers = Vec::new();
    for _ in 0..2 {
        let (s, r) = channel::bounded(CHANNEL_SIZE);
        discord_event_senders.push(s);
        discord_event_receivers.push(r);
    }

    let thread_shared_state = shared_state.clone();
    let discord_token = secrets.discord_token;
    thread::spawn(|| {
        let mut discord_client =
            discord::connect(discord_token, discord_event_senders, thread_shared_state);
        if let Err(why) = discord_client.start_autosharded() {
            println!("Discord client error: {:?}", why);
        }
    });

    discord_event_receivers
}

fn connect_admin_cli_thread(shared_state: Arc<Mutex<SharedState>>) -> Vec<Receiver<AdminEvent>> {
    let mut admin_event_senders = Vec::new();
    let mut admin_event_receivers = Vec::new();
    for _ in 0..2 {
        let (s, r) = channel::bounded(CHANNEL_SIZE);
        admin_event_senders.push(s);
        admin_event_receivers.push(r);
    }

    let thread_shared_state = shared_state.clone();
    thread::spawn(|| {
        admin::cli_run(admin_event_senders, thread_shared_state);
    });

    admin_event_receivers
}

impl ConnectedState {
    fn bot_receiver(&self) -> Result<Poll<Option<Event>>, TryRecvError> {
        match self.bot_event_receiver.try_recv() {
            Ok(e) => Ok(Poll::Ready(Some(Event::BotEvent(e)))),
            Err(e) => Err(e),
        }
    }

    #[cfg(feature = "twitch")]
    fn twitch_receiver(&self) -> Result<Poll<Option<Event>>, TryRecvError> {
        match self.twitch_event_receiver.try_recv() {
            Ok(e) => Ok(Poll::Ready(Some(Event::TwitchEvent(e)))),
            Err(e) => Err(e),
        }
    }

    #[cfg(feature = "discord")]
    fn discord_receiver(&self) -> Result<Poll<Option<Event>>, TryRecvError> {
        match self.discord_event_receiver.try_recv() {
            Ok(e) => Ok(Poll::Ready(Some(Event::DiscordEvent(e)))),
            Err(e) => Err(e),
        }
    }

    fn admin_receiver(&self) -> Result<Poll<Option<Event>>, TryRecvError> {
        match self.admin_event_receiver.try_recv() {
            Ok(e) => Ok(Poll::Ready(Some(Event::AdminEvent(e)))),
            Err(e) => Err(e),
        }
    }
}

impl Stream for ConnectedState {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut shared_state = self.shared_state.lock().unwrap();
        let receivers = [
            ConnectedState::bot_receiver,
            #[cfg(feature = "twitch")]
            ConnectedState::twitch_receiver,
            #[cfg(feature = "discord")]
            ConnectedState::discord_receiver,
            ConnectedState::admin_receiver,
        ];

        for receiver in receivers.iter() {
            match receiver(&self) {
                Ok(poll) => return poll,
                Err(_) => {}
            };
        }

        shared_state.waker = Some(cx.waker().clone());
        Poll::Pending
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

#[derive(Clone)]
pub enum Event {
    BotEvent(bot::BotEvent),
    #[cfg(feature = "twitch")]
    TwitchEvent(twitch::TwitchEvent),
    #[cfg(feature = "discord")]
    DiscordEvent(discord::DiscordEvent),
    AdminEvent(admin::AdminEvent),
}

// Application needs Debug implemented, but we can't implement it on an DiscordEvent.
impl Debug for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(
            match self {
                Event::BotEvent(e) => match e {
                    BotEvent::LoadCommand(command) => {
                        format!("Load Command: {}: {}", command.trigger, command.response)
                    }
                    BotEvent::AddCommand(command, user) => format!(
                        "Add Command by {}: {}: {}",
                        user.username, command.trigger, command.response
                    ),
                    BotEvent::EditCommand(command, user) => format!(
                        "Edit Command by {}: {}: {}",
                        user.username, command.trigger, command.response
                    ),
                    BotEvent::DeleteCommand(command, user) => format!(
                        "Delete Command by {}: {}",
                        user.username, command.trigger
                    ),
                    BotEvent::LoadVariable(variable) => {
                        format!("Load Variable: {}: {}", variable.name, variable.value)
                    }
                    BotEvent::AddVariable(variable, user) => format!(
                        "Add Variable by {}: {}: {}",
                        user.username, variable.name, variable.value
                    ),
                    BotEvent::EditVariable(variable, user) => format!(
                        "Edit Variable by {}: {}: {}",
                        user.username, variable.name, variable.value
                    ),
                    BotEvent::DeleteVariable(variable, user) => format!(
                        "Delete Variable by {}: {}",
                        user.username, variable.name
                    ),
                },
                #[cfg(feature = "twitch")]
                Event::TwitchEvent(e) => match e {
                    TwitchEvent::Ready(_) => "Twitch - Ready".to_string(),
                    TwitchEvent::PrivMsg(_, msg) => {
                        format!("{}: {}", msg.user(), msg.message()).to_string()
                    }
                },
                #[cfg(feature = "discord")]
                Event::DiscordEvent(e) => match e {
                    DiscordEvent::Ready => "Discord - Ready".to_string(),
                    DiscordEvent::Message(_, msg) => {
                        format!("{}: {}", msg.author.name, msg.content).to_string()
                    }
                },
                Event::AdminEvent(e) => match e {
                    AdminEvent::Message(msg) => msg.clone(),
                },
            }
            .as_str(),
        )
    }
}
