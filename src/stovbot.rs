use admin::AdminEvent;
use bot::{Bot, BotEvent};
use clap::Parser;
use crossbeam::channel::{bounded, Receiver, Sender};
use discord::DiscordEvent;
use futures::stream::{BoxStream, StreamExt};
use futures::task::{Context, Poll, Waker};
use futures::Stream;
use serde::Deserialize;
use std::fmt::{Debug, Error, Formatter};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::thread;
use twitch::TwitchEvent;

mod admin;
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
pub mod waifu;

#[derive(Deserialize, Debug, Clone)]
struct Secrets {
    twitch_token: String,
    discord_token: String,
}

#[derive(Parser, Debug, Clone)]
#[command(name = "StovBot", about = "Does awesome things", version)]
struct Args {
    #[arg(long)]
    gui: bool,
    #[arg(long)]
    server: bool,
    #[arg(long)]
    cli: bool,
    #[arg(long)]
    client: bool,
    #[arg(long)]
    twitch: bool,
    #[arg(long)]
    discord: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            gui: false,
            server: false,
            cli: false,
            client: false,
            twitch: false,
            discord: false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), ConnectError> {
    env_logger::init();

    let args = Args::parse();

    if args.gui {
        gui::run();
    } else {
        run(&args).await?;
    }

    Ok(())
}

async fn run(args: &Args) -> Result<(), ConnectError> {
    let connected_state = connect(args.clone()).await?;
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
    let secrets_file = tokio::fs::read_to_string("./.stovbot/secrets.toml")
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

async fn connect(args: Args) -> Result<ConnectedState, ConnectError> {

    let (event_bus, event_sender) = EventBus::new();

    let secrets = load_secrets().await?;

    if args.cli {
        connect_admin_cli_thread(event_sender.clone());
    }

    if args.client {
        connect_client_thread(secrets.clone(), event_sender.clone(), event_bus.add_rx());
    } else {
        if args.twitch {
            connect_twitch_thread(secrets.clone(), event_sender.clone());
        }

        if args.discord {
            connect_discord_thread(secrets.clone(), event_sender.clone());
        }

        if args.server {
            connect_server_thread(event_sender.clone(), event_bus.add_rx());
        }

        connect_bot_thread(event_sender.clone(), event_bus.add_rx());
    }

    Ok(ConnectedState {
        event_rx: Arc::new(Mutex::new(event_bus.add_rx())),
        waker: event_sender.waker.clone(),
    })
}

fn connect_twitch_thread(secrets: Secrets, sender: EventBusSender) {
    let twitch_token = secrets.twitch_token;
    thread::spawn(move || {
        let mut rt = tokio02::runtime::Runtime::new().expect("Failed to create tokio 0.2 runtime");
        rt.block_on(async move {
            match twitch::connect(twitch_token).await {
                Ok(runner) => {
                    let handler = twitch::Handler { sender };
                    handler.listen(runner).await;
                }
                Err(e) => println!("Twitch connect error: {}", e),
            }
        });
    });
}

fn connect_discord_thread(secrets: Secrets, sender: EventBusSender) {
    let discord_token = secrets.discord_token;
    tokio::spawn(async move {
        let mut discord_client = discord::connect(discord_token, sender).await;
        if let Err(why) = discord_client.start_autosharded().await {
            println!("Discord client error: {:?}", why);
        }
    });
}

fn connect_admin_cli_thread(sender: EventBusSender) {
    thread::spawn(|| {
        admin::cli_run(sender);
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
    tokio::spawn(async move {
        if let Err(e) = server::run(sender, event_rx).await {
            println!("Server error: {}", e);
        }
    });
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
                    TwitchEvent::PrivMsg(_, msg) => format!("{}: {}", msg.user, msg.message),
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
