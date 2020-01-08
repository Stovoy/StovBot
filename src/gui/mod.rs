use crate::ConnectError;
use crate::ConnectedState;
use crate::Event;
use futures::stream::BoxStream;
use iced::*;
use iced_native::subscription::Recipe;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::pin::Pin;

pub fn run() {
    BotGui::run(Settings::default())
}

#[derive(Debug, Clone)]
struct BotGui {
    events: Vec<Event>,
    events_scroll: scrollable::State,
    connections: Option<ConnectedState>,
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
                events: Vec::new(),
                events_scroll: scrollable::State::new(),
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
                self.events.push(event);
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
        let events = self.events.iter().fold(
            Scrollable::new(&mut self.events_scroll)
                .width(Length::Shrink)
                .spacing(10),
            |column, event| {
                let text = format!("{:?}", event);
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

pub struct Events(ConnectedState);

impl<H, I> Recipe<H, I> for Events
where
    H: Hasher,
{
    type Output = Event;

    fn hash(&self, state: &mut H) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, _input: I) -> BoxStream<'static, Self::Output> {
        Pin::from(Box::from(self.0))
    }
}
