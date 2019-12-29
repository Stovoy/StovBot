use serde::Deserialize;

mod bot;
mod discord;
mod gui;
mod twitch;

#[derive(Deserialize, Debug)]
struct Secrets {
    twitch_token: String,
    discord_token: String,
}

fn main() {
    gui::run();
}
