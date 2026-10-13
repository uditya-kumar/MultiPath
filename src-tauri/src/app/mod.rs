//! The Tauri side: shared state, settings, commands, the ticker, and the
//! tray popup. Also the headless runner, which drives the same state without
//! Tauri.

pub mod autostart;
pub mod commands;
pub mod headless;
pub mod popup;
pub mod settings;
pub mod state;
pub mod status;
pub mod ticker;
pub mod tray;
pub mod usage;
