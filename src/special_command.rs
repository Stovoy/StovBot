use crate::models::{Action, ActionError, Actor, Command, Message, Variable, VariableValue};

pub fn commands() -> Vec<Command> {
    vec![
        Command::new(
            "!command add".to_string(),
            "Your command has been added".to_string(),
        )
        .with_actor(Actor(add_command))
        .build(),
        Command::new(
            "!command edit".to_string(),
            "Your command has been edited".to_string(),
        )
        .with_actor(Actor(edit_command))
        .build(),
        Command::new(
            "!command delete".to_string(),
            "Your command has been deleted".to_string(),
        )
        .with_actor(Actor(delete_command))
        .build(),
        Command::new(
            "!variable add".to_string(),
            "Your variable has been added".to_string(),
        )
        .with_actor(Actor(add_variable))
        .build(),
        Command::new(
            "!variable edit".to_string(),
            "Your variable has been edited".to_string(),
        )
        .with_actor(Actor(edit_variable))
        .build(),
        Command::new(
            "!variable delete".to_string(),
            "Your variable has been deleted".to_string(),
        )
        .with_actor(Actor(delete_variable))
        .build(),
    ]
}

fn add_command(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let (trigger, response) = parse_command_message(command, message)?;
    Ok(Action::AddCommand(Command::new(trigger, response)))
}

fn edit_command(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let (trigger, response) = parse_command_message(command, message)?;
    Ok(Action::EditCommand(Command::new(trigger, response)))
}

fn delete_command(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let command = message.after_trigger(&command.trigger);
    if !command.starts_with('!') {
        Err(ActionError::BadCommandTriggerPrefix)
    } else {
        let trigger = command.split(' ').next().unwrap();
        Ok(Action::DeleteCommand(Command::new(
            trigger.to_string(),
            "".to_string(),
        )))
    }
}

fn parse_command_message(
    command: &Command,
    message: &Message,
) -> Result<(String, String), ActionError> {
    let command = message.after_trigger(&command.trigger);
    let parts: Vec<&str> = command.split(' ').collect();
    if parts.len() <= 1 {
        Err(ActionError::BadCommand(command.to_string()))
    } else {
        let trigger = parts[0];
        let response = parts[1..].join(" ");
        if !trigger.starts_with('!') {
            Err(ActionError::BadCommandTriggerPrefix)
        } else {
            Ok((trigger.to_string(), response))
        }
    }
}

fn add_variable(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let (name, value) = parse_variable_message(command, message)?;
    Ok(Action::AddVariable(Variable::new(name, value)))
}

fn edit_variable(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let (name, value) = parse_variable_message(command, message)?;
    Ok(Action::EditVariable(Variable::new(name, value)))
}

fn delete_variable(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let (name, value) = parse_variable_message(command, message)?;
    Ok(Action::DeleteVariable(Variable::new(name, value)))
}

fn parse_variable_message(
    command: &Command,
    message: &Message,
) -> Result<(String, VariableValue), ActionError> {
    let variable = message.after_trigger(&command.trigger);
    let parts: Vec<&str> = variable.split(' ').collect();
    if parts.is_empty() {
        Err(ActionError::BadVariable(variable.to_string()))
    } else if parts.len() == 1 {
        let name = parts[0];
        Ok((name.to_string(), VariableValue::Text("".to_string())))
    } else {
        let name = parts[0];
        let value = parts[1..].join(" ");
        Ok((name.to_string(), VariableValue::Text(value)))
    }
}
