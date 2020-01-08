#![allow(dead_code)]

use crate::database::Database;
use crate::models::{Variable, VariableValue};
use crossbeam::channel::bounded;
use crossbeam::channel::RecvTimeoutError;
use rand::Rng;
use rhai::{Any, AnyExt, Engine, EvalAltResult, RegisterFn};
use std::convert::TryInto;
use std::fmt::Display;
use std::ops::{Add, Mul};
use std::process::exit;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use std::{env, panic};
use rusqlite::Error;

mod database;
mod models;

fn main() {
    let args: Vec<String> = env::args().collect();
    let script = args.get(1).unwrap().clone();

    let timeout = Duration::from_millis(1500);
    let (sender, receiver) = bounded(0);

    thread::spawn(move || {
        panic::set_hook(Box::new(|_| {}));

        let result = panic::catch_unwind(|| {
            let mut script_engine = ScriptEngine::new();
            match script_engine.0.eval::<String>(script.as_str()) {
                Ok(result) => result,
                Err(e) => match &e {
                    EvalAltResult::ErrorMismatchOutputType(t, output) => match t.as_ref() {
                        "i64" => format!("{}", output.clone().downcast::<i64>().unwrap()),
                        "f64" => format!("{}", output.clone().downcast::<f64>().unwrap()),
                        "bool" => format!("{}", output.clone().downcast::<bool>().unwrap()),
                        _ => format!("Script Error: Unknown type {}", e),
                    },
                    _ => format!("Script Error: {}", e),
                },
            }
        });
        match sender.send(match result {
            Ok(result) => result,
            Err(err) => format!(
                "Script Error: {}",
                match err.downcast_ref::<&'static str>() {
                    Some(s) => *s,
                    None => match err.downcast_ref::<String>() {
                        Some(s) => &s[..],
                        None => "Box<Any>",
                    },
                }
            ),
        }) {
            Ok(_) => {}
            Err(e) => println!("{:?}", e),
        }
    });
    match receiver.recv_timeout(timeout) {
        Ok(result) => {
            print!("{}", result);
            exit(0);
        }
        Err(e) => match e {
            RecvTimeoutError::Timeout => exit(100),
            RecvTimeoutError::Disconnected => exit(128),
        },
    }
}

pub struct ScriptEngine(Engine);

impl ScriptEngine {
    fn new() -> ScriptEngine {
        let mut engine = Engine::new();
        engine.register_fn("string", ScriptFunction::string as fn(x: i64) -> String);
        engine.register_fn("string", ScriptFunction::string as fn(x: f64) -> String);
        engine.register_fn("string", ScriptFunction::string as fn(x: bool) -> String);
        engine.register_fn("random", ScriptFunction::random);
        engine.register_fn("random_index", ScriptFunction::random_index);
        engine.register_fn("len", ScriptFunction::len);
        engine.register_fn("floor", ScriptFunction::floor);
        engine.register_fn("int", ScriptFunction::int);
        engine.register_fn("*", ScriptFunction::mul as fn(x: i64, y: i64) -> i64);
        engine.register_fn("*", ScriptFunction::mul as fn(x: f64, y: f64) -> f64);
        engine.register_fn("*", ScriptFunction::mul as fn(x: i64, y: f64) -> i64);
        engine.register_fn("*", ScriptFunction::mul as fn(x: f64, y: i64) -> f64);
        engine.register_fn(
            "+",
            ScriptFunction::add_string_number as fn(x: String, y: i64) -> i64,
        );
        engine.register_fn(
            "+",
            ScriptFunction::add_string_number as fn(x: String, y: f64) -> f64,
        );
        engine.register_fn("get", ScriptFunction::get);
        engine.register_fn("set", ScriptFunction::set as fn(x: String, y: String));
        engine.register_fn("set", ScriptFunction::set as fn(x: String, y: i64));
        engine.register_fn("set", ScriptFunction::set as fn(x: String, y: f64));
        engine.register_fn("set", ScriptFunction::set as fn(x: String, y: bool));
        engine.register_fn("get_list", ScriptFunction::get_list);
        ScriptEngine(engine)
    }
}

struct ScriptFunction(Database);

impl ScriptFunction {
    fn string<T: Display>(x: T) -> String {
        format!("{}", x)
    }

    fn random() -> f64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(0.0, 1.0)
    }

    fn random_index(x: Vec<Box<dyn Any>>) -> i64 {
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0, x.len());

        index as i64
    }

    fn len(x: Vec<Box<dyn Any>>) -> i64 {
        x.len().try_into().unwrap()
    }

    fn floor(x: f64) -> i64 {
        x.floor() as i64
    }

    fn int(x: String) -> i64 {
        match x.parse() {
            Ok(i) => i,
            Err(_) => 0,
        }
    }

    fn mul<T: Mul + From<U>, U: Mul>(x: T, y: U) -> <T as Mul>::Output {
        x * (Into::into(y))
    }

    fn add_string_number<T: FromStr + Add>(x: String, y: T) -> <T as Add>::Output {
        match x.parse::<T>() {
            Ok(x) => x + y,
            Err(_) => panic!("Could not parse string as number"),
        }
    }

    // TODO: How to get these variable modification events across process to StovBot?
    // Want to serialize all these events and send them on stdout along with script output,
    // which means we'll need to store them somehow.
    fn get(name: String) -> String {
        let database = Database::connect(None).unwrap();
        match database.get_variable(&name) {
            Ok(variable) => match variable.value {
                VariableValue::Text(text) => text,
                VariableValue::StringList(_) => panic!(format!(
                    "Variable {} is StringList, not Text. Use get_list()!",
                    name
                )),
            },
            Err(e) => match e {
                Error::QueryReturnedNoRows => panic!(format!("Variable {} does not exist!", name)),
                _ => panic!(e)
            }
        }
    }

    fn set<T: Display>(name: String, value: T) {
        let database = Database::connect(None).unwrap();
        database
            .set_variable(&Variable::new(
                name,
                VariableValue::Text(format!("{}", value)),
            ))
            .unwrap();
    }

    fn get_list(name: String) -> Vec<Box<dyn Any>> {
        let database = Database::connect(None).unwrap();
        match database.get_variable(&name) {
            Ok(variable) => match variable.value {
                VariableValue::Text(_) => panic!(format!(
                    "Variable {} is Text, not StringList. Use get()!",
                    name
                )),
                VariableValue::StringList(list) => {
                    let mut results: Vec<Box<dyn Any>> = Vec::new();
                    for item in list.iter() {
                        results.push(Box::new(item.value.clone()));
                    }
                    results
                }
            },
            Err(e) => match e {
                Error::QueryReturnedNoRows => panic!(format!("Variable {} does not exist!", name)),
                _ => panic!(e)
            }
        }
    }
}

trait From<T>: Sized {
    fn from(_: T) -> Self;
}

impl From<f64> for f64 {
    fn from(x: f64) -> Self {
        x
    }
}

impl From<i64> for i64 {
    fn from(x: i64) -> Self {
        x
    }
}

impl From<i64> for f64 {
    fn from(x: i64) -> Self {
        x as f64
    }
}

impl From<f64> for i64 {
    fn from(x: f64) -> Self {
        x as i64
    }
}

impl<T, U> Into<U> for T
    where
        U: From<T>,
{
    fn into(self) -> U {
        U::from(self)
    }
}

trait Into<T>: Sized {
    fn into(self) -> T;
}
