use std::env;
use std::io::Error;
use std::process::Command;

enum ScriptRunnerError {
    Timeout,
    Crash,
    IO(Error),
}

impl From<Error> for ScriptRunnerError {
    fn from(e: Error) -> Self {
        ScriptRunnerError::IO(e)
    }
}

pub fn run(script: &str, database_path: &str) -> String {
    match eval(script, database_path) {
        Ok(result) => result,
        Err(e) => match e {
            ScriptRunnerError::Timeout => "Script Error: Timeout".to_string(),
            ScriptRunnerError::Crash => "Script Error: Crash".to_string(),
            ScriptRunnerError::IO(_) => "Script Error: IO".to_string(),
        },
    }
}

fn eval(script: &str, database_path: &str) -> Result<String, ScriptRunnerError> {
    let mut path = env::current_exe()?;
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("script_engine");
    let output = Command::new(path)
        .args(&[script])
        .env("WITH_DATABASE", database_path)
        .output()
        .unwrap();
    match output.status.code().unwrap() {
        100 => Err(ScriptRunnerError::Timeout),
        128 => Err(ScriptRunnerError::Crash),
        _ => Ok(output.stdout.iter().map(|c| *c as char).collect::<String>()),
    }
}
