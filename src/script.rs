use crossbeam::channel::bounded;
use rand::Rng;
use rhai::{Any, AnyExt, Engine, EvalAltResult, RegisterFn};
use std::convert::TryInto;
use std::fmt::Display;
use std::thread;
use std::time::Duration;

struct TimeoutError;

pub(crate) struct ScriptEngine(Engine);

impl ScriptEngine {
    fn new() -> ScriptEngine {
        let mut engine = Engine::new();
        engine.register_fn(
            "to_string",
            ScriptFunction::to_string as fn(x: i64) -> String,
        );
        engine.register_fn("random", ScriptFunction::random);
        engine.register_fn("len", ScriptFunction::len);
        engine.register_fn("floor", ScriptFunction::floor);
        engine.register_fn("int", ScriptFunction::int);
        engine.register_fn("*", ScriptFunction::mul_f32_u32);
        ScriptEngine(engine)
    }

    pub(crate) fn run(script: &String) -> String {
        let millis = 1000;
        match ScriptEngine::eval_with_timeout(script, millis) {
            Ok(result) => result,
            Err(_) => format!("Script Error: Timeout after {} seconds", millis / 1000),
        }
    }

    fn eval_with_timeout(script: &String, timeout_millis: u64) -> Result<String, TimeoutError> {
        let (sender, receiver) = bounded(0);
        let timeout_sender = sender.clone();
        let script = script.clone();
        thread::spawn(move || {
            let mut script_engine = ScriptEngine::new();
            let result = match script_engine.0.eval::<String>(script.as_str()) {
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
            };

            match sender.send(Ok(result)) {
                Ok(()) => {} // Finished in time.
                Err(_) => {} // Timed out.
            }
        });
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(timeout_millis));
            match timeout_sender.send(Err(TimeoutError)) {
                Ok(()) => {} // Timed out.
                Err(_) => {} // Finished in time.
            }
        });
        return receiver.recv().unwrap(); // Catch whatever comes first, finish or timeout
    }
}

struct ScriptFunction {}

impl ScriptFunction {
    fn to_string<T: Display>(x: T) -> String {
        format!("{}", x)
    }

    fn random() -> f32 {
        let mut rng = rand::thread_rng();
        rng.gen_range(0.0, 1.0)
    }

    fn len(x: Vec<Box<dyn Any>>) -> i64 {
        x.len().try_into().unwrap()
    }

    fn floor(x: f32) -> i64 {
        x.floor() as i64
    }

    fn int(x: String) -> i64 {
        match x.parse() {
            Ok(i) => i,
            Err(_) => 0,
        }
    }

    fn mul_f32_u32(x: f32, y: i64) -> f32 {
        x * (y as f32)
    }
}
