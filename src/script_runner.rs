use std::env;
use std::io::Error;
use std::process::Command;

enum ScriptRunnerError {
    TimeoutError,
    IOError(Error),
}

impl From<Error> for ScriptRunnerError {
    fn from(e: Error) -> Self {
        ScriptRunnerError::IOError(e)
    }
}

pub(crate) fn run(script: &String) -> String {
    let millis = 1000;
    match eval(script) {
        Ok(result) => {
            println!("{}", result);
            result
        }
        Err(_) => format!("Script Error: Timeout after {} seconds", millis / 1000),
    }
}

fn eval(script: &String) -> Result<String, ScriptRunnerError> {
    let mut script_engine_path = env::current_exe()?;
    script_engine_path.pop();
    script_engine_path.push("script_engine");
    let output = Command::new(script_engine_path)
        .args(&[script])
        .output()
        .unwrap();
    match output.status.code().unwrap() {
        100 => Err(ScriptRunnerError::TimeoutError),
        _ => Ok(output.stdout.iter().map(|c| *c as char).collect::<String>()),
    }
}
