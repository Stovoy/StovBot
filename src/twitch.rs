use crate::{Event, EventBusSender};
use twitchchat::{
    connector,
    messages,
    runner::{AsyncRunner, Status},
    UserConfig, Writer,
};

#[derive(Clone)]
pub struct TwitchPrivMsg {
    pub user: String,
    pub message: String,
    pub channel: String,
}

pub async fn connect(
    token: String,
) -> Result<AsyncRunner, Box<dyn std::error::Error + Send + Sync>> {
    let user_config = UserConfig::builder()
        .name("StovBot")
        .token(token)
        .enable_all_capabilities()
        .build()?;
    let connector = connector::tokio::Connector::twitch()?;
    Ok(AsyncRunner::connect(connector, &user_config).await?)
}

#[derive(Clone)]
pub enum TwitchEvent {
    Ready(Writer),
    PrivMsg(Writer, TwitchPrivMsg),
}

pub struct Handler {
    pub sender: EventBusSender,
}

impl Handler {
    pub async fn listen(&self, mut runner: AsyncRunner) {
        let writer = runner.writer();
        let _ = runner.join("stovoy").await;
        self.sender
            .send(Event::TwitchEvent(TwitchEvent::Ready(writer.clone())));

        loop {
            match runner.next_message().await {
                Ok(Status::Message(msg)) => match msg {
                    messages::Commands::Privmsg(msg) => {
                        self.sender.send(Event::TwitchEvent(TwitchEvent::PrivMsg(
                            writer.clone(),
                            TwitchPrivMsg {
                                user: msg.name().to_string(),
                                message: msg.data().to_string(),
                                channel: msg.channel().to_string(),
                            },
                        )));
                    }
                    _ => {}
                },
                Ok(Status::Quit) | Ok(Status::Eof) => break,
                Err(err) => {
                    eprintln!("Twitch error: {}", err);
                    break;
                }
            }
        }
    }
}
