use crate::command::Command;
use rusqlite::{params, Connection, Error, ErrorCode, Result, Row};

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

    fn migrate(&self) -> Result<()> {
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS command (
              id            INTEGER PRIMARY KEY AUTOINCREMENT,
              time_created  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
              trigger       TEXT NOT NULL UNIQUE,
              response      TEXT NOT NULL
          )",
            params![],
        )?;

        for command in Command::default_commands() {
            if let Err(error) = self.add_command(command) {
                match error {
                    Error::SqliteFailure(inner_error, _) => {
                        if inner_error.code == ErrorCode::ConstraintViolation {
                            continue;
                        } else {
                            return Err(error);
                        }
                    }
                    _ => {
                        return Err(error);
                    }
                }
            }
        }

        Ok(())
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
    assert!(commands.len() > 0);
    let mut found = false;
    for command in commands {
        if command.trigger == "!test" {
            found = true;
        }
    }
    assert!(found);
    Ok(())
}
