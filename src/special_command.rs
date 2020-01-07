use crate::models::{Action, ActionError, Actor, Command, Message};

pub fn commands() -> Vec<Command> {
    vec![
        Command::new_with_actor(
            "!command add".to_string(),
            "Your command has been added".to_string(),
            Some(Actor(add_command)),
        ),
        Command::new_with_actor(
            "!command delete".to_string(),
            "Your command has been deleted".to_string(),
            Some(Actor(delete_command)),
        ),
        Command::new_with_actor(
            "!command edit".to_string(),
            "Your command has been edited".to_string(),
            Some(Actor(edit_command)),
        ),
    ]
}

fn add_command(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let command = message.after_trigger(&command.trigger);
    match command.find(' ') {
        Some(index) => {
            let (trigger, response) = command.split_at(index);
            if !trigger.starts_with("!") {
                Err(ActionError::BadCommandTriggerPrefix)
            } else {
                Ok(Action::AddCommand(Command::new(
                    trigger.to_string(),
                    response.to_string(),
                )))
            }
        }
        None => Err(ActionError::BadCommand(command.to_string())),
    }
}

fn delete_command(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let command = message.after_trigger(&command.trigger);
    if !command.starts_with("!") {
        Err(ActionError::BadCommandTriggerPrefix)
    } else {
        let trigger = command.split(" ").next().unwrap();
        Ok(Action::DeleteCommand(Command::new(
            trigger.to_string(),
            "".to_string(),
        )))
    }
}

fn edit_command(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let command = message.after_trigger(&command.trigger);
    match command.find(' ') {
        Some(index) => {
            let (trigger, response) = command.split_at(index);
            if !trigger.starts_with("!") {
                Err(ActionError::BadCommandTriggerPrefix)
            } else {
                Ok(Action::EditCommand(Command::new(
                    trigger.to_string(),
                    response.to_string(),
                )))
            }
        }
        None => Err(ActionError::BadCommand(command.to_string())),
    }
}
