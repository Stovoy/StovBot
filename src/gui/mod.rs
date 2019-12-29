use crate::bot;
use crate::discord::DiscordEvent;
use crate::twitch::TwitchEvent;
use crate::ConnectError;
use crate::ConnectedState;
use crate::{discord, twitch};
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

pub fn run() {
    BotGui::run(Settings::default())
}

#[derive(Debug, Clone)]
struct BotGui {
    last: Vec<Event>,
    connections: Option<ConnectedState>,
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

impl Application for BotGui {
    type Message = Message;

    fn new() -> (BotGui, Command<Message>) {
        (
            BotGui {
                last: Vec::new(),
                connections: None,
            },
            Command::perform(crate::connect(), Message::Connected),
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
