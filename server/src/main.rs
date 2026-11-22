//! Flacora's headless server: the download engine plus a web UI.

mod api;
mod engine;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

fn main() {
    let host = std::env::var("FLACORA_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("FLACORA_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);

    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("flacora-engine".to_string())
        .spawn(move || engine::run(cmd_rx, state_tx))
        .expect("could not start the engine thread");

    let shared = Arc::new(api::Shared { cmd_tx, state: Mutex::new(None) });
    let forward = shared.clone();
    std::thread::Builder::new()
        .name("flacora-state".to_string())
        .spawn(move || {
            while let Ok(state) = state_rx.recv() {
                if let Ok(mut slot) = forward.state.lock() {
                    *slot = Some(state);
                }
            }
        })
        .expect("could not start the state thread");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("could not start tokio");
    runtime.block_on(api::serve(host, port, shared));
}
