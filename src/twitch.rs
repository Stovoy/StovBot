use twitchchat::*;
use env_logger;

pub fn connect(token: String) {
    env_logger::init().unwrap();

    let client = twitchchat::connect(
        twitchchat::UserConfig::builder()
            .membership()
            .commands()
            .tags()
            .nick("StovBot")
            .token(token)
            .build()
            .expect("error creating UserConfig"))
        .expect("failed to connect to twitch")
        .filter::<commands::PrivMsg>();

    let writer = client.writer();

    for event in client {
        match event {
            Event::IrcReady(msg) => {
                println!("irc ready: {}", msg);
            }
            Event::TwitchReady(msg) => {
                println!("twitch ready {:?}", msg);
                writer.join("stovoy").unwrap();
            }
            Event::Message(Message::Join(msg)) => {
                println!("*** {} joined {}", msg.user(), msg.channel())
            }
            Event::Message(Message::Part(msg)) => {
                println!("Part: {:?}", msg);
            }
            Event::Message(Message::PrivMsg(msg)) => {
                println!("Private message - {}: {}", msg.user(), msg.message());
            }
            Event::Message(Message::Mode(msg)) => {
                println!("Mode: {:?}", msg);
            }
            Event::Message(Message::NamesStart(msg)) => {
                println!("NamesStart: {:?}", msg);
            }
            Event::Message(Message::NamesEnd(msg)) => {
                println!("NamesEnd: {:?}", msg);
            }
            Event::Message(Message::ClearChat(msg)) => {
                println!("ClearChat: {:?}", msg);
            }
            Event::Message(Message::ClearMsg(msg)) => {
                println!("ClearMsg: {:?}", msg);
            }
            Event::Message(Message::HostTargetStart(msg)) => {
                println!("HostTargetStart: {:?}", msg);
            }
            Event::Message(Message::HostTargetEnd(msg)) => {
                println!("HostTargetEnd: {:?}", msg);
            }
            Event::Message(Message::Notice(msg)) => {
                println!("Notice: {:?}", msg);
            }
            Event::Message(Message::Reconnect(msg)) => {
                println!("Reconnect: {:?}", msg);
            }
            Event::Message(Message::RoomState(msg)) => {
                println!("RoomState: {:?}", msg);
            }
            Event::Message(Message::UserNotice(msg)) => {
                println!("UserNotice: {:?}", msg);
            }
            Event::Message(Message::UserState(msg)) => {
                println!("UserState: {:?}", msg);
            }
            Event::Message(Message::GlobalUserState(msg)) => {
                println!("GlobalUserState: {:?}", msg);
            }
            Event::Message(Message::Irc(msg)) => {
                match *msg {
                    twitchchat::irc::Message::Ping { token: _ } => {
                        println!("irc ping");
                    }
                    twitchchat::irc::Message::Cap { acknowledge, cap } => {
                        println!("irc cap: {} {}", acknowledge, cap);
                    }
                    twitchchat::irc::Message::Connected { name: _ } => {
                        println!("irc connected");
                    }
                    twitchchat::irc::Message::Ready { name: _ } => {
                        println!("irc ready");
                    }
                    twitchchat::irc::Message::Unknown { prefix: _, tags: _, head, args: _, tail: _ } => {
                        println!("Message head: {}", head);
                    }
                }
            }
            Event::Error(err) => {
                eprintln!("error: {}", err);
                break;
            }
            _ => unreachable!()
        }
    }

    println!("done");
}
