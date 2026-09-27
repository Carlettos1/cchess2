use cchess_session::{handle, Config, Session};
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Write},
    sync::{Arc, Mutex},
};
fn main() {
    let session = Arc::new(Mutex::new(
        Session::new(Config::default())
            .expect("built-in game")
            .with_storage(
                std::env::current_dir()
                    .expect("working directory")
                    .join(".local/cchess-save.json"),
            ),
    ));
    let output = Arc::new(Mutex::new(io::stdout()));
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let session = session.clone();
        let output = output.clone();
        std::thread::spawn(move || {
            let request: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => return,
            };
            let result = handle(&session, request["request"].clone());
            let response = match result {
                Ok(value) => json!({"id":request["id"],"value":value}),
                Err(error) => json!({"id":request["id"],"error":error}),
            };
            let mut out = output.lock().unwrap();
            let _ = writeln!(out, "{response}");
            let _ = out.flush();
        });
    }
}
