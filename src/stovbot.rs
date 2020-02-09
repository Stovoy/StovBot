#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

#[macro_use]
extern crate clap;

use admin::AdminEvent;
use bot::{Bot, BotEvent};
use clap::{App, Arg, ArgMatches};
use crossbeam::channel::{bounded, Receiver, Sender};
use discord::DiscordEvent;
use futures::executor::block_on;
use futures::stream::{BoxStream, StreamExt};
use futures::task::{Context, Poll, Waker};
use futures::Stream;
use serde::export::fmt::Error;
use serde::export::Formatter;
use serde::Deserialize;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::thread;
use twitch::TwitchEvent;

mod admin;
mod background;
mod bot;
mod client;
mod command;
pub mod database;
mod discord;
mod gui;
pub mod models;
mod script_runner;
mod server;
mod special_command;
mod twitch;

#[derive(Deserialize, Debug, Clone)]
struct Secrets {
    twitch_token: String,
    twitch_client_id: String,
    discord_token: String,
}

fn parse_args<'a>() -> ArgMatches<'a> {
    App::new("StovBot")
        .version(&crate_version!()[..])
        .author("Steve Mostovoy <stevemostovoysm@gmail.com>")
        .about("Does awesome things")
        .arg(
            Arg::with_name("gui")
                .long("gui")
                .help("Runs the gui")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("server")
                .long("server")
                .help("Runs the server")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("cli")
                .long("cli")
                .help("Runs the cli")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("client")
                .long("client")
                .help("Runs the client which will talk to a server")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("twitch")
                .long("twitch")
                .help("Connects to twitch")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("discord")
                .long("discord")
                .help("Connects to discord")
                .takes_value(false),
        )
        .get_matches()
}

fn main() -> Result<(), ConnectError> {
    let args = parse_args();

    if args.is_present("gui") {
        gui::run();
    } else {
        block_on(run())?;
    }

    Ok(())
}

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
    TomlError,
}

