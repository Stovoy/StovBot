use crate::ConnectError;
use crate::ConnectedState;
use crate::Event;
use iced::alignment::Alignment;
use iced::widget::{Column, Container, Scrollable, Text};
use iced::{application, Element, Length, Settings, Task, Theme};
use futures::StreamExt;

pub fn run() {
    let _ = application(BotGui::new, update, view)
        .title(title)
        .theme(theme)
        .settings(Settings::default())
        .run();
}

#[derive(Debug)]
struct BotGui {
    events: Vec<Event>,
    connections: Option<ConnectedState>,
}

#[derive(Debug, Clone)]
enum Message {
    Connected(std::result::Result<ConnectedState, ConnectError>),
    EventReceived(ConnectedState, Option<Event>),
}

impl BotGui {
    fn new() -> (BotGui, Task<Message>) {
        (
            BotGui {
                events: Vec::new(),
                connections: None,
            },
            Task::perform(crate::connect(crate::Args::default()), Message::Connected),
        )
    }
}

fn title(_state: &BotGui) -> String {
    String::from("StovBot")
}

fn theme(_state: &BotGui) -> Theme {
    Theme::Light
}

fn update(state: &mut BotGui, message: Message) -> Task<Message> {
    match message {
        Message::Connected(connected_state) => match connected_state {
            Ok(connected_state) => {
                state.connections = Some(connected_state);
            }
            Err(_) => {}
        },
        Message::EventReceived(connected_state, event) => {
            state.connections = Some(connected_state);
            if let Some(event) = event {
                state.events.push(event);
            }
        }
    };

    if let Some(connection) = state.connections.take() {
        Task::perform(next_event(connection), |(state, event)| {
            Message::EventReceived(state, event)
        })
    } else {
        Task::none()
    }
}

async fn next_event(mut state: ConnectedState) -> (ConnectedState, Option<Event>) {
    let event = state.next().await;
    (state, event)
}

fn view(state: &BotGui) -> Element<'_, Message> {
    let events = state.events.iter().fold(
        Column::new().spacing(10),
        |column, event| {
            let text = format!("{:?}", event);
            column.push(Text::new(text).size(24).width(Length::Shrink))
        },
    );

    let content = Column::new()
        .width(Length::Shrink)
        .align_x(Alignment::Center)
        .spacing(20)
        .push(Scrollable::new(events).width(Length::Shrink));

    Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
