//! Flacora's library core: the download engine, extension model, store and UI.
//!
//! The GUI binary (`main.rs`) and the headless web server (`src/bin/server.rs`)
//! both link this crate. Everything here is shared; the UI modules are only
//! reached through [`app::App`], which the server drives without a window.

pub mod app;
pub mod catalog;
pub mod extensions;
pub mod fixtures;
pub mod gui;
pub mod icons;
pub mod jobs;
pub mod library;
pub mod output;
pub mod package_picker;
pub mod protocol;
pub mod runner;
pub mod services;
pub mod store;
pub mod track_links;
pub mod ui;
pub mod updates;
#[cfg(windows)]
pub mod window;

// Window-level items the UI tests and the engine reach as `crate::` paths.
pub use gui::{LOGO, MIN_WINDOW_SIZE, on_key, poll};
