use crate::command::{Command, Variable, VariableValue};
use rusqlite::types::{FromSql, FromSqlError, ToSql, ToSqlOutput, Value, ValueRef};
use rusqlite::{params, Connection, Error, ErrorCode, Row};
use serde_json;
use time;

pub(crate) struct Database {
    connection: Connection,
}

impl Database {
    pub(crate) fn new() -> Result<Database, Error> {
        let path = "./db.db3";
        let connection = Connection::open(&path)?;

        let database = Database { connection };
        database.migrate()?;
        Ok(database)
    }

    #[cfg(test)]
    fn new_in_memory() -> Result<Database, Error> {
        let connection = Connection::open_in_memory()?;
        let database = Database { connection };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&self) -> Result<(), Error> {
        let tables = [
            "CREATE TABLE IF NOT EXISTS command (
              id            INTEGER PRIMARY KEY AUTOINCREMENT,
              time_created  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
              trigger       TEXT NOT NULL UNIQUE,
              response      TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS variable (
              id            INTEGER PRIMARY KEY AUTOINCREMENT,
              time_created  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
              time_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
              name          TEXT NOT NULL UNIQUE,
              value         TEXT NOT NULL
            )",
        ];
        for table in tables.iter() {
            self.connection.execute(table, params![])?;
        }

        for command in Command::default_commands() {
            if let Err(error) = self.add_command(&command) {
                match error {
                    Error::SqliteFailure(inner_error, _) => {
                        if inner_error.code == ErrorCode::ConstraintViolation {
                            continue;
                        } else {
                            return Err(error);
                        }
                    }
                    _ => return Err(error),
                }
            }
        }

        Ok(())
    }

    pub(crate) fn add_command(&self, command: &Command) -> Result<usize, Error> {
        self.connection.execute(
            "INSERT INTO command (trigger, response) VALUES (?1, ?2)",
            params![command.trigger, command.response],
        )
    }

    pub(crate) fn delete_command(&self, command: &Command) -> Result<usize, Error> {
        self.connection.execute(
            "DELETE FROM command WHERE trigger = ?1",
            params![command.trigger],
        )
    }

    pub(crate) fn update_command(&self, command: &Command) -> Result<usize, Error> {
        self.connection.execute(
            "UPDATE command SET response = ?2 WHERE trigger = ?1",
            params![command.trigger, command.response],
        )
    }

    pub(crate) fn get_commands(&self) -> Result<Vec<Command>, Error> {
        let mut statement = self
            .connection
            .prepare("SELECT id, time_created, trigger, response FROM command")?;
        let commands_iter = statement.query_map(params![], |row: &Row| {
            Ok(Command {
                id: row.get(0)?,
                time_created: row.get(1)?,
                trigger: row.get(2)?,
                response: row.get(3)?,
                actor: None,
            })
        })?;

        let mut commands = Vec::new();
        for command in commands_iter {
            commands.push(command.unwrap());
        }
        Ok(commands)
    }

    pub(crate) fn get_variable(&self, name: String) -> Result<Variable, Error> {
        let mut statement = self.connection.prepare(
            "SELECT id, time_created, time_modified, name, value \
             FROM variable WHERE name = ?1",
        )?;
        statement.query_row(params![name], |row: &Row| {
            Ok(Variable {
                id: row.get(0)?,
                time_created: row.get(1)?,
                time_modified: row.get(2)?,
                name: row.get(3)?,
                value: row.get(4)?,
            })
        })
    }

    pub(crate) fn set_variable(&self, variable: &Variable) -> Result<usize, Error> {
        self.connection.execute(
            "INSERT INTO variable(name, value) VALUES(?1, ?2)
             ON CONFLICT(name) DO UPDATE SET value = ?2, time_modified = ?3",
            params![variable.name, variable.value, time::get_time()],
        )
    }
}

impl FromSql for VariableValue {
    fn column_result(value: ValueRef<'_>) -> Result<Self, FromSqlError> {
        match serde_json::from_str(value.as_str()?) {
            Ok(result) => Ok(result),
            Err(_) => Err(FromSqlError::InvalidType),
        }
    }
}

impl ToSql for VariableValue {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, Error> {
        Ok(ToSqlOutput::Owned(Value::Text(
            serde_json::to_string(self).unwrap(),
        )))
    }
}

#[test]
fn test_add_command() -> Result<(), Error> {
    let database = Database::new_in_memory()?;
    database.add_command(&Command::new(
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

#[test]
fn test_set_variable() -> Result<(), Error> {
    let database = Database::new_in_memory()?;
    database.set_variable(&Variable::new(
        "variable".to_string(),
        VariableValue::Text("value".to_string()),
    ))?;
    let variable = database.get_variable("variable".to_string())?;
    println!("{:?}", variable);
    assert_eq!(variable.value, VariableValue::Text("value".to_string()));
    Ok(())
}
