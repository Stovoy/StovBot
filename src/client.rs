use crate::{BotEvent, Event, EventBusSender};
use crossbeam::channel::Receiver;
use rand::Rng;
use reqwest::header::HeaderMap;
use std::thread;
use std::time::Duration;

pub fn run(token: String, sender: EventBusSender, _event_rx: Receiver<Event>) {
    let mut rng = rand::thread_rng();
    let client_id = format!("client_{}", rng.gen_range(1, 100000));
    loop {
        for event in get_events(&token, &client_id) {
            sender.send(Event::BotEvent(event));
        }
        thread::sleep(Duration::from_secs(5));
    }
}

fn get_events(token: &str, client_id: &str) -> Vec<BotEvent> {
    let client = reqwest::blocking::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert("TOKEN", token.parse().unwrap());
    client
        .get(&format!("http://stovoy.tech:8000/poll/{}", client_id))
        .headers(headers)
        .send()
        .unwrap()
        .json()
        .unwrap()
}
