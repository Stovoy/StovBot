use crate::discord::DiscordEvent;
use crate::Event;
use bus::BusReader;
use chrono::NaiveDateTime;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use std::thread;
use std::time::Duration;

mod date_serializer {
    use chrono::NaiveDateTime;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<NaiveDateTime, D::Error> {
        let time: String = Deserialize::deserialize(deserializer)?;
        Ok(NaiveDateTime::parse_from_str(&time, "%Y-%m-%dT%H:%M:%SZ")
            .map_err(serde::de::Error::custom)?)
    }
}

#[derive(Deserialize)]
struct TwitchStreamStatus {
    data: Vec<TwitchStreamStatusData>,
}

#[derive(Deserialize)]
struct TwitchStreamStatusData {
    #[serde(rename = "type")]
    status: String,
    #[serde(with = "date_serializer", rename = "started_at")]
    _started_at: NaiveDateTime,
}

pub fn run(twitch_client_id: String, mut event_rx: BusReader<Event>) {
    let mut notification_channel;
    loop {
        notification_channel = match event_rx.recv() {
            Ok(message) => match message {
                #[cfg(feature = "discord")]
                Event::DiscordEvent(event) => match event {
                    DiscordEvent::Ready(ctx, notification_channel_id) => {
                        Some((ctx, notification_channel_id))
                    }
                    _ => None,
                },
                _ => None,
            },
            Err(_) => None,
        };
        match notification_channel {
            None => {}
            Some(_) => {
                break;
            }
        };
    }

    loop {
        if is_live(&twitch_client_id) {
            let (ctx, channel_id) = notification_channel.unwrap();
            channel_id.send_message(
                ctx.lock().unwrap().http.clone(),
                |m| {
                    m.content("Hey @everyone, Stovoy is now live! Come watch over at https://www.twitch.tv/stovoy !");

                    m
                },
            ).unwrap();

            break;
        }

        thread::sleep(Duration::from_secs(5));
    }
}

fn is_live(twitch_client_id: &str) -> bool {
    let client = reqwest::blocking::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert("CLIENT-ID", twitch_client_id.parse().unwrap());

    let response: TwitchStreamStatus = client
        .get("https://api.twitch.tv/helix/streams?user_login=Stovoy")
        .headers(headers)
        .send()
        .unwrap()
        .json()
        .unwrap();
    !response.data.is_empty() && response.data[0].status == "live"
}
