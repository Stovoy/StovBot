use crate::bot;
use crate::bot::BotEvent;
use crate::discord::DiscordEvent;
use crate::twitch::TwitchEvent;
use crate::{discord, twitch, Secrets};
use crossbeam::channel;
use crossbeam::channel::Receiver;
use futures::stream::BoxStream;
use futures::task::{Context, Poll, Waker};
use iced::{
    Align, Application, Column, Command, Container, Element, Length, Settings, Subscription, Text,
};
use iced_native::subscription::Recipe;
use serde::export::fmt::Error;
use serde::export::Formatter;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::thread;

pub fn run() {
    BotGui::run(Settings::default())
}

#[derive(Debug, Clone)]
struct BotGui {
    last: Vec<Event>,
    connections: Option<ConnectedState>,
}

#[derive(Clone)]
struct ConnectedState {
    bot_event_receiver: Receiver<BotEvent>,
    twitch_event_receiver: Receiver<TwitchEvent>,
    discord_event_receiver: Receiver<DiscordEvent>,

    shared_state: Arc<Mutex<SharedState>>,
}

// Application needs Debug implemented, but we can't implement it on an Arc.
impl Debug for ConnectedState {
    fn fmt(&self, _: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}

pub struct SharedState {
    pub waker: Option<Waker>,
}

#[derive(Debug, Clone)]
enum Message {
    Connected(Result<ConnectedState, ConnectError>),
    EventOccurred(Event),
}

#[derive(Debug, Clone)]
enum ConnectError {
    FileError,
}

async fn connect() -> Result<ConnectedState, ConnectError> {
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
        twitch::listen(twitch_client, twitch_event_senders, thread_shared_state);
    });

    let thread_shared_state = shared_state.clone();
    let discord_token = secrets.discord_token;
    thread::spawn(|| {
        let mut discord_client =
            discord::connect(discord_token, discord_event_senders, thread_shared_state);
        if let Err(why) = discord_client.start_autosharded() {
            println!("Discord client error: {:?}", why);
        }
        println!("started");
    });

    let bot_twitch_event_receiver = twitch_event_receivers[0].clone();
    let bot_discord_event_receiver = discord_event_receivers[0].clone();

    thread::spawn(|| {
        twitch_writer.join("stovoy").unwrap();
        let mut stov_bot = bot::Bot {
            username: "StovBot".to_string(),
            commands: Vec::new(),
            bot_event_sender,
            twitch_event_receiver: bot_twitch_event_receiver,
            discord_event_receiver: bot_discord_event_receiver,
            twitch_writer,
        };
        stov_bot.commands.push(Box::from(bot::BasicCommand {
            trigger: "!test".to_string(),
            response: "test successful".to_string(),
        }));
        loop {
            stov_bot.process_messages();
        }
    });

    Ok(ConnectedState {
        bot_event_receiver,
        twitch_event_receiver: twitch_event_receivers[1].clone(),
        discord_event_receiver: discord_event_receivers[1].clone(),

        shared_state,
    })
}

impl Application for BotGui {
    type Message = Message;

    fn new() -> (BotGui, Command<Message>) {
        (
            BotGui {
                last: Vec::new(),
                connections: None,
            },
            Command::perform(connect(), Message::Connected),
        )
    }

    fn title(&self) -> String {
        String::from("StovBot")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Connected(connected_state) => match connected_state {
                Ok(connected_state) => {
                    self.connections = Some(connected_state);
                }
                Err(_) => {}
            },
            Message::EventOccurred(event) => {
                self.last.push(event);

                if self.last.len() > 5 {
                    self.last.remove(0);
                }
            }
        };

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        match &self.connections {
            None => Subscription::none(),
            Some(c) => Subscription::from_recipe(Events(c.clone())).map(Message::EventOccurred),
        }
    }

    fn view(&mut self) -> Element<Message> {
        let events = self.last.iter().fold(
            Column::new().width(Length::Shrink).spacing(10),
            |column, event| {
                let text = match event {
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
                };
                column.push(Text::new(text).size(40).width(Length::Shrink))
            },
        );

        let content = Column::new()
            .width(Length::Shrink)
            .align_items(Align::Center)
            .spacing(20)
            .push(events);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

impl futures::stream::Stream for ConnectedState {
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
        unimplemented!()
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
    fn fmt(&self, _: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}

pub struct Events(ConnectedState);

/// The hasher used to compare subscriptions.
#[derive(Debug)]
pub struct Hasher(DefaultHasher);

impl Default for Hasher {
    fn default() -> Self {
        Hasher(DefaultHasher::default())
    }
}

impl std::hash::Hasher for Hasher {
    fn finish(&self) -> u64 {
        self.0.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes)
    }
}

impl<H, I> Recipe<H, I> for Events
where
    H: std::hash::Hasher,
{
    type Output = Event;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;

        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, _input: I) -> BoxStream<'static, Self::Output> {
        Pin::from(Box::from(self.0))
    }
}
