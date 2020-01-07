use std::env;
use std::io::Error;
use std::process::Command;

enum ScriptRunnerError {
    TimeoutError,
    CrashError,
    IOError(Error),
}

impl From<Error> for ScriptRunnerError {
    fn from(e: Error) -> Self {
        ScriptRunnerError::IOError(e)
    }
}

pub fn run(script: &String) -> String {
    match eval(script) {
        Ok(result) => {
            println!("{}", result);
            result
        }
        Err(e) => match e {
            ScriptRunnerError::TimeoutError => format!("Script Error: Timeout"),
            ScriptRunnerError::CrashError => format!("Script Error: Crash"),
            ScriptRunnerError::IOError(_) => format!("Script Error: IO"),
        },
    }
}

fn eval(script: &String) -> Result<String, ScriptRunnerError> {
    let mut path = env::current_exe()?;
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("script_engine");
    let output = Command::new(path).args(&[script]).output().unwrap();
    match output.status.code().unwrap() {
        100 => Err(ScriptRunnerError::TimeoutError),
        128 => Err(ScriptRunnerError::CrashError),
        _ => Ok(output.stdout.iter().map(|c| *c as char).collect::<String>()),
    }
}
