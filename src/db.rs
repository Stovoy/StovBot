use rusqlite::{params, Connection, Result};
use time;
use time::Timespec;

#[derive(Debug)]
struct Command {
    id: i32,
    time_created: Timespec,
    trigger: String,
    response: String,
}

impl Command {
    fn new(trigger: String, response: String) -> Command {
        Command {
            id: 0,
            time_created: time::empty_tm().to_timespec(),
            trigger,
            response,
        }
    }
}

pub fn main() -> Result<()> {
    let path = "./db.db3";
    let conn = Connection::open(&path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS command (
          id            INTEGER PRIMARY KEY AUTOINCREMENT,
          time_created  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
          trigger       TEXT NOT NULL UNIQUE,
          response      TEXT NOT NULL
        )",
        params![],
    )?;

    let command = Command::new("!test".to_string(), "response".to_string());
    conn.execute(
        "INSERT INTO command (trigger, response) VALUES (?1, ?2)",
        params![command.trigger, command.response],
    )?;

    let mut statement = conn.prepare(
        "SELECT id, time_created, trigger, response FROM command")?;
    let commands = statement.query_map(params![], |row| {
        Ok(Command {
            id: row.get(0)?,
            time_created: row.get(1)?,
            trigger: row.get(2)?,
            response: row.get(3)?,
        })
    })?;

    for command in commands {
        println!("Found command {:?}", command.unwrap());
    }
    Ok(())
}
