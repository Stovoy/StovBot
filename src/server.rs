use crate::bot::BotEvent;
use crate::{Event, EventBusSender};
use crossbeam::channel::Receiver;
use rocket::State;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Deserialize)]
struct PollState {
    events: Arc<Mutex<Vec<BotEvent>>>,
    client_indicies: Mutex<HashMap<String, usize>>,
}

impl PollState {
    fn new(event_rx: Receiver<Event>) -> PollState {
        let events = Arc::new(Mutex::new(Vec::new()));

        let poll_state = PollState {
            events: events.clone(),
            client_indicies: Mutex::new(HashMap::new()),
        };

        thread::spawn(move || loop {
            let event = event_rx.recv().unwrap();
            match event {
                Event::BotEvent(event) => {
                    events.lock().unwrap().push(event);
                }
                _ => {}
            }
        });

        poll_state
    }
}

#[get("/poll/<client>")]
fn poll(client: String, state: State<PollState>) -> String {
    let mut index_map = state.client_indicies.lock().unwrap();
    let index = index_map.entry(client).or_insert(0);
    let events = state.events.lock().unwrap();
    match serde_json::to_string(&events[*index..]) {
        Ok(json) => {
            *index = events.len();
            json
        }
        Err(e) => e.to_string(),
    }
}

pub fn run(_sender: EventBusSender, event_rx: Receiver<Event>) {
    rocket::ignite()
        .manage(PollState::new(event_rx))
        .mount("/", routes![poll])
        .launch();
}
