use serde::Deserialize;

mod bot;
mod gui;
mod twitch;

#[derive(Deserialize, Debug)]
struct Secrets {
    token: String,
}

fn main() {
    gui::run();
}
