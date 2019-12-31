use crate::bot::BotEvent;
use crate::discord::DiscordEvent;
use crate::twitch::TwitchEvent;
use crossbeam::channel;
use crossbeam::channel::Receiver;
use gui::SharedState;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread;

mod bot;
mod command;
mod db;
mod discord;
mod gui;
mod script;
mod twitch;

#[derive(Deserialize, Debug)]
struct Secrets {
    twitch_token: String,
    discord_token: String,
}

fn main() {
    gui::run();
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
        println!("started");
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
