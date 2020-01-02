use rand::Rng;
use rhai::{Any, AnyExt, Engine, EvalAltResult, RegisterFn};
use std::convert::TryInto;
use std::fmt::Display;
use std::time::Duration;
use async_std::future;
use async_std::future::TimeoutError;
use async_std::task;

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
        engine.register_fn("*", ScriptFunction::mul_f64_i64);
        ScriptEngine(engine)
    }

    pub(crate) fn run(script: &String) -> String {
        let millis = 1000;
        match task::block_on(ScriptEngine::eval_with_timeout(script, millis)) {
            Ok(result) => result,
            Err(_) => format!("Script Error: Timeout after {} seconds", millis / 1000),
        }
    }

    async fn eval_with_timeout(script: &String, timeout_millis: u64) -> Result<String, TimeoutError> {
        let script = script.clone();
        let task = task::spawn(async move {
            let mut script_engine = ScriptEngine::new();
            match script_engine.0.eval::<String>(script.as_str()) {
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
            }
        });
        future::timeout(Duration::from_millis(timeout_millis), task).await
    }
}

struct ScriptFunction {}

impl ScriptFunction {
    fn to_string<T: Display>(x: T) -> String {
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
