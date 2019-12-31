use rusqlite::{params, Connection, Result, Row};
use time;
use time::Timespec;

#[derive(Debug)]
pub(crate) struct Command {
    id: i32,
    time_created: Timespec,
    trigger: String,
    response: String,
}

impl Command {
    pub(crate) fn new(trigger: String, response: String) -> Command {
        Command {
            id: 0,
            time_created: time::empty_tm().to_timespec(),
            trigger,
            response,
        }
    }
}

pub(crate) struct Database {
    connection: Connection,
}

impl Database {
    pub(crate) fn new() -> Result<Database> {
        let path = "./db.db3";
        let connection = Connection::open(&path)?;

        let database = Database { connection };
        database.migrate()?;
        Ok(database)
    }

    #[cfg(test)]
    fn new_in_memory() -> Result<Database> {
        let connection = Connection::open_in_memory()?;
        let database = Database { connection };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&self) -> Result<usize> {
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS command (
              id            INTEGER PRIMARY KEY AUTOINCREMENT,
              time_created  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
              trigger       TEXT NOT NULL UNIQUE,
              response      TEXT NOT NULL
          )",
            params![],
        )
    }

    pub(crate) fn add_command(&self, command: Command) -> Result<usize> {
        self.connection.execute(
            "INSERT INTO command (trigger, response) VALUES (?1, ?2)",
            params![command.trigger, command.response],
        )
    }

    pub(crate) fn get_commands(&self) -> Result<Vec<Command>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, time_created, trigger, response FROM command")?;
        let commands_iter = statement.query_map(params![], |row: &Row| {
            Ok(Command {
                id: row.get(0)?,
                time_created: row.get(1)?,
                trigger: row.get(2)?,
                response: row.get(3)?,
            })
        })?;

        let mut commands = Vec::new();
        for command in commands_iter {
            commands.push(command.unwrap());
        }
        Ok(commands)
    }
}

#[test]
fn test_add_command() -> Result<()> {
    let database = Database::new_in_memory()?;
    database.add_command(Command::new(
        "!test".to_string(),
        "test successful".to_string(),
    ))?;
    let commands = database.get_commands()?;
    assert_eq!(1, commands.len());
    assert_eq!("!test", commands.get(0).unwrap().trigger);
    Ok(())
}
