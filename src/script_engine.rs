use crossbeam::channel::bounded;
use rand::Rng;
use rhai::{Any, AnyExt, Engine, EvalAltResult, RegisterFn};
use std::convert::TryInto;
use std::env;
use std::fmt::Display;
use std::process::exit;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    let script = args.iter().nth(1).unwrap().clone();

    let timeout = Duration::from_millis(1000);
    let (sender, receiver) = bounded(0);

    thread::spawn(move || {
        let mut script_engine = ScriptEngine::new();
        match sender.send(match script_engine.0.eval::<String>(script.as_str()) {
            Ok(result) => result,
            Err(e) => match &e {
                EvalAltResult::ErrorMismatchOutputType(t, output) => match t.as_ref() {
                    "i32" => format!("{}", output.clone().downcast::<i32>().unwrap()),
                    "i64" => format!("{}", output.clone().downcast::<i64>().unwrap()),
                    "f32" => format!("{}", output.clone().downcast::<f32>().unwrap()),
                    "f64" => format!("{}", output.clone().downcast::<f64>().unwrap()),
                    "bool" => format!("{}", output.clone().downcast::<bool>().unwrap()),
                    _ => format!("Script Error: Unknown type {}", e),
                },
                _ => format!("Script Error: {}", e),
            },
        }) {
            Ok(_) => {}
            Err(_) => {}
        }
    });
    match receiver.recv_timeout(timeout) {
        Ok(result) => {
            print!("{}", result);
            exit(0);
        }
        Err(_) => {
            exit(100);
        }
    }
}

pub(crate) struct ScriptEngine(Engine);

impl ScriptEngine {
    fn new() -> ScriptEngine {
        let mut engine = Engine::new();
        engine.register_fn("string", ScriptFunction::string as fn(x: i64) -> String);
        engine.register_fn("string", ScriptFunction::string as fn(x: f32) -> String);
        engine.register_fn("string", ScriptFunction::string as fn(x: bool) -> String);
        engine.register_fn("random", ScriptFunction::random);
        engine.register_fn("len", ScriptFunction::len);
        engine.register_fn("floor", ScriptFunction::floor);
        engine.register_fn("int", ScriptFunction::int);
        engine.register_fn("*", ScriptFunction::mul_f64_i64);
        ScriptEngine(engine)
    }
}

struct ScriptFunction;

impl ScriptFunction {
    fn string<T: Display>(x: T) -> String {
        format!("{}", x)
    }

    fn random() -> f64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(0.0, 1.0)
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

    fn mul_f64_i64(x: f64, y: i64) -> f64 {
        x * (y as f64)
    }
}
