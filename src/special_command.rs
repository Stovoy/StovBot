use crate::models::{
    Action, ActionError, Actor, Command, EditType, Message, StringItem, Variable, VariableValue,
};

/*
Variables
    Text
    StringList

Commands
    done !variable add <var name> <value>
    done !variable add <var name> [<value?>] (array)
    done !variable edit <var name> <new value> (should never change form from text -> stringlist or vice versa)
    !variable edit <var name>+ <value to append> (append to array or string)
    !variable edit <var name>- <value to remove> (remove from array or string)
    !variable edit <var name>-# <index to remove> (remove from array or string)
    !variable edit <var name>+# <index to insert at> (insert into array or string)
    !variable delete <var name>
*/
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
    let (name, value, edit_type) = parse_variable_message(command, message)?;
    if edit_type != EditType::Overwrite() {
        Err(ActionError::VariableEditTypeNotSupported)
    } else {
        Ok(Action::AddVariable(Variable::new(name, value)))
    }
}

fn edit_variable(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let (name, value, edit_type) = parse_variable_message(command, message)?;
    Ok(Action::EditVariable(Variable::new(name, value), edit_type))
}

fn delete_variable(command: &Command, message: &Message) -> Result<Action, ActionError> {
    let (name, value, edit_type) = parse_variable_message(command, message)?;
    if edit_type != EditType::Overwrite() {
        Err(ActionError::VariableEditTypeNotSupported)
    } else {
        Ok(Action::DeleteVariable(Variable::new(name, value)))
    }
}

fn parse_variable_message(
    command: &Command,
    message: &Message,
) -> Result<(String, VariableValue, EditType), ActionError> {
    let variable = message.after_trigger(&command.trigger);
    let parts: Vec<&str> = variable.split(' ').collect();
    if parts.is_empty() {
        Err(ActionError::BadVariable(variable.to_string()))
    } else if parts.len() == 1 {
        let name = parts[0];
        Ok((
            name.to_string(),
            VariableValue::Text("".to_string()),
            EditType::Overwrite(),
        ))
    } else {
        let mut name = parts[0];
        let mut edit_type = EditType::Overwrite();
        let mut value = parts[1..].join(" ");

        if name.ends_with("+") {
            edit_type = EditType::Append();
            name = &name[0..name.len() - 1];
        } else if name.ends_with("-") {
            edit_type = EditType::Remove();
            name = &name[0..name.len() - 1];
        } else if name.ends_with("+#") {
            match parse_variable_index(parts) {
                Ok((index, v)) => {
                    edit_type = EditType::InsertAt(index);
                    match v {
                        Some(v) => value = v,
                        None => return Err(ActionError::VariableBadEditIndexValue),
                    }
                }
                Err(err) => return Err(err),
            }
            name = &name[0..name.len() - 2];
        } else if name.ends_with("-#") {
            match parse_variable_index(parts) {
                Ok((index, _)) => {
                    edit_type = EditType::RemoveAt(index);
                    value = "".to_string();
                }
                Err(err) => return Err(err),
            }
            name = &name[0..name.len() - 2];
        }

        if value.starts_with("[") && value.ends_with("]") {
            let vec = if value == "[]" {
                Vec::new()
            } else {
                vec![StringItem::new(&value[1..value.len() - 1])]
            };

            Ok((name.to_string(), VariableValue::StringList(vec), edit_type))
        } else {
            Ok((name.to_string(), VariableValue::Text(value), edit_type))
        }
    }
}

fn parse_variable_index(parts: Vec<&str>) -> Result<(usize, Option<String>), ActionError> {
    match parts[1].parse::<usize>() {
        Ok(index) => {
            if parts.len() == 2 {
                Ok((index, None))
            } else {
                Ok((index, Some(parts[2..].join(" "))))
            }
        }
        Err(_) => Err(ActionError::VariableBadEditIndex),
    }
}