#[derive(Clone)]
struct ConnectedState {
    event_rx: Arc<Mutex<Receiver<Event>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

// Can't derive debug on Arc, so implement our own.
impl Debug for ConnectedState {
    fn fmt(&self, _: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}

async fn load_secrets() -> Result<Secrets, ConnectError> {
    let secrets_file = async_std::fs::read_to_string("./.stovbot/secrets.toml")
        .await
        .map_err(|_| ConnectError::FileError)?;

    let secrets = toml::from_str(&secrets_file).map_err(|_| ConnectError::TomlError)?;
    Ok(secrets)
}

const CHANNEL_SIZE: usize = 1024;

pub struct EventBus {
    txs: Arc<Mutex<Vec<Sender<Event>>>>,
}

#[derive(Clone)]
pub struct EventBusSender {
    tx: Sender<Event>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl EventBus {
    fn new() -> (EventBus, EventBusSender) {
        let (tx_bus, rx_bus) = bounded::<Event>(CHANNEL_SIZE);

        let txs = Arc::new(Mutex::new(Vec::<Sender<Event>>::new()));
        let txs_clone = txs.clone();
        // Bus dispatcher thread.
        thread::spawn(move || {
            for event in rx_bus.iter() {
                for tx in txs_clone.lock().unwrap().iter() {
                    match tx.send(event.clone()) {
                        Ok(_) => {}
                        // Don't crash if the rx failed.
                        Err(_) => {}
                    }
                }
            }
        });

        (
            EventBus { txs },
            EventBusSender {
                tx: tx_bus,
                waker: Arc::new(Mutex::new(None)),
            },
        )
    }

    pub fn add_rx(&self) -> Receiver<Event> {
        let (tx, rx) = bounded(CHANNEL_SIZE);
        self.txs.lock().unwrap().push(tx);
        rx
    }
}

impl EventBusSender {
    pub fn send(&self, event: Event) {
        self.tx.send(event).unwrap();
        let mut waker = self.waker.lock().unwrap();
        if let Some(waker) = waker.take() {
            waker.wake()
        }
    }
}

async fn connect() -> Result<ConnectedState, ConnectError> {
    let args = parse_args();

    let (event_bus, event_sender) = EventBus::new();

    let secrets = load_secrets().await?;

    if args.is_present("cli") {
        connect_admin_cli_thread(event_sender.clone());
    }

    if args.is_present("client") {
        connect_client_thread(secrets.clone(), event_sender.clone(), event_bus.add_rx());
    } else {
        if args.is_present("twitch") {
            connect_twitch_thread(secrets.clone(), event_sender.clone());
        }

        if args.is_present("discord") {
            connect_discord_thread(secrets.clone(), event_sender.clone());
        }

        if args.is_present("server") {
            connect_server_thread(event_sender.clone(), event_bus.add_rx());
        }

        connect_background_thread(secrets, event_bus.add_rx());

        connect_bot_thread(event_sender.clone(), event_bus.add_rx());
    }

    Ok(ConnectedState {
        event_rx: Arc::new(Mutex::new(event_bus.add_rx())),
        waker: event_sender.waker.clone(),
    })
}

fn connect_twitch_thread(secrets: Secrets, sender: EventBusSender) {
    let twitch_token = secrets.twitch_token;
    thread::spawn(|| {
        let client = twitch::connect(twitch_token);
        let handler = twitch::Handler { sender };
        handler.listen(client);
    });
}

fn connect_discord_thread(secrets: Secrets, sender: EventBusSender) {
    let discord_token = secrets.discord_token;
    thread::spawn(|| {
        let mut discord_client = discord::connect(discord_token, sender);
        if let Err(why) = discord_client.start_autosharded() {
            println!("Discord client error: {:?}", why);
        }
    });
}

fn connect_admin_cli_thread(sender: EventBusSender) {
    thread::spawn(|| {
        admin::cli_run(sender);
    });
}

fn connect_background_thread(secrets: Secrets, event_rx: Receiver<Event>) {
    let twitch_client_id = secrets.twitch_client_id;
    thread::spawn(|| {
        background::run(twitch_client_id, event_rx);
    });
}

fn connect_bot_thread(sender: EventBusSender, event_rx: Receiver<Event>) {
    thread::spawn(|| match Bot::new(sender, event_rx) {
        Ok(mut stovbot) => {
            stovbot.run();
        }
        Err(e) => {
            println!("Error running bot: {}", e);
        }
    });
}

fn connect_server_thread(sender: EventBusSender, event_rx: Receiver<Event>) {
    thread::spawn(|| server::run(sender, event_rx));
}

fn connect_client_thread(secrets: Secrets, sender: EventBusSender, event_rx: Receiver<Event>) {
    thread::spawn(|| client::run(secrets.twitch_token, sender, event_rx));
}

impl Stream for ConnectedState {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.event_rx.lock().unwrap().try_recv() {
            Ok(event) => Poll::Ready(Some(event)),
            Err(_) => {
                let mut waker = self.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
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
                    BotEvent::DeleteCommand(command, user) => {
                        format!("Delete Command by {}: {}", user.username, command.trigger)
                    }
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
                    BotEvent::DeleteVariable(variable, user) => {
                        format!("Delete Variable by {}: {}", user.username, variable.name)
                    }
                },
                Event::TwitchEvent(e) => match e {
                    TwitchEvent::Ready(_) => "Twitch - Ready".to_string(),
                    TwitchEvent::PrivMsg(_, msg) => format!("{}: {}", msg.user(), msg.message()),
                },
                Event::DiscordEvent(e) => match e {
                    DiscordEvent::Ready(_, _) => "Discord - Ready".to_string(),
                    DiscordEvent::Message(_, msg) => {
                        format!("{}: {}", msg.author.name, msg.content)
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
